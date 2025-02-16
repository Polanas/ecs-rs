use std::{cell::RefCell, collections::HashMap, ops::Range, rc::Rc};

use mlua::{FromLua, IntoLua, IntoLuaMulti, Lua, Table};
use smol_str::ToSmolStr;

use crate::{
    archetypes::{ComponentAddState, EntityKind, TableReusage},
    entity::Entity,
    identifier::Identifier,
    world::{archetypes, archetypes_mut},
};

pub struct LuaApi {
    pub lua: Lua,
}

trait LuaMultiExt<'lua> {
    fn multi_error(&'lua self, error: impl Into<String>) -> mlua::Result<mlua::MultiValue<'lua>>;
    fn single_error(&'lua self, error: impl Into<String>) -> mlua::Result<mlua::Value<'lua>>;
    fn multi_value(&'lua self, value: mlua::Value<'lua>) -> mlua::MultiValue<'lua>;
}

impl<'lua> LuaMultiExt<'lua> for Lua {
    fn multi_error(&self, error: impl Into<String>) -> mlua::Result<mlua::MultiValue> {
        Ok(mlua::MultiValue::from_vec(vec![
            error.into().into_lua(self)?,
            mlua::Value::Boolean(true),
        ]))
    }

    fn multi_value(&'lua self, value: mlua::Value<'lua>) -> mlua::MultiValue<'lua> {
        mlua::MultiValue::from_vec(vec![value])
    }

    fn single_error(&'lua self, error: impl Into<String>) -> mlua::Result<mlua::Value<'lua>> {
        error.into().into_lua(self)
    }
}

impl LuaApi {
    pub fn new() -> mlua::Result<Self> {
        let lua = unsafe { Lua::unsafe_new() };
        Self::create_api(&lua)?;
        Ok(Self { lua })
    }

    fn create_api(lua: &Lua) -> mlua::Result<()> {
        // NOTE: it's better to accept mlua Values instead of concrete rust types, so that passing
        // arguments of wrong types can be handled without raising errors
        // TODO: add is_alive checks
        macro_rules! add_fns {
            ($($name:ident),+) => {
                $(
                        lua
                        .globals()
                        .set(stringify!($name), &$name)?;
                )+
            };
        }
        let entity_by_global_name =
            lua.create_function(|lua, name: mlua::Value| -> mlua::Result<mlua::MultiValue> {
                let Some(name) = name.as_str() else {
                    return lua
                        .multi_error(format!("expected entity name, got {}", name.type_name()));
                };
                match archetypes(|a| a.entity_by_global_name(name.to_smolstr())) {
                    Some(entity) => Ok(mlua::MultiValue::from_vec(vec![mlua::Value::Integer(
                        Entity::new(entity).into_lua(lua)?.as_integer().unwrap(),
                    )])),
                    None => lua.multi_error(format!("could not find entity with name {}", name)),
                }
            })?;
        let global_name_by_entity = lua.create_function(
            |lua, entity: mlua::Value| -> mlua::Result<mlua::MultiValue> {
                let entity_type_name = entity.type_name();
                let Ok(entity) = Entity::from_lua(entity, lua) else {
                    return lua.multi_error(format!("expected entity, got {}", entity_type_name));
                };
                match entity.get_name() {
                    Some(name_getter) => Ok(lua.multi_value(mlua::Value::String(
                        name_getter.get(|n| lua.create_string(n.as_bytes()))?,
                    ))),
                    None => {
                        lua.multi_error(format!("could not find name of {}", entity.debug_name()))
                    }
                }
            },
        )?;
        let add_entity = lua.create_function(
            |lua, name: Option<mlua::Value>| -> mlua::Result<mlua::Value> {
                //TODO: add support of adding component entities
                let entity = archetypes_mut(|a| Entity::new(a.add_entity(EntityKind::Regular)));
                dbg!(&name);
                if let Some(name) = name {
                    if let Some(name) = name.as_str() {
                        entity.set_name(name);
                    }
                }
                entity.into_lua(lua)
            },
        )?;
        let add_entity_tag =
            lua.create_function(|_, entity: mlua::Value| -> mlua::Result<()> { Ok(()) })?;
        let add_comp = lua.create_function(
            |lua, args: (mlua::Value, mlua::Value, mlua::Value)| -> mlua::Result<mlua::Value> {
                let comp_name_value = match args.1 {
                    mlua::Value::String(string) => string,
                    value => {
                        return lua.single_error(format!(
                            "expected component name, got {}",
                            value.type_name()
                        ))
                    }
                };
                let comp_name = &comp_name_value.to_str().unwrap();

                let Some(entity) = args.0.as_u64() else {
                    return lua.single_error(format!(
                        "expected entity to be integer, got {}",
                        args.0.type_name()
                    ));
                };
                let entity = Identifier(entity.to_ne_bytes());
                archetypes_mut(|a| {
                    if !a.is_entity_alive(entity) {
                        return lua.single_error(format!(
                            "entity {:?} is not alive",
                            Entity::new(entity)
                        ));
                    }
                    let type_registry = a.type_registry_rc().clone();
                    let type_registry = type_registry.borrow();
                    let Some(component_id) = type_registry.identifiers_by_names.get(*comp_name)
                    else {
                        return lua.single_error(format!("could not find component {}", comp_name));
                    };
                    let fns = &type_registry.functions[&component_id.stripped()];
                    match a.add_component(*component_id, entity, TableReusage::New) {
                        Ok((comp_archetype, add_state)) => {
                            match add_state {
                                ComponentAddState::New => {
                                    // let record = a.record(entity).unwrap();
                                    let comp_archetype = comp_archetype.borrow();
                                    let table = comp_archetype.table().borrow();
                                    let Some(storage) = table.storage(*component_id) else {
                                        return lua.single_error(format!(
                                            "component {} does not contain any data",
                                            a.debug_id_name(*component_id)
                                        ));
                                    };
                                    let storage = storage.borrow_mut();
                                    (fns.from_lua)(args.2, storage, lua).unwrap();
                                }
                                ComponentAddState::AlreadyExists => {
                                    //TODO: handle that
                                }
                            }
                        }
                        Err(err) => return lua.single_error(err.to_string()),
                    };
                    Ok(mlua::Value::Nil)
                })
            },
        )?;

        add_fns!(
            entity_by_global_name,
            global_name_by_entity,
            add_entity,
            add_comp,
            add_entity_tag
        );

        Ok(())
    }
}
