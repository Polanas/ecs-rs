use std::{cell::RefCell, collections::HashMap, io::Result, ops::Range, ptr::NonNull, rc::Rc};

use log::error;
use mlua::{FromLua, IntoLua, IntoLuaMulti, Lua, LuaSerdeExt, Table};
use num::ToPrimitive;
use smol_str::ToSmolStr;

use crate::{
    archetypes::{ComponentAddState, EntityKind, TableReusage},
    entity::Entity,
    identifier::Identifier,
    world::{self, archetypes, archetypes_mut},
};

pub struct LuaApi {
    pub lua: Lua,
}

impl LuaApi {
    pub fn new() -> mlua::Result<Self> {
        let lua = unsafe { Lua::unsafe_new() };
        Self::create_api(&lua)?;
        Ok(Self { lua })
    }

    fn create_api(lua: &Lua) -> mlua::Result<()> {
        // TODO: add is_alive checks
        macro_rules! add_fns {
            ($internal:ident, $($name:ident),+ $(,)?) => {
                $(
                        $internal
                        .set(stringify!($name), &$name)?;
                )+
            };
        }
        let internal_table = lua.create_table()?;
        lua.globals().set("__internal", &internal_table)?;
        let component_metatable = lua.create_table()?;
        component_metatable.raw_set(
            "__index",
            lua.create_function(
                |_, args: (mlua::Table, mlua::Value)| -> mlua::Result<mlua::Value> {
                    let (component, key) = args;
                    component
                        .raw_get::<_, mlua::Table>("data")?
                        .raw_get::<_, mlua::Value>(key)
                },
            )?,
        )?;
        component_metatable.raw_set(
            "__newindex",
            lua.create_function(
                |_, args: (mlua::Table, mlua::Value, mlua::Value)| -> mlua::Result<mlua::Value> {
                    let (component, key, value) = args;
                    let _ = component
                        .raw_get::<_, mlua::Table>("data")?
                        .raw_set::<_, mlua::Value>(key, value);
                    Ok(mlua::Value::Nil)
                },
            )?,
        )?;
        internal_table.raw_set("component_metatable", component_metatable)?;
        //used to store metatables for objects so that they can be injected when calling into_lua
        internal_table.raw_set("object_metatables", lua.create_table()?)?;
        //random useful functions
        internal_table.raw_set("utils", lua.create_table()?)?;

        let entity_by_global_name =
            lua.create_function(|lua, name: mlua::String| -> mlua::Result<mlua::Value> {
                if let Ok(name) = name.to_str() {
                    match archetypes(|a| a.entity_by_global_name(name.to_smolstr())) {
                        Some(entity) => Ok(Entity::new(entity).into_lua(lua)?),
                        None => Ok(mlua::Value::Nil),
                    }
                } else {
                    Ok(mlua::Value::Nil)
                }
            })?;
        let global_name_by_entity =
            lua.create_function(|lua, entity: Entity| -> mlua::Result<mlua::Value> {
                if !entity.is_alive() {
                    return Ok(mlua::Value::Nil);
                }
                match entity.get_name() {
                    Some(name_getter) => Ok(mlua::Value::String(
                        name_getter.get(|n| lua.create_string(n.as_bytes()))?,
                    )),
                    None => Ok(mlua::Value::Nil),
                }
            })?;
        let add_entity =
            lua.create_function(|_, name: Option<String>| -> mlua::Result<Entity> {
                //TODO: add support of adding component entities
                let entity = archetypes_mut(|a| Entity::new(a.add_entity(EntityKind::Regular)));
                if let Some(name) = &name {
                    entity.set_name(name);
                }
                Ok(entity)
            })?;
        let has_ent_tag =
            lua.create_function(|_, args: (Entity, Entity)| -> mlua::Result<bool> {
                let (entity, tag) = args;
                Ok(entity.has_ent_tag(&tag))
            })?;
        let add_ent_tag = lua.create_function(|_, args: (Entity, Entity)| -> mlua::Result<()> {
            let (entity, tag) = args;
            archetypes_mut(|a| {
                if let Err(error) = a.add_entity_tag(entity.id(), tag.id()) {
                    error!("while adding entity tag: {}", error.to_string());
                }
            });
            Ok(())
        })?;
        let remove_entity = lua.create_function(|_, entity: Entity| -> mlua::Result<()> {
            entity.remove();
            Ok(())
        })?;
        let unpack_entity =
            lua.create_function(|lua, entity: Entity| -> mlua::Result<mlua::Value> {
                if !entity.is_alive() {
                    return Ok(mlua::Value::Nil);
                }
                let Some(record) = archetypes(|a| a.record(entity.id())) else {
                    return Ok(mlua::Value::Nil);
                };
                let table = lua.create_table()?;
                let unpacked = record.entity.unpack();
                table.raw_set("low32", unpacked.low32)?;
                table.raw_set("second", unpacked.high32.second.to_u32().unwrap())?;
                table.raw_set("is_target", unpacked.high32.is_target)?;
                table.raw_set("is_relation", unpacked.high32.is_relation)?;
                table.raw_set(
                    "is_relation_exclusive",
                    unpacked.high32.is_relation_exclusive,
                )?;
                table.raw_set("is_active", unpacked.high32.is_active)?;
                table.raw_set("is_relationship", unpacked.high32.is_relationship)?;
                Ok(mlua::Value::Table(table))
            })?;

        let is_entity_alive = lua
            .create_function(|_, entity: Entity| -> mlua::Result<bool> { Ok(entity.is_alive()) })?;
        let get_comp = lua.create_function(
            |lua, args: (Entity, mlua::Table)| -> mlua::Result<mlua::Value> {
                let (entity, component) = args;
                let Ok(comp_name) = component.raw_get::<_, String>("comp_name") else {
                    error!("while getting component: could not get component name");
                    return Ok(mlua::Value::Nil);
                };
                archetypes_mut(|a| {
                    if !a.is_entity_alive(entity.id()) {
                        error!("while getting component {comp_name}: entity was not alive");
                        return Ok(mlua::Value::Nil);
                    };

                    let type_registry = a.type_registry_rc().clone();
                    let type_registry = type_registry.borrow();
                    let Some(component_id) =
                        type_registry.identifiers_by_names.get(comp_name.as_str())
                    else {
                        error!("while getting component {comp_name}: could not find component");
                        return Ok(mlua::Value::Nil);
                    };
                    let fns = &type_registry.functions[&component_id.stripped()];
                    let Some(record) = a.record(entity.id()) else {
                        error!("while getting component {comp_name}: could not add component");
                        return Ok(mlua::Value::Nil);
                    };
                    let Some(archetype) = a.archetype_from_record(&record) else {
                        error!("while getting component {comp_name}: could not add component");
                        return Ok(mlua::Value::Nil);
                    };

                    let archetype = archetype.borrow();
                    let table_mut = archetype.table().borrow_mut();
                    let storage = table_mut.storage(*component_id).unwrap().borrow_mut();
                    let component_ptr: *mut u8 =
                        unsafe { storage.0.get_checked(record.table_row.0).as_ptr() };
                    let component_ptr: bevy_ptr::Ptr<'_> =
                        unsafe { bevy_ptr::Ptr::new(NonNull::new(component_ptr).unwrap()) };
                    match (fns.into_lua)(component_ptr, lua) {
                        Ok(value) => {
                            let table = lua.create_table()?;
                            let component_metatable = lua
                                .globals()
                                .raw_get::<_, Table>("__internal")?
                                .raw_get::<_, Table>("component_metatable")?;
                            table.raw_set("comp_name", comp_name)?;
                            table.raw_set("data", value)?;
                            table.set_metatable(Some(component_metatable));
                            Ok(mlua::Value::Table(table))
                        }
                        Err(error) => {
                            error!("while adding component {comp_name}: {}", error.to_string());
                            Ok(mlua::Value::Nil)
                        }
                    }
                })
            },
        )?;
        let set_comp = lua.create_function(|lua, args: (Entity, mlua::Table)| -> mlua::Result<mlua::Value> {
                let (entity, component) = args;
                let Ok(data) = component.raw_get::<_, mlua::Value>("data") else {
                    error!("while adding component: could not get component data");
                    return Ok(mlua::Value::Nil);
                };
                let Ok(comp_name) = component.raw_get::<_, String>("comp_name") else {
                    error!("while adding component: could not get component name");
                    return Ok(mlua::Value::Nil);
                };
                archetypes_mut(|a| {
                    if !a.is_entity_alive(entity.id()) {
                        error!("while getting component {comp_name}: entity was not alive");
                        return Ok(mlua::Value::Nil);
                    };

                    let type_registry = a.type_registry_rc().clone();
                    let type_registry = type_registry.borrow();
                    let Some(component_id) =
                        type_registry.identifiers_by_names.get(comp_name.as_str())
                    else {
                        error!("while getting component {comp_name}: could not find component");
                        return Ok(mlua::Value::Nil);
                    };
                    let fns = &type_registry.functions[&component_id.stripped()];
                    let Some(record) = a.record(entity.id()) else {
                        error!("while getting component {comp_name}: could not add component");
                        return Ok(mlua::Value::Nil);
                    };
                    let Some(archetype) = a.archetype_from_record(&record) else {
                        error!("while getting component {comp_name}: could not add component");
                        return Ok(mlua::Value::Nil);
                    };
                    let archetype = archetype.borrow();
                    let table_mut = archetype.table().borrow_mut();
                    let storage = table_mut.storage(*component_id).unwrap().borrow_mut();
                    (fns.from_lua)(data, storage, Some(record.table_row.0), lua)?;
                    Ok(mlua::Value::Nil)
                })
        })?;
        let add_comp = lua.create_function(
            |lua, args: (Entity, mlua::Table)| -> mlua::Result<mlua::Value> {
                let (entity, component) = args;
                let Ok(data) = component.raw_get::<_, mlua::Value>("data") else {
                    error!("while adding component: could not get component data");
                    return Ok(mlua::Value::Nil);
                };
                let Ok(comp_name) = component.raw_get::<_, String>("comp_name") else {
                    error!("while adding component: could not get component name");
                    return Ok(mlua::Value::Nil);
                };

                archetypes_mut(|a| {
                    if !a.is_entity_alive(entity.id()) {
                        error!("while adding component {comp_name}: entity was not alive");
                        return Ok(mlua::Value::Nil);
                    }
                    let utils = lua.globals().raw_get::<_, mlua::Table>("__internal")?.raw_get::<_, mlua::Table>("utils")?;
                    let is_lua_component = utils.raw_get::<_, mlua::Function>("is_lua_component")?;
                    let get_lua_component = utils.raw_get::<_, mlua::Function>("get_lua_component")?;
                    if is_lua_component.call::<_, bool>(&component)? {
                        return get_lua_component.call::<_, mlua::Value>((entity, component));
                    }

                    let type_registry = a.type_registry_rc().clone();
                    let type_registry = type_registry.borrow();
                    let Some(component_id) = type_registry.identifiers_by_names.get(comp_name.as_str())
                    else {
                        error!("while adding component {comp_name}: could not find component");
                        return Ok(mlua::Value::Nil);
                    };
                    let fns = &type_registry.functions[&component_id.stripped()];
                    match a.add_component(*component_id, entity.id(), TableReusage::New) {
                         
                        Ok((comp_archetype, add_state)) => {
                            let comp_archetype = comp_archetype.borrow();
                            let table = comp_archetype.table().borrow();
                            let Some(storage) = table.storage(*component_id) else {
                                error!("while adding component {comp_name}: component did not contain any data");
                                    return Ok(mlua::Value::Nil);
                                };
                            let storage = storage.borrow_mut();

                            match add_state {
                                ComponentAddState::New => {
                                    (fns.from_lua)(data, storage, None, lua)?;
                                }
                                ComponentAddState::AlreadyExists => {
                                    let record = a.record(entity.id()).unwrap();
                                    (fns.from_lua)(data, storage, Some(record.table_row.0), lua)?;
                                }
                            }
                        }
                        Err(err) => {
                            error!("while adding component {comp_name}: {0}", err.to_string());
                        },
                    };
                    Ok(mlua::Value::Nil)
                })
            },
        )?;

        add_fns!(
            internal_table,
            entity_by_global_name,
            global_name_by_entity,
            add_entity,
            add_comp,
            add_ent_tag,
            has_ent_tag,
            remove_entity,
            is_entity_alive,
            unpack_entity,
            get_comp,
            set_comp,
        );

        Ok(())
    }
}
