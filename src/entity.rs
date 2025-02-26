use std::marker::PhantomData;
pub use std::{fmt::Debug, hash::Hash, os::unix::process::parent_id};

use smol_str::SmolStr;

use crate::{
    archetypes::{
        self, Archetypes, ChildOf, ComponentGetter, EntityNameGetter, EntityRecord,
        GetComponentError, InstanceOf, NameLeft, Prefab, TableReusage, TryGetComponent, Wildcard,
        WILDCARD_RELATIONSHIP,
    },
    children_iter::ChildrenRecursiveIter,
    components::{
        component::{AbstractComponent, EnumTag},
        component_bundle::ComponentBundle,
        component_query::ComponentQuery,
        components_iter::{ComponentsReflectIter, ComponentsReflectIterMut},
    },
    expect_fn::ExpectFnResult,
    identifier::Identifier,
    query::{Query, QueryState},
    relationship::{FindRelationshipsIter, Relationship, RelationshipsIter},
    world::{archetypes, archetypes_mut},
};

#[derive(Clone)]
pub struct Entity(pub(crate) Identifier, PhantomData<()>);

impl<'lua> mlua::FromLua<'lua> for Entity {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Table(value) => Ok(Self::new(Identifier(
                value.get::<_, mlua::Integer>(1)?.to_ne_bytes(),
            ))),
            value => {
                let type_name = value.type_name();
                Err(mlua::Error::FromLuaConversionError {
                    from: type_name,
                    to: "Entity",
                    message: Some(format!("expected Integer, got {}", type_name)),
                })
            }
        }
    }
}

impl<'lua> mlua::IntoLua<'lua> for Entity {
    fn into_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
        let table = lua.create_table()?;
        table.set(1, mlua::Value::Integer(i64::from_ne_bytes(self.0 .0)))?;
        Ok(mlua::Value::Table(table))
    }
}

impl From<Entity> for Identifier {
    fn from(value: Entity) -> Self {
        value.0
    }
}

impl From<&Entity> for Identifier {
    fn from(value: &Entity) -> Self {
        value.0
    }
}

impl From<&mut Entity> for Identifier {
    fn from(value: &mut Entity) -> Self {
        value.0
    }
}

impl From<Identifier> for Entity {
    fn from(value: Identifier) -> Self {
        Entity(value, Default::default())
    }
}

pub const WILDCARD: Entity = Entity::new(WILDCARD_RELATIONSHIP);

impl Entity {
    pub fn id(&self) -> Identifier {
        self.0
    }
    pub const fn new(id: Identifier) -> Self {
        Self(id, PhantomData)
    }
}

impl PartialEq for Entity {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Entity {}

impl PartialOrd for Entity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl Hash for Entity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Debug for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Entity").field(&self.0).finish()
    }
}

impl Entity {
    pub fn serialize(&self) -> Option<String> {
        archetypes(|archetypes| archetypes.serialize_entity(self.0))
    }

    pub fn debug_name(&self) -> SmolStr {
        archetypes(|archetypes| archetypes.debug_id_name(self.0))
    }

    pub fn name_parent_or_wildcard(&self) -> Entity {
        if let Some(parent) = self.parent() {
            parent
        } else {
            WILDCARD
        }
    }
    pub fn set_name(&self, name: &str) -> Self {
        self.name().set(name);
        self.clone()
    }
    pub fn has_name(&self) -> bool {
        let parent = self.name_parent_or_wildcard();
        archetypes(|archetypes| archetypes.entity_has_name(&NameLeft::from_ids(self.0, parent.0)))
    }
    pub fn remove_name(&self) -> Self {
        let parent = self.name_parent_or_wildcard();
        archetypes_mut(|archetypes| archetypes.remove_entity_name((self.0, parent.0).into()));
        self.clone()
    }
    pub fn get_name(&self) -> Option<EntityNameGetter> {
        if !self.has_name() {
            return None;
        }
        let parent = self.name_parent_or_wildcard();
        Some(EntityNameGetter::new((self.0, parent.0).into()))
    }

    pub fn name(&self) -> EntityNameGetter {
        let parent = self.name_parent_or_wildcard();
        EntityNameGetter::new((self.0, parent.0).into())
    }

    pub fn parent(&self) -> Option<Entity> {
        self.find_rel::<ChildOf, Wildcard>().map(|r| r.target())
    }

    pub fn find_mixed_rels<R: AbstractComponent>(&self, target: &Entity) -> FindRelationshipsIter {
        archetypes_mut(|archetypes| {
            let relation = archetypes.component_id::<R>();
            let record = archetypes.record(self.0).unwrap();
            let archetype = archetypes.archetype_from_record(&record).unwrap();
            FindRelationshipsIter::from_archetype(archetype, relation, target.0)
        })
    }
    pub fn find_ent_rels(&self, relation: &Entity, target: Entity) -> FindRelationshipsIter {
        archetypes(|archetypes| {
            let record = archetypes.record(self.0).unwrap();
            let archetype = archetypes.archetype_from_record(&record).unwrap();
            FindRelationshipsIter::from_archetype(archetype, relation.0, target.0)
        })
    }
    pub fn find_rel<R: AbstractComponent, T: AbstractComponent>(&self) -> Option<Relationship> {
        let (archetype, relation, target) = archetypes_mut(|archetypes| {
            let relation = archetypes.component_id::<R>();
            let target = archetypes.component_id::<T>();
            let record = archetypes.record(self.0).unwrap();
            let archetype = archetypes.archetype_from_record(&record).unwrap();
            (archetype.clone(), relation, target)
        });
        //TODO: refactor rel methods
        FindRelationshipsIter::from_archetype(&archetype, relation, target).next()
    }
    pub fn find_rels<R: AbstractComponent, T: AbstractComponent>(&self) -> FindRelationshipsIter {
        archetypes_mut(|archetypes| {
            let record = archetypes.record(self.0).unwrap();
            archetypes.find_rels::<R, T>(&record).unwrap()
        })
    }

    pub fn iter_comps_reflect(&self) -> ComponentsReflectIter {
        archetypes(|archetypes| {
            let record = archetypes.record(self.0).unwrap();
            let archetype = archetypes.archetype_by_id(record.arhetype_id);
            ComponentsReflectIter::new(
                archetype.clone(),
                record,
                &self.1,
                archetypes.type_registry_rc(),
            )
        })
    }

    pub fn iter_comps_reflect_mut(&self) -> ComponentsReflectIterMut {
        archetypes(|archetypes| {
            let record = archetypes.record(self.0).unwrap();
            let archetype = archetypes.archetype_by_id(record.arhetype_id);
            ComponentsReflectIterMut::new(
                archetype.clone(),
                record,
                &self.1,
                archetypes.type_registry_rc(),
            )
        })
    }

    pub fn iter_rels(&self) -> RelationshipsIter {
        archetypes(|archetypes| {
            let record = archetypes.record(self.0).unwrap();
            let archetype = archetypes.archetype_from_record(&record).unwrap();
            RelationshipsIter::new(archetype)
        })
    }
    pub fn has_relationship(&self, relationship: Relationship) -> bool {
        archetypes(|archetypes| archetypes.has_component(relationship.0, self.0))
    }
    pub fn add_child_of(&mut self, parent: &Entity) -> Self {
        let name_parent = self.name_parent_or_wildcard();
        let old_entity_and_parent = NameLeft::from_ids(self.into(), name_parent.into());
        self.add_mixed_tag_rel::<ChildOf>(&parent);
        archetypes_mut(|archetypes| {
            if archetypes.name_by_entity(&old_entity_and_parent).is_some() {
                let name = archetypes
                    .name_by_entity(&old_entity_and_parent)
                    .unwrap()
                    .to_owned();
                let entity_and_parent = NameLeft::from_ids(self.into(), parent.into());
                archetypes.remove_entity_name(old_entity_and_parent);
                archetypes.set_entity_name(entity_and_parent, name);
            }
        });
        if !parent.is_active() {
            self.diactivate();
        }
        self.clone()
    }

    pub fn is_child_of(&self, parent: &Entity) -> bool {
        self.has_mixed_rel::<ChildOf>(parent)
    }

    pub fn remove_child_of(&self, parent: &Entity) {
        let old_entity_and_parent = NameLeft::from_ids(self.into(), parent.into());
        self.remove_mixed_rel::<ChildOf>(parent);
        archetypes_mut(|archetypes| {
            if archetypes.name_by_entity(&old_entity_and_parent).is_some() {
                let name = archetypes
                    .name_by_entity(&old_entity_and_parent)
                    .unwrap()
                    .to_owned();
                let entity_and_parent = NameLeft::from_ids(self.into(), WILDCARD.into());
                archetypes.remove_entity_name(old_entity_and_parent);
                archetypes.set_entity_name(entity_and_parent, name);
            }
        })
    }

    pub fn remove_all_child_of_rels(&self) {
        archetypes_mut(|archetypes| {
            for rel in self.find_rels::<ChildOf, Wildcard>() {
                let _ = archetypes.remove_component(rel.id(), self.0, TableReusage::Reuse);
            }
        });
    }

    pub fn children_recursive(&self) -> ChildrenRecursiveIter {
        let children_pool = archetypes(|a| a.children_pool().clone());
        ChildrenRecursiveIter::new(self.0, children_pool)
    }

    pub fn children(&self) -> Query<&Entity> {
        QueryState::<&Entity, ()>::new()
            .with_rel::<ChildOf, Wildcard>()
            .build()
    }

    pub fn is_alive(&self) -> bool {
        archetypes(|a| a.is_entity_alive(self.0))
    }

    pub fn add_comp<T: ComponentBundle>(&mut self, bundle: T) -> Entity {
        assert!(std::mem::size_of::<T>() > 0);
        bundle.add(self);
        self.clone()
    }

    pub fn get_or_add_comp<T: AbstractComponent>(
        &mut self,
        init: impl FnOnce() -> T,
        get: impl FnOnce(&T),
    ) {
        assert!(std::mem::size_of::<T>() > 0);
        if !self.has_comp::<T>() {
            self.add_comp(init());
        }
        self.comp::<T>(get);
    }

    pub fn get_or_add_comp_mut<T: AbstractComponent>(
        &mut self,
        init: impl FnOnce() -> T,
        get: impl FnOnce(&mut T),
    ) {
        assert!(std::mem::size_of::<T>() > 0);
        if !self.has_comp::<T>() {
            self.add_comp(init());
        }
        self.comp_mut::<T>(get);
    }

    pub fn remove_comp<T: ComponentBundle>(&self) -> Entity {
        assert!(std::mem::size_of::<T>() > 0);
        T::remove(self);
        self.clone()
    }

    pub fn has_enum_tag<T: EnumTag>(&self, tag: T) -> bool {
        archetypes_mut(|archetypes| archetypes.has_enum_tag(tag, self.0))
    }

    pub fn add_enum_tag<T: EnumTag>(&mut self, tag: T) -> Entity {
        archetypes_mut(|archetypes| {
            archetypes.add_enum_tag(self.0, tag).unwrap();
        });
        self.clone()
    }

    pub fn enum_tag<T: EnumTag>(&self) -> T {
        archetypes_mut(|archetypes| archetypes.get_enum_tag::<T>(self.0).unwrap())
    }

    pub fn get_enum_tag<T: EnumTag>(&self) -> Option<T> {
        archetypes_mut(|archetypes| archetypes.get_enum_tag::<T>(self.0))
    }

    pub fn has_any_enum_tag<T: EnumTag>(&self) -> bool {
        self.get_enum_tag::<T>().is_some()
    }

    pub fn remove_enum_tag<T: EnumTag>(&self) -> Entity {
        archetypes_mut(|archetypes| {
            let _ = archetypes.remove_enum_tag::<T>(self.0);
        });
        self.clone()
    }

    pub fn has_comp<T: AbstractComponent>(&self) -> bool {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes.has_component(id, self.0)
        })
    }

    pub fn remove_tag<T: AbstractComponent>(&self) -> Entity {
        assert!(std::mem::size_of::<T>() == 0);
        archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            let _ = archetypes.remove_component(id, self.0, archetypes::TableReusage::Reuse);
        });
        self.clone()
    }

    pub fn add_rel_second<R: AbstractComponent, T: AbstractComponent>(&mut self, value: T) -> Self {
        assert!(std::mem::size_of::<R>() == 0);
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<R>();
            let target_id = archetypes.component_id::<T>();
            archetypes
                .add_data_relationship_typed(self.0, relation_id, target_id, value)
                .unwrap();
        });

        self.clone()
    }

    pub fn rel_second_ret<R: AbstractComponent, T: AbstractComponent, U>(
        &self,
        f: impl FnOnce(&T) -> U,
    ) -> U {
        assert!(std::mem::size_of::<R>() == 0);
        assert!(std::mem::size_of::<T>() > 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes
                .get_component(relationship, self.0)
                .expect_fn(|err| panic!("{err}"))
                .get(|c| f(c))
        })
    }

    pub fn rel_second_mut_ret<R: AbstractComponent, T: AbstractComponent, U>(
        &mut self,
        f: impl FnOnce(&mut T) -> U,
    ) -> U {
        assert!(std::mem::size_of::<R>() == 0);
        assert!(std::mem::size_of::<T>() > 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes
                .get_component(relationship, self.0)
                .expect_fn(|err| panic!("{err}"))
                .get_mut(|c| f(c))
        })
    }

    pub fn rel_second<R: AbstractComponent, T: AbstractComponent>(
        &self,
        f: impl FnOnce(&T),
    ) -> &Self {
        assert!(std::mem::size_of::<R>() == 0);
        assert!(std::mem::size_of::<T>() > 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes
                .get_component(relationship, self.0)
                .expect_fn(|err| panic!("{err}"))
                .get(f);
        });
        self
    }

    pub fn get_rel_second<R: AbstractComponent, T: AbstractComponent>(
        &self,
        f: impl FnOnce(Result<&T, GetComponentError>),
    ) -> &Self {
        assert!(std::mem::size_of::<R>() == 0);
        assert!(std::mem::size_of::<T>() > 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes.get_component(relationship, self.0).try_get(f);
        });
        self
    }

    pub fn get_rel_second_ret<R: AbstractComponent, T: AbstractComponent, U>(
        &self,
        f: impl FnOnce(Result<&T, GetComponentError>) -> U,
    ) -> U {
        assert!(std::mem::size_of::<R>() == 0);
        assert!(std::mem::size_of::<T>() > 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes.get_component(relationship, self.0).try_get(f)
        })
    }

    pub fn rel_second_mut<R: AbstractComponent, T: AbstractComponent>(
        &mut self,
        f: impl FnOnce(&mut T),
    ) -> &mut Self {
        assert!(std::mem::size_of::<R>() == 0);
        assert!(std::mem::size_of::<T>() > 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes
                .get_component(relationship, self.0)
                .expect_fn(|err| panic!("{err}"))
                .get_mut(f);
        });
        self
    }

    pub fn get_rel_second_mut<R: AbstractComponent, T: AbstractComponent>(
        &mut self,
        f: impl FnOnce(Result<&mut T, GetComponentError>),
    ) -> &mut Self {
        assert!(std::mem::size_of::<R>() == 0);
        assert!(std::mem::size_of::<T>() > 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes
                .get_component(relationship, self.0)
                .try_get_mut(f);
        });
        self
    }

    pub fn add_rel_first<R: AbstractComponent, T: AbstractComponent>(&mut self, value: R) -> Self {
        assert!(std::mem::size_of::<R>() > 0);
        assert!(std::mem::size_of::<T>() == 0);
        {
            archetypes_mut(|archetypes| {
                let relation_id = archetypes.component_id::<R>();
                let target_id = archetypes.component_id::<T>();
                archetypes
                    .add_data_relationship_typed(self.0, relation_id, target_id, value)
                    .unwrap();
            });
        }
        self.clone()
    }

    pub fn rel_first_ret<R: AbstractComponent, T: AbstractComponent, U>(
        &self,
        f: impl FnOnce(&R) -> U,
    ) -> U {
        assert!(std::mem::size_of::<R>() > 0);
        assert!(std::mem::size_of::<T>() == 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes
                .get_component(relationship, self.0)
                .expect_fn(|err| panic!("{err}"))
                .get(|c| f(c))
        })
    }

    pub fn rel_first_mut_ret<R: AbstractComponent, T: AbstractComponent, U>(
        &mut self,
        f: impl FnOnce(&mut R) -> U,
    ) -> U {
        assert!(std::mem::size_of::<R>() > 0);
        assert!(std::mem::size_of::<T>() == 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes
                .get_component(relationship, self.0)
                .expect_fn(|err| panic!("{err}"))
                .get_mut(|c| f(c))
        })
    }
    pub fn rel_first<R: AbstractComponent, T: AbstractComponent>(
        &self,
        f: impl FnOnce(&R),
    ) -> &Self {
        assert!(std::mem::size_of::<R>() > 0);
        assert!(std::mem::size_of::<T>() == 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes
                .get_component(relationship, self.0)
                .expect_fn(|err| panic!("{err}"))
                .get(|c| f(c));
        });
        self
    }

    pub fn rel_first_mut<R: AbstractComponent, T: AbstractComponent>(
        &mut self,
        f: impl FnOnce(&mut R),
    ) -> &mut Self {
        assert!(std::mem::size_of::<R>() > 0);
        assert!(std::mem::size_of::<T>() == 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes
                .get_component(relationship, self.0)
                .expect_fn(|err| panic!("{err}"))
                .get_mut(|c| f(c));
        });
        self
    }

    pub fn has_mixed_rel<R: AbstractComponent>(&self, target: &Entity) -> bool {
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<R>();
            let relationship = Archetypes::relationship_id(relation_id, target.0);
            archetypes.has_component(relationship, self.0)
        })
    }

    pub fn add_mixed_rel<R: AbstractComponent>(&mut self, target: &Entity, value: R) -> Self {
        assert!(std::mem::size_of::<R>() > 0);
        {
            archetypes_mut(|archetypes| {
                let relation_id = archetypes.component_id::<R>();
                archetypes
                    .add_data_relationship_typed(self.0, relation_id, target.0, value)
                    .unwrap();
            });
        }
        self.clone()
    }

    pub fn add_mixed_tag_rel<R: AbstractComponent>(&self, target: &Entity) -> Self {
        assert!(std::mem::size_of::<R>() == 0);
        {
            archetypes_mut(|archetypes| {
                let relation_id = archetypes.component_id::<R>();
                archetypes
                    .add_relationship(self.0, relation_id, target.0, TableReusage::Reuse)
                    .unwrap();
            });
        }
        self.clone()
    }

    pub fn mixed_rel_ret<R: AbstractComponent, U>(
        &self,
        target: &Entity,
        f: impl FnOnce(&R) -> U,
    ) -> U {
        assert!(std::mem::size_of::<R>() > 0);
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<R>();
            let relationship = Archetypes::relationship_id(relation_id, target.0);
            archetypes
                .get_component(relationship, self.0)
                .expect_fn(|err| panic!("{err}"))
                .get(|c| f(c))
        })
    }

    pub fn mixed_rel_mut_ret<R: AbstractComponent, U>(
        &mut self,
        target: &Entity,
        f: impl FnOnce(&mut R) -> U,
    ) -> U {
        assert!(std::mem::size_of::<R>() > 0);
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<R>();
            let relationship = Archetypes::relationship_id(relation_id, target.0);
            archetypes
                .get_component(relationship, self.0)
                .expect_fn(|err| panic!("{err}"))
                .get_mut(|c| f(c))
        })
    }
    pub fn mixed_rel<R: AbstractComponent>(&self, target: &Entity, f: impl FnOnce(&R)) -> &Self {
        assert!(std::mem::size_of::<R>() > 0);
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<R>();
            let relationship = Archetypes::relationship_id(relation_id, target.0);
            archetypes
                .get_component(relationship, self.0)
                .expect_fn(|err| panic!("{err}"))
                .get(|c| f(c));
        });
        self
    }

    pub fn mixed_rel_mut<R: AbstractComponent>(
        &mut self,
        target: &Entity,
        f: impl FnOnce(&mut R),
    ) -> &mut Self {
        assert!(std::mem::size_of::<R>() > 0);
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<R>();
            let relationship = Archetypes::relationship_id(relation_id, target.0);
            archetypes
                .get_component(relationship, self.0)
                .expect_fn(|err| panic!("{err}"))
                .get_mut(|c| f(c));
        });
        self
    }

    pub fn remove_mixed_rel<R: AbstractComponent>(&self, target: &Entity) -> Self {
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<R>();
            let relationship = Archetypes::relationship_id(relation_id, target.0);
            let table_reusage = if archetypes.is_component_empty(relationship) {
                TableReusage::Reuse
            } else {
                TableReusage::New
            };
            let _ = archetypes.remove_component(relationship, self.0, table_reusage);
        });

        self.clone()
    }

    pub fn add_tag<T: AbstractComponent>(&mut self) -> Entity {
        assert!(std::mem::size_of::<T>() == 0);
        archetypes_mut(|archetypes| {
            let tag = archetypes.component_id::<T>();
            archetypes.add_component_tag(self.0, tag).unwrap();
        });
        self.clone()
    }

    pub fn has_tag<T: AbstractComponent>(&self) -> bool {
        assert!(std::mem::size_of::<T>() == 0);
        archetypes_mut(|archetypes| {
            let tag = archetypes.component_id::<T>();
            archetypes.has_component(tag, self.0)
        })
    }

    pub fn add_ent_tag(&mut self, tag: &Entity) -> Entity {
        archetypes_mut(|archetypes| {
            archetypes.add_entity_tag(self.0, tag.0).unwrap();
        });
        self.clone()
    }

    pub fn remove_ent_tag(&self, tag: &Entity) -> Entity {
        archetypes_mut(|archetypes| {
            let _ = archetypes.remove_component(tag.0, self.0, TableReusage::Reuse);
        });
        self.clone()
    }

    pub fn remove_ent_rel(&self, relation: &Entity, target: &Entity) -> Self {
        archetypes_mut(|archetypes| {
            let relationship = Archetypes::relationship_id(relation.0, target.0);
            let _ = archetypes.remove_component(relationship, self.0, TableReusage::Reuse);
        });
        self.clone()
    }

    pub fn has_ent_rel(&self, relation: &Entity, target: &Entity) -> bool {
        archetypes_mut(|archetypes| {
            let relationship = Archetypes::relationship_id(relation.0, target.0);
            archetypes.has_component(relationship, self.0)
        })
    }

    pub fn add_ent_rel(&mut self, relation: &Entity, target: &Entity) -> Self {
        archetypes_mut(|archetypes| {
            archetypes
                .add_relationship(self.0, relation.0, target.0, TableReusage::Reuse)
                .unwrap();
        });
        self.clone()
    }

    pub fn add_rel<R: AbstractComponent, T: AbstractComponent>(&mut self) -> Self {
        assert!(std::mem::size_of::<T>() == 0);
        assert!(std::mem::size_of::<R>() == 0);
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<R>();
            let target_id = archetypes.component_id::<T>();
            archetypes
                .add_relationship(self.0, relation_id, target_id, TableReusage::Reuse)
                .unwrap();
        });
        self.clone()
    }

    pub fn remove_rel<R: AbstractComponent, T: AbstractComponent>(&self) -> Self {
        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            let table_reusage = if archetypes.is_component_empty(relationship) {
                TableReusage::Reuse
            } else {
                TableReusage::New
            };
            archetypes
                .remove_component(relationship, self.0, table_reusage)
                .unwrap();
        });
        self.clone()
    }

    ///Clones all of entities' components.
    pub fn deep_clone(&self) -> Entity {
        archetypes_mut(|archetypes| Self::new(archetypes.clone_entity(self.0).unwrap()))
    }

    pub fn has_rel<R: AbstractComponent, T: AbstractComponent>(&self) -> bool {
        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes.has_component(relationship, self.0)
        })
    }

    pub fn has_ent_tag(&self, tag: &Entity) -> bool {
        archetypes_mut(|archetypes| archetypes.has_component(tag.0, self.0))
    }

    pub fn get_comp<T: AbstractComponent>(
        &self,
        f: impl FnOnce(Result<&T, GetComponentError>),
    ) -> Entity {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes.get_component(id, self.0).try_get(f)
        });
        self.clone()
    }

    pub fn get_comp_ret<T: AbstractComponent, U>(
        &self,
        f: impl FnOnce(Result<&T, GetComponentError>) -> U,
    ) -> U {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes.get_component(id, self.0).try_get(f)
        })
    }

    pub fn get_comp_mut_ret<T: AbstractComponent, U>(
        &self,
        f: impl FnOnce(Result<&mut T, GetComponentError>) -> U,
    ) -> U {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes.get_component(id, self.0).try_get_mut(f)
        })
    }

    pub fn get_comp_mut<T: AbstractComponent>(
        &self,
        f: impl FnOnce(Result<&mut T, GetComponentError>),
    ) -> Entity {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes.get_component(id, self.0).try_get_mut(f)
        });
        self.clone()
    }

    pub fn comp_mut<T: AbstractComponent>(&self, f: impl FnOnce(&mut T)) -> Entity {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes
                .get_component(id, self.0)
                .expect_fn(|err| panic!("{}", err))
                .get_mut(f);
        });
        self.clone()
    }

    pub fn comp<T: AbstractComponent>(&self, f: impl FnOnce(&T)) -> Entity {
        assert!(std::mem::size_of::<T>() > 0);
        let component = archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes.get_component(id, self.0)
        });
        component.expect_fn(|err| panic!("{}", err)).get(f);
        self.clone()
    }

    pub fn comp_ret<T: AbstractComponent, U>(&self, f: impl FnOnce(&T) -> U) -> U {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes
                .get_component(id, self.0)
                .expect_fn(|err| panic!("{}", err))
                .get(f)
        })
    }

    pub fn comp_mut_ret<T: AbstractComponent, U>(&self, f: impl FnOnce(&mut T) -> U) -> U {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes
                .get_component(id, self.0)
                .expect_fn(|err| panic!("{}", err))
                .get_mut(f)
        })
    }

    pub fn comps<T: ComponentQuery>(&self, f: impl FnOnce(T::Item<'_>)) -> Entity {
        f(T::fetch(self));
        self.clone()
    }

    pub fn comps_ret<T: ComponentQuery, R>(&self, f: impl FnOnce(T::Item<'_>) -> R) -> R {
        f(T::fetch(self))
    }

    pub fn has_children(&self) -> bool {
        !QueryState::<(), ()>::new()
            .with_children_of(self.clone())
            .build()
            .is_empty()
    }

    pub fn remove(self) {
        archetypes_mut(|archetypes| {
            let pool = archetypes.entities_pool_rc().clone();
            let pool: &mut _ = &mut pool.borrow_mut();
            archetypes.remove_entity(self.0, 0.into(), pool).unwrap();
        })
    }

    pub fn is_active(&self) -> bool {
        archetypes_mut(|archetypes| {
            let record: &Option<EntityRecord> = &archetypes.record_mut(self.0);
            match record {
                Some(record) => record.entity.is_active(),
                None => false,
            }
        })
    }

    pub fn diactivate(&self) -> Entity {
        self.set_active_recursive(false);
        self.clone()
    }

    pub fn toggle_active(&self) -> Entity {
        let is_active =
            archetypes_mut(|archetypes| archetypes.record_mut(self.0).unwrap().entity.is_active());
        self.set_active_recursive(!is_active);
        self.clone()
    }

    pub fn activate(&self) -> Entity {
        self.set_active_recursive(true);
        self.clone()
    }

    fn set_active_recursive(&self, is_active: bool) -> Entity {
        archetypes_mut(|archetypes| {
            let mut record = archetypes.record_mut(self.0);
            let record = record.as_mut().unwrap();
            record.entity.set_is_active(is_active);
        });
        for (child, _) in self.children_recursive() {
            archetypes_mut(|archetypes| {
                let mut record = archetypes.record_mut(child.0);
                let record = record.as_mut().unwrap();
                record.entity.set_is_active(is_active);
            });
        }
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::hash::{DefaultHasher, Hasher};

    use archetypes::{Wildcard, ENTITY_ID};
    use bevy_reflect::{DynamicStruct, FromReflect, Reflect, Struct};
    use bevy_utils::hashbrown::HashMap;
    use macro_rules_attribute::apply;
    use regex::Regex;
    use serde_json::json;

    use crate::components::test_components::{
        Apples, Begin, End, IsCool, Likes, Name, Oranges, Owes, Position, Velocity,
    };
    use crate::locals::SystemLocals;
    use crate::plugin::Plugin;
    use crate::systems::{
        IntoSystem, IntoSystems, MyFunctionSystem, States, System, SystemWithState,
    };
    use crate::{impl_system_states, world, ComponentBundle, EnumTag};
    use crate::{
        query::QueryComoponentId,
        query_structs::{Not, With, WithRelation},
        systems::SystemStage,
        world::World,
    };

    #[test]
    pub fn plugins() {
        let mut world = World::new();

        struct TestPlugin;
        impl Plugin for TestPlugin {
            fn build(&self, world: &World) {
                world.add_systems(
                    |_: &World| {
                        println!("Im a system!");
                    },
                    SystemStage::Update,
                );
            }
        }
        struct TestPlugin1;
        impl Plugin for TestPlugin1 {
            fn build(&self, _world: &World) {}
        }

        world.add_plugins((TestPlugin, TestPlugin1));
        world.run(&egui::Context::default());
    }

    #[test]
    pub fn comps() {
        let world = World::new();
        world.register_components::<(Position, Velocity)>();
        let _ = world
            .add_entity()
            .add_comp(Position::new(1, 2))
            .add_comp(Velocity::new(3, 4))
            .comps::<(&Position, &Velocity)>(|(pos, vel)| {
                assert_eq!(pos.x, 1);
                assert_eq!(pos.y, 2);
                assert_eq!(vel.x, 3);
                assert_eq!(vel.y, 4);
            });
    }

    #[test]
    pub fn events() {
        let mut world = World::new();
        #[derive(Debug)]
        struct MyEvent {
            value: i32,
        }

        impl MyEvent {
            fn new(value: i32) -> Self {
                Self { value }
            }
        }

        fn read_event_before(world: &World) {
            for event in world.event_reader::<MyEvent>().borrow().read() {
                println!("reading before");
                dbg!(event);
            }
            for event in world.event_reader::<MyEvent>().borrow().read() {
                dbg!(event);
            }
        }
        fn read_event_after(world: &World) {
            for event in world.event_reader::<MyEvent>().borrow().read() {
                println!("reading after");
                dbg!(event);
            }
        }
        fn send_event(world: &World) {
            world.send_event(MyEvent::new(15));
        }
        //TODO: add global event reader
        world.add_event_type::<MyEvent>().add_systems(
            (read_event_before, send_event, read_event_after),
            SystemStage::Update,
        );
        let context = egui::Context::default();
        world.run(&context);
        world.run(&context);
    }

    #[test]
    pub fn resources() {
        #[derive(Debug)]
        struct ResourceOne {
            value: i32,
        }
        #[derive(Debug)]
        struct ResourceTwo {
            value: String,
        }

        let world = World::new();
        world.add_resource(ResourceOne { value: 10 });
        world.add_resource(ResourceTwo {
            value: String::from("hello"),
        });
        world.resources::<(&ResourceOne, &mut ResourceTwo)>(|(r1, r2)| {});
    }
    #[test]
    pub fn ecs_ub_test() {
        Component! {
            #[derive(Default)]
            pub struct Position(i32, i32, u16);
        }
        impl Position {
            pub fn new(x: i32, y: i32) -> Self {
                Self { 0: x, 1: y, 2: 15 }
            }
        }
        impl Velocity {
            pub fn new(x: i32, y: i32) -> Self {
                Self { 0: x, 1: y }
            }
        }
        Component! {
            #[derive(Default)]
            pub struct Velocity(i32, i32);
        }
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        world.add_entity().add_comp(Position::default());
        world
            .add_entity()
            .add_comp(Position::default())
            .add_comp(Velocity::new(1, 2));
        let sprites = world.query::<(&Velocity, &Position)>();
        let mut sprites = sprites.build();

        for (vel, pos) in sprites.iter() {
            dbg!(vel, pos);
        }
    }
    #[test]
    fn without_children_query() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let parent = world.add_entity();
        world
            .add_entity()
            .add_comp(Position::new(1, 2))
            .add_child_of(&parent);
        world.add_entity().add_comp(Position::new(3, 4));

        let mut query_all = world.query::<&Position>().build();
        let mut query_without_children = world
            .query::<&Position>()
            .without_rel::<ChildOf, Wildcard>()
            .build();

        assert_eq!(query_all.iter().count(), 2);
        assert_eq!(query_without_children.iter().count(), 1);
    }

    #[test]
    fn bundle_struct_test() {
        let world = World::new();
        world
            .register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin, Name)>(
            );
        ComponentBundle! {
            struct Extra {
                name: Name,
            }
        }
        ComponentBundle! {
            struct GameObject {
                pos: Position,
                vel: Velocity,
                extra: Extra,
            }
        }

        let e = world.add_entity().add_comp(GameObject {
            pos: Position::new(1, 2),
            vel: Velocity::new(1, 2),
            extra: Extra {
                name: Name {
                    value: "hey!".to_owned(),
                },
            },
        });

        assert!(e.has_comp::<Velocity>());
        assert!(e.has_comp::<Name>());
    }

    #[test]
    fn bundle_test() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let e = world
            .add_entity()
            .add_comp((Position::new(1, 2), Velocity::new(3, 4)));

        let pos = e.comp_ret(|p: &Position| *p);
        let vel = e.comp_ret(|v: &Velocity| *v);

        assert_eq!(pos.x + pos.y + vel.x + vel.y, 10);
    }

    #[test]
    fn wildcard_data_query() {
        return;
        let world = World::new();

        world
            .add_entity()
            .add_rel_second::<Begin, _>(Position { x: 1, y: 2 });
        world
            .add_entity()
            .add_rel_second::<End, _>(Position { x: 3, y: 4 });

        let sum: i32 = world
            .query::<&Position>()
            .term_relation::<Wildcard>(0)
            .build()
            .iter()
            .map(|p| p.x + p.y)
            .sum();

        // assert_eq!(sum, 3);
    }

    #[test]
    fn get_or_add_comp() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();

        let mut e = world.add_entity();
        e.get_or_add_comp_mut(
            || Position::new(1, 2),
            |pos| {
                assert_eq!(pos.x + pos.y, 3);
                pos.x += 1;
            },
        );
        e.get_or_add_comp(
            || Position::new(1, 2),
            |p| {
                assert_eq!(p.x, 2);
            },
        );
    }

    #[test]
    fn on_component_add() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();

        world.on_comp_add::<Position>(|entity: Entity, _| {
            entity.comp_mut::<Position>(|p| p.x += 1);
        });

        let e = world
            .add_entity()
            .add_comp::<Position>(Position { x: 0, y: 0 });

        e.comp::<Position>(|p| {
            assert_eq!(p.x, 1);
        });
    }

    #[test]
    fn adding_components_inside_query() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let e1 = world.add_entity_named("e1").add_tag::<IsCool>();
        let e2 = world
            .add_entity_named("e2")
            .add_tag::<IsCool>()
            .add_comp::<Position>(Position { x: 1, y: 2 });

        let mut count = 0;
        for mut e in world
            .query_filtered::<&Entity, With<IsCool>>()
            .build()
            .iter()
        {
            e.add_comp(Position { x: 10, y: 15 });
            e.add_comp(Position { x: 11, y: 16 });
            e.add_comp(Velocity { x: 1, y: 5 });
            e.remove_tag::<IsCool>();
            count += 1;
        }
        assert_eq!(count, 2);

        for e in [e1, e2].iter() {
            assert!(e.has_comp::<Position>());
            assert!(!e.has_tag::<IsCool>());
            assert!(e.has_comp::<Velocity>());
            e.comp::<Position>(|p| {
                assert_eq!(p.x, 11);
                assert_eq!(p.y, 16);
            });
            e.comp::<Velocity>(|p| {
                assert_eq!(p.x, 1);
                assert_eq!(p.y, 5);
            });
        }
    }

    #[test]
    fn querying_empty_entities() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let e = world.add_entity();
        world.add_entity().add_comp(Position { x: 1, y: 2 });
        let mut query = world
            .query::<&Entity>()
            .with_ent_tag(Entity::new(ENTITY_ID))
            .build();
        assert_eq!(1, query.iter().count());

        query.iter().for_each(|e| e.remove());

        assert!(!e.is_alive());
    }

    #[test]
    fn replacing_components() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let mut e1 = world.add_entity().add_comp(Position { x: 1, y: 1 });
        world.add_entity().add_comp(Position { x: 2, y: 2 });

        e1.add_comp(Position { x: 3, y: 3 });

        e1.comp::<Position>(|p| {
            assert_eq!(p.x, 3);
            assert_eq!(p.y, 3);
        });
    }

    #[test]
    fn replacing_relationships() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let mut e1 = world
            .add_entity()
            .add_rel_second::<Begin, _>(Position { x: 1, y: 1 });
        world
            .add_entity()
            .add_rel_second::<Begin, _>(Position { x: 2, y: 2 });

        e1.add_rel_second::<Begin, _>(Position { x: 3, y: 3 });

        e1.rel_second::<Begin, Position>(|p| {
            assert_eq!(p.x, 3);
            assert_eq!(p.y, 3);
        });
    }

    #[test]
    fn removing_children() {
        let world = World::new();
        let e = world.add_entity();
        let child = world.add_entity().add_child_of(&e);
        let grand_child = world.add_entity().add_child_of(&child);
        for child in e.children().iter() {
            child.remove();
        }

        assert!(!child.is_alive());
        assert!(!grand_child.is_alive());
        let child = world.add_entity_named("child").add_child_of(&e);
        child.remove_child_of(&e);

        child.name().get(|n| {
            assert_eq!(n, "child");
        })
    }

    #[test]
    fn recursive_deactivation() {
        let world = World::new();
        let e = world.add_entity();
        let child = world.add_entity().add_child_of(&e);

        e.diactivate();
        assert!(!child.is_active());
    }
    #[test]
    fn entity_tags() {
        let world = World::new();
        let mut e = world.add_entity_named("e");
        let tag = world.add_entity_named("tag");
        e.add_ent_tag(&tag);
        assert!(e.has_ent_tag(&tag));

        tag.remove();
    }
    #[test]
    fn colliding_names() {
        let world = World::new();
        let e = world.add_entity();
        let child_bob = world.add_entity().add_child_of(&e).set_name("Bob");
        let another_bob = world.add_entity().set_name("Bob");

        child_bob.name().get(|n| assert_eq!("Bob", n));
        another_bob.name().get(|n| assert_eq!("Bob", n));
    }
    #[test]
    fn names() {
        let world = World::new();
        let e = world.add_entity().set_name("Bob");
        let e1 = world.add_entity();
        e.name().get(|n| assert_eq!(n, "Bob"));
        assert!(!e1.has_name());
    }

    #[test]
    fn enum_components() {
        EnumTag! {
            #[derive(Eq, PartialEq)]
            enum PlayerState {
                Walking,
                Falling,
            }
        }

        let world = World::new();
        world.register_components::<(PlayerState, Progress)>();
        let mut e = world.add_entity().add_enum_tag(PlayerState::Walking);

        assert!(e.has_any_enum_tag::<PlayerState>());
        assert!(e.has_enum_tag(PlayerState::Walking));
        assert!(!e.has_enum_tag(PlayerState::Falling));
        assert_eq!(e.enum_tag::<PlayerState>(), PlayerState::Walking);

        e.add_enum_tag(PlayerState::Falling);
        assert!(e.has_enum_tag(PlayerState::Falling));
        assert_eq!(e.enum_tag::<PlayerState>(), PlayerState::Falling);

        EnumTag! {
            #[derive(Eq, PartialEq)]
            enum Progress {
                Beginner,
                Pro,
            }
        }

        e.add_enum_tag(Progress::Pro);

        let mut query = world
            .query::<()>()
            .with_enum_tag(PlayerState::Falling)
            .with_enum_tag(Progress::Pro)
            .build();
        let count = query.iter().count();
        assert_eq!(count, 1);

        e.remove_enum_tag::<PlayerState>();
        assert!(!e.has_enum_tag(PlayerState::Falling));
        assert!(!e.has_enum_tag(PlayerState::Falling));
        assert!(!e.has_any_enum_tag::<PlayerState>());
    }

    #[test]
    fn systems() {
        let mut world = World::new();

        fn update_positions_system(world: &World) {
            println!("updating positions");
            //...
        }

        fn display_game_menu_system(world: &World) {
            println!("displaying menu");
            //...
        }

        fn update_world_active(world: &World) {
            println!("updating active world...");
            world.set_state(WorldState::Inactive);
        }

        struct CustomSystem {
            value: i32,
        }
        impl IntoSystems<World> for CustomSystem {
            type System = Self;

            fn into_systems(self) -> crate::systems::SystemWithState<Self::System> {
                SystemWithState {
                    system: self,
                    should_run: None,
                    states: HashMap::new(),
                }
            }
        }
        impl System for CustomSystem {
            fn run(&mut self, _world: &World, _egui: &egui::Context) {
                self.value += 1;
            }
        }
        enum GameState {
            InMainMenu,
            InGame,
        }

        enum WorldState {
            Active,
            Inactive,
        }

        impl_system_states!(GameState, WorldState);

        world
            .set_state(GameState::InGame)
            .set_state(WorldState::Active)
            .add_systems(
                display_game_menu_system.should_run(|w: &World| true),
                SystemStage::First,
            )
            .add_systems(
                update_positions_system.with_state(GameState::InGame),
                SystemStage::Update,
            )
            .add_systems(
                update_world_active.with_state((GameState::InGame, WorldState::Active)),
                SystemStage::Update,
            )
            .add_systems(CustomSystem { value: 10 }, SystemStage::First);
        world.run(&egui::Context::default());
    }

    #[test]
    fn children() {
        let world = World::new();
        let e = world.add_entity_named("e");
        assert!(!e.has_children());

        let child1 = world.add_entity().add_child_of(&e).set_name("child1");
        //TODO: figure out why this fails
        //that's because I dont update query archetypes
        assert!(e.has_children());
        let child2 = world.add_entity().add_child_of(&e).set_name("child2");
        let mut grand_child = world.add_entity();
        let mut grand_grand_child = world.add_entity();
        grand_child.add_child_of(&child1).set_name("grand child");
        grand_grand_child
            .add_child_of(&grand_child)
            .set_name("grand-grand child");

        assert!(child1.is_child_of(&e));
        assert!(child2.is_child_of(&e));

        let mut count = 0;
        for _ in e.children_recursive() {
            count += 1;
        }
        assert_eq!(count, 4);
        archetypes(|a| a.debug_print_entities());
        println!("---------------");
        e.remove();
        archetypes(|a| a.debug_print_entities());
        assert!(!grand_child.is_alive());
        assert!(!grand_grand_child.is_alive());
        assert!(!child1.is_alive());
        assert!(!child2.is_alive());
    }
    #[test]
    fn find_relationships() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let e = world
            .add_entity()
            .add_rel::<Likes, Apples>()
            .add_rel_second::<Likes, _>(Position { x: 1, y: 2 });

        let count = e.find_rels::<Likes, Wildcard>().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn iter_relationships() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let likes = world.add_entity();
        let apples = world.add_entity();
        let parent = world.add_entity();
        let e = world
            .add_entity()
            .add_rel::<Likes, Apples>()
            .add_ent_rel(&likes, &apples)
            .add_child_of(&parent)
            .add_comp(Velocity { x: 10, y: 10 })
            .add_rel_second::<Begin, _>(Velocity { x: 10, y: 10 });
        let mut count = 0;
        for rel in e.iter_rels() {
            count += 1;
            assert!(e.has_relationship(rel));
        }
        assert_eq!(count, 4);
    }

    #[test]
    fn tag_only_query() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let e1 = world.add_entity().add_tag::<IsCool>();
        let e2 = world.add_entity().add_tag::<IsCool>();

        let count = world
            .query_filtered::<(), With<IsCool>>()
            .build()
            .iter()
            .count();
        assert_eq!(count, 2);
    }
    #[test]
    fn active_entities() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let e1 = world.add_entity().add_tag::<IsCool>();
        let e2 = world.add_entity().add_tag::<IsCool>();
        let e3 = world.add_entity().add_tag::<IsCool>().diactivate();
        //When I change something about an entity, I also have to change it in table and archetype
        assert!(e1.is_active());
        assert!(e2.is_active());
        assert!(!e3.is_active());

        let count = world
            .query_filtered::<(), With<IsCool>>()
            .build()
            .iter()
            .count();
        assert_eq!(count, 2);

        e3.toggle_active();

        let count = world
            .query_filtered::<(), With<IsCool>>()
            .build()
            .iter()
            .count();
        assert_eq!(count, 3);
    }
    #[test]
    fn adding_tags_to_empty_entity() {
        let world = World::new();
        let tag = world.add_entity();
        let mut entity = world.add_entity();
        entity.add_ent_tag(&tag);
    }

    use super::*;
    #[test]
    fn query_that_has_it_all() {
        //prepare thyself
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let bob = world.add_entity();
        let hates = world.add_entity();
        let idkwhat = world.add_entity();
        let entity_tag = world.add_entity();
        let mut child = world.add_entity();
        let god_entity = world
            .add_entity()
            .add_comp(Position { x: 10, y: 20 })
            .add_comp(Position::new(3, 4))
            .add_tag::<IsCool>()
            .add_ent_tag(&entity_tag)
            .add_ent_rel(&hates, &bob)
            .add_rel::<Likes, Apples>()
            .add_rel_first::<Owes, Apples>(Owes { amount: 10 })
            .add_rel_second::<Begin, Position>(Position::new(1, 2))
            .add_mixed_rel(&idkwhat, Position::new(5, 6));

        child.add_child_of(&god_entity);

        // let query = world.query_filtered()
    }

    #[test]
    fn data_relation_query() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let e1 = world
            .add_entity()
            .add_rel_first::<Owes, Apples>(Owes { amount: 10 })
            .add_rel_second::<Begin, Position>(Position { x: 1, y: 2 });

        let sum: i32 = world
            .query::<(&Owes, &Position)>()
            .set_target::<Apples>(QueryComoponentId(0))
            .set_relation::<Begin>(QueryComoponentId(1))
            .build()
            .iter()
            .map(|(owes, pos)| owes.amount + pos.x + pos.y)
            .sum();
        assert_eq!(sum, 13);
    }

    #[test]
    fn not_queries() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let mut e1 = world.add_entity().add_comp(Position { x: 1, y: 2 });
        e1.add_comp(Velocity { x: 1, y: 0 })
            .add_tag::<IsCool>()
            .add_tag::<Apples>();
        world.add_entity().add_comp(Position { x: 3, y: 4 });

        let mut query = world
            .query_filtered::<&Position, Not<(With<IsCool>, With<Apples>)>>()
            .build();

        let sum: i32 = query.iter().map(|p| p.x + p.y).sum();
        assert_eq!(sum, 7);
    }

    #[test]
    fn filtered_queries() {
        let world = World::new();
        world.register_components::<(Position, IsCool, Velocity, Likes, Apples)>();
        let mut e1 = world.add_entity().add_comp(Position { x: 1, y: 2 });
        e1.add_comp(Velocity { x: 1, y: 0 }).add_tag::<IsCool>();
        world
            .add_entity()
            .add_comp(Position { x: 3, y: 4 })
            .add_rel::<Likes, Apples>();

        let mut query = world
            .query_filtered::<&Position, WithRelation<Wildcard, Apples>>()
            .build();

        let sum: i32 = query.iter().map(|p| p.x + p.y).sum();
        assert_eq!(sum, 7);
    }
    #[test]
    fn queries() {
        let world = World::new();
        world.register_components::<(Position, Velocity)>();
        let mut e1 = world.add_entity().add_comp(Position { x: 1, y: 2 });
        e1.add_comp(Velocity { x: 1, y: 0 });
        let e2 = world.add_entity().add_comp(Position { x: 3, y: 4 });

        let mut query_ref = world.query::<&Position>().build();
        let mut counter = 0;
        for pos in query_ref.iter() {
            counter += pos.x + pos.y;
        }
        assert_eq!(counter, 10);

        let mut query_mut = world.query::<(&mut Position, &Entity)>().build();
        for (mut pos, _) in query_mut.iter() {
            pos.x = 10;
            pos.y = 1;
        }

        let pos_sum_1 = e1.comp_ret(|p: &Position| p.x + p.y);
        let pos_sum_2 = e2.comp_ret(|p: &Position| p.x + p.y);
        assert_eq!(pos_sum_1 + pos_sum_2, 22);

        let combined_query = world.query::<(&Position, &Velocity)>();
        let sum: i32 = combined_query
            .build()
            .iter()
            .map(|(pos, vel)| vel.x + vel.y + pos.x + pos.y)
            .sum();
        assert_eq!(sum, 12);
        let mut count = 0;
        let mut sum = 0;
        let mut optional_query = world.query::<(&Position, Option<&Velocity>)>().build();
        for (pos, vel) in optional_query.iter() {
            count += 1;
            let vel = vel.map(|v| *v).unwrap_or(Velocity { x: 0, y: 0 });
            sum += pos.x + pos.y + vel.x + vel.y;
        }
        assert_eq!(count, 2);
        assert_eq!(sum, 23);
    }

    #[test]
    fn prefab() {
        let world = World::new();
        world.register_components::<(Velocity, Likes, Oranges)>();
        let prefab = world
            .add_prefab()
            .add_comp(Velocity { x: 10, y: 20 })
            .add_rel::<Likes, Oranges>();

        let instance = world.add_instance_of(&prefab);
        assert!(instance.has_mixed_rel::<InstanceOf>(&prefab));
        instance.comp::<Velocity>(|p| {
            assert_eq!(p.x, 10);
            assert_eq!(p.y, 20);
        });
        assert!(instance.has_rel::<Likes, Oranges>());
    }

    #[test]
    fn everything_at_once_cloned() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let bob = world.add_entity();
        let hates = world.add_entity();
        let idkwhat = world.add_entity();
        let entity_tag = world.add_entity();
        let entity = world
            .add_entity()
            .add_comp(Position { x: 10, y: 20 })
            .add_comp(Position::new(3, 4))
            .add_tag::<IsCool>()
            .add_ent_tag(&entity_tag)
            .add_ent_rel(&hates, &bob)
            .add_rel::<Likes, Apples>()
            .add_rel_first::<Owes, Apples>(Owes { amount: 10 })
            .add_rel_second::<Begin, Position>(Position::new(1, 2))
            .add_mixed_rel(&idkwhat, Position::new(5, 6));

        let entity = entity.deep_clone();

        assert!(entity.has_comp::<Position>());
        assert!(entity.has_tag::<IsCool>());
        assert!(entity.has_ent_tag(&entity_tag));
        assert!(entity.has_rel::<Likes, Apples>());
        assert!(entity.has_rel::<Owes, Apples>());
        assert!(entity.has_rel::<Begin, Position>());
        assert!(entity.has_ent_rel(&hates, &bob));
        assert!(entity.has_mixed_rel::<Position>(&idkwhat));

        entity.comp::<Position>(|pos| {
            assert_eq!(pos.x, 3);
            assert_eq!(pos.y, 4);
        });
        // entity.rel_first::<Owes, Begin>().try_get(|stuff| {});
        entity.rel_first::<Owes, Apples>(|ows| {
            assert_eq!(ows.amount, 10);
        });
        entity.rel_second::<Begin, Position>(|pos| {
            assert_eq!(pos.x, 1);
            assert_eq!(pos.y, 2);
        });
        entity.mixed_rel::<Position>(&idkwhat, |pos| {
            assert_eq!(pos.x, 5);
            assert_eq!(pos.y, 6);
        });
        entity.remove_comp::<Position>();
        assert!(!entity.has_comp::<Position>());
        entity.remove_tag::<IsCool>();
        assert!(!entity.has_tag::<IsCool>());
        entity.remove_ent_tag(&entity_tag);
        assert!(!entity.has_ent_tag(&entity_tag));
        entity.remove_ent_rel(&hates, &bob);
        assert!(!entity.has_ent_rel(&hates, &bob));
        entity.remove_rel::<Likes, Apples>();
        assert!(!entity.has_rel::<Likes, Apples>());
        entity.remove_rel::<Owes, Apples>();
        assert!(!entity.has_rel::<Owes, Apples>());
        entity.remove_rel::<Begin, Position>();
        assert!(!entity.has_rel::<Begin, Position>());
        entity.remove_mixed_rel::<Position>(&idkwhat);
        assert!(!entity.has_mixed_rel::<Position>(&idkwhat));
        entity.remove_mixed_rel::<Likes>(&bob);
        assert!(!entity.has_mixed_rel::<Likes>(&bob));
    }

    #[test]
    fn wildcard() {
        let world = World::new();
        world.register_components::<(Likes, Apples)>();
        let mut entity = world.add_entity();
        entity.add_rel::<Likes, Apples>();
        assert!(entity.has_rel::<Likes, Wildcard>());
        assert!(entity.has_rel::<Wildcard, Apples>());

        entity.remove_rel::<Likes, Apples>();
        let is = world.add_entity();
        let helicopter = world.add_entity();

        entity.add_ent_rel(&is, &helicopter);
        assert!(entity.has_ent_rel(&WILDCARD, &helicopter));
        assert!(entity.has_ent_rel(&is, &WILDCARD));
    }
    #[test]
    fn everything_at_once() {
        let world = World::new();
        world.register_components::<(Velocity, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let bob = world.add_entity();
        let hates = world.add_entity();
        let idkwhat = world.add_entity();
        let entity_tag = world.add_entity();

        world
            .add_entity()
            .add_comp::<Velocity>(Velocity { x: 1, y: 2 });
        let mut entity = None;
        //also testing adding stuff inside a query
        for _ in world.empty_query().with_comp::<Velocity>().build().iter() {
            entity = Some(
                world
                    .add_entity()
                    .add_comp(Position::new(3, 4))
                    .add_tag::<IsCool>()
                    .add_ent_tag(&entity_tag)
                    .add_ent_rel(&hates, &bob)
                    .add_rel::<Likes, Apples>()
                    .add_rel_first::<Owes, Apples>(Owes { amount: 10 })
                    .add_rel_second::<Begin, Position>(Position::new(1, 2))
                    .add_mixed_rel(&idkwhat, Position::new(5, 6))
                    .add_mixed_tag_rel::<Likes>(&bob),
            )
        }

        let entity = entity.unwrap();

        assert!(entity.has_comp::<Position>());
        assert!(entity.has_tag::<IsCool>());
        assert!(entity.has_ent_tag(&entity_tag));
        assert!(entity.has_rel::<Likes, Apples>());
        assert!(entity.has_rel::<Owes, Apples>());
        assert!(entity.has_rel::<Begin, Position>());
        assert!(entity.has_ent_rel(&hates, &bob));
        assert!(entity.has_mixed_rel::<Position>(&idkwhat));
        assert!(entity.has_mixed_rel::<Likes>(&bob));

        entity.comp::<Position>(|pos| {
            assert_eq!(pos.x, 3);
            assert_eq!(pos.y, 4);
        });
        // entity.rel_first::<Owes, Begin>().try_get(|stuff| {});
        entity.rel_first::<Owes, Apples>(|ows| {
            assert_eq!(ows.amount, 10);
        });
        entity.rel_second::<Begin, Position>(|pos| {
            assert_eq!(pos.x, 1);
            assert_eq!(pos.y, 2);
        });
        entity.mixed_rel::<Position>(&idkwhat, |pos| {
            assert_eq!(pos.x, 5);
            assert_eq!(pos.y, 6);
        });
        entity.remove_comp::<Position>();
        assert!(!entity.has_comp::<Position>());
        entity.remove_tag::<IsCool>();
        assert!(!entity.has_tag::<IsCool>());
        entity.remove_ent_tag(&entity_tag);
        assert!(!entity.has_ent_tag(&entity_tag));
        entity.remove_ent_rel(&hates, &bob);
        assert!(!entity.has_ent_rel(&hates, &bob));
        entity.remove_rel::<Likes, Apples>();
        assert!(!entity.has_rel::<Likes, Apples>());
        entity.remove_rel::<Owes, Apples>();
        assert!(!entity.has_rel::<Owes, Apples>());
        entity.remove_rel::<Begin, Position>();
        assert!(!entity.has_rel::<Begin, Position>());
        entity.remove_mixed_rel::<Position>(&idkwhat);
        assert!(!entity.has_mixed_rel::<Position>(&idkwhat));
        entity.remove_mixed_rel::<Likes>(&bob);
        assert!(!entity.has_mixed_rel::<Position>(&idkwhat));
    }

    #[test]
    fn data_rels_first() {
        let world = World::new();
        world.register_components::<(Owes, Apples, Oranges)>();
        let mut ann = world.add_entity();
        ann.add_rel_first::<Owes, Apples>(Owes { amount: 10 });
        ann.add_rel_first::<Owes, Oranges>(Owes { amount: 10 });
        ann.remove_rel::<Owes, Apples>();
        assert!(!ann.has_rel::<Owes, Apples>());
        assert!(ann.has_rel::<Owes, Oranges>());
    }

    #[test]
    fn data_rels_second() {
        let world = World::new();
        world.register_components::<(Begin, Position, End, Apples)>();
        let mut entity = world.add_entity();
        entity
            .add_rel_second::<Begin, _>(Position::new(1, 2))
            .add_rel_second::<End, _>(Position::new(3, 4));
        let p1 = entity.rel_second_ret::<Begin, Position, _>(|c| *c);
        let p2 = entity.rel_second_ret::<End, Position, _>(|c| *c);

        assert!(entity.has_rel::<Begin, Position>());
        assert!(entity.has_rel::<End, Position>());
        assert!(!entity.has_rel::<Apples, Position>());

        assert_eq!(p1.x, 1);
        assert_eq!(p1.y, 2);
        assert_eq!(p2.x, 3);
        assert_eq!(p2.y, 4);
    }

    #[test]
    fn tag_rels() {
        let world = World::new();
        world.register_components::<(Likes, Apples, Position)>();
        let mut entity = world.add_entity();
        entity.add_rel::<Likes, Apples>();
        entity.add_comp(Position { x: 10, y: 20 });

        let pos = entity.comp_ret(|c: &Position| *c);
        assert_eq!(pos.x, 10);
        assert_eq!(pos.y, 20);

        assert!(entity.has_rel::<Likes, Apples>());
    }
    #[test]
    fn adding_comps_to_comps() {
        use macro_rules_attribute::apply;

        #[apply(Component)]
        struct Material {
            data: u32,
        }

        #[apply(Component)]
        #[derive(Default)]
        struct IsMaterial {}

        let world = World::new();
        world.register_components::<(IsMaterial, Position, Material)>();

        let mut material = world.comp_entity::<Material>();
        material.add_tag::<IsMaterial>();

        _ = world
            .add_entity()
            .add_comp(Material { data: 10 })
            .add_comp(Position::new(1, 2));

        let mut query = world.query::<(&Position, &Entity)>().build();
        for (pos, entity) in query.iter() {
            assert_eq!(pos.x, 1);
            assert_eq!(pos.y, 2);

            for (id, value) in entity.iter_comps_reflect() {
                if id.has_tag::<IsMaterial>() {
                    let material = value.unwrap().downcast_ref::<Material>().unwrap();
                    dbg!(material.data);
                }
            }
        }
    }

    #[test]
    fn bevy_reflection() {
        #[derive(Debug, Reflect)]
        struct SomeValues {
            string: String,
            int: i32,
        }

        let mut dynamic_struct = DynamicStruct::default();
        dynamic_struct.insert("string".to_string(), "A string!".to_string());
        dynamic_struct.insert("int".to_string(), 25);
        let some_values = SomeValues::from_reflect(&dynamic_struct);
        dbg!(some_values);
    }
    #[test]
    fn deserialization() {
        let json = json!(
        {
            "Name": "John",
            "($Owes, Apples)": {
                "amount": 10
            },
            "(Begin, $Position)": {
                "x": 2,
                "y": 3
            },
            "Position": {
                "x": 1,
                "y": 2
            },
            "Tags": [
                "#Enemy",
                "IsCool",
                "(Likes, #Enemy)",
                "(Likes, Apples)",
            ]
        });
        let world = World::new();
        world.register_components::<(Likes, Apples, IsCool, Position, Begin, Owes)>();
        let entity = world
            .deserialize_entity(&json.to_string())
            .map_err(|e| e.to_string())
            .unwrap();
        entity.comp::<Position>(|p| {
            assert_eq!(p.x, 1);
            assert_eq!(p.y, 2);
        });
        let enemy = world.entity_by_global_name("Enemy").unwrap();
        entity.rel_first::<Owes, Apples>(|owes| assert_eq!(owes.amount, 10));
        entity.rel_second::<Begin, Position>(|pos| assert_eq!(pos.x + pos.y, 5));
        entity.comp::<Position>(|pos| assert_eq!(pos.x + pos.y, 3));
        entity.name().get(|n| assert!(n == "John"));
        assert!(entity.has_tag::<IsCool>());
        assert!(entity.has_rel::<Likes, Apples>());
        assert!(entity.has_mixed_rel::<Likes>(&enemy));
        assert!(entity.has_ent_tag(&enemy));
    }
    #[test]
    fn serialization() {
        EnumTag! {
            #[derive(Eq, PartialEq)]
            enum MyEnumTag {
                StateOne
            }
        }

        let world = World::new();
        world.register_components::<(MyEnumTag, Position, IsCool, Likes, Apples, Owes, Begin)>();
        let enemy = world.add_entity_named("Enemy");
        let entity = world
            .add_entity_named("John")
            .add_rel_first::<_, Apples>(Owes { amount: 10 })
            .add_rel_second::<Begin, _>(Position::new(2, 3))
            .add_comp(Position::new(1, 2))
            .add_tag::<IsCool>()
            .add_ent_tag(&enemy)
            .add_rel::<Likes, Apples>()
            .add_mixed_tag_rel::<Likes>(&enemy)
            .serialize()
            .unwrap();
        println!("{entity}");
    }

    #[test]
    fn debug_name() {
        let world = World::new();
        let mut entity = world.add_entity_named("entity");
        assert_eq!("entity", entity.debug_name());
        let parent = world.add_entity();
        entity.add_child_of(&parent);
        assert_eq!("entity", entity.debug_name());
        entity.set_name("other name");
        assert_eq!("other name", entity.debug_name());
        entity.remove_child_of(&parent);
        assert_eq!("other name", entity.debug_name());
        entity.remove_name();
        dbg!(entity.debug_name());
    }
    #[test]
    fn reflect() {
        let world = World::new();
        world.register_components::<(Position, Velocity, Begin, Name)>();
        let e = world
            .add_entity()
            .add_comp(Position::new(1, 2))
            .add_comp(Velocity::new(3, 4))
            .add_comp(Name {
                value: "Hi".to_string(),
            });
        let pos_id = world.comp_entity::<Position>().0;
        let vel_id = world.comp_entity::<Velocity>().0;
        let name_id = world.comp_entity::<Name>().0;
        for (id, value) in e.iter_comps_reflect() {
            let value = value.unwrap();
            match id {
                _ if id.0 == pos_id => {
                    let pos = value.downcast_ref::<Position>().unwrap();
                    assert_eq!(pos.x, 1);
                    assert_eq!(pos.y, 2);
                }
                _ if id.0 == vel_id => {
                    let vel = value.downcast_ref::<Velocity>().unwrap();
                    assert_eq!(vel.x, 3);
                    assert_eq!(vel.y, 4);
                }
                _ if id.0 == name_id => {
                    let name = value.downcast_ref::<Name>().unwrap();
                    assert_eq!(&name.value, "Hi");
                }
                _ => {}
            }
        }
    }
    #[test]
    fn locals() {
        let mut world = World::new();
        fn locals_system(w: &World) {
            w.resources::<&mut SystemLocals>(|locals| {
                let (value, boolean) = locals.get_mut::<(&mut i32, &mut bool)>();
                println!("{} {}", value, boolean);
                *value += 1;
                *boolean = true;
            });
        }

        world.add_systems(locals_system, SystemStage::Init);
        world.run(&egui::Context::default());
        world.run(&egui::Context::default());
    }
    #[test]
    fn debug_tables() {
        let world = World::new();
        world.register_components::<(IsCool, Position, Velocity)>();
        world
            .add_entity()
            .add_comp(Position::new(10, 20))
            .add_comp(Velocity::new(1, 2))
            .add_tag::<IsCool>();

        for (i, (id, table_info)) in world.debug_tables_components_info().iter().enumerate() {
            println!("{}", id.0);
            println!("{table_info}");
        }
    }

    #[test]
    fn cloning_and_accessing_component() {
        let world = World::new();
        world.register_components::<(Position, Velocity, IsCool)>();
        let prefab = world
            .add_prefab_named("prefab")
            .add_comp(Position::new(1, 2))
            .add_comp(Velocity::new(3, 4));
        world
            .add_entity_named("john")
            .add_comp(Position::new(30, 40))
            .add_comp(Velocity::new(10, 20));

        let e1 = world
            .add_instance_of(&prefab)
            .comp_mut(|t: &mut Position| t.x = -1)
            .set_name("e1");
    }

    #[test]
    fn adding_rels_in_queries() {
        let world = World::new();
        world.register_components::<(Position, Velocity)>();

        let mut e1 = world
            .add_entity()
            .add_comp(Position::default())
            .set_name("e1");
        let mut e2 = world
            .add_entity()
            .add_comp(Position::default())
            .set_name("e2");
        archetypes(|a| {
            let e1_record = a.record(e1.0).unwrap();
            let e2_record = a.record(e2.0).unwrap();
        });
        e1.add_comp(Velocity::default());
        // for (id, info) in world.debug_tables_info() {
        //     println!("{info}");
        // }
        // archetypes(|a| {
        //     let e1_record = a.record(e1.0).unwrap();
        //     let e2_record = a.record(e2.0).unwrap();
        //
        // });
        // let mut s = format!("{:#?}", Position::new(1, 2));
        // let new_lines_indices: Vec<_> = s
        //     .chars()
        //     .enumerate()
        //     .filter(|(_, s)| *s == '\n')
        //     .enumerate()
        //     .map(|(j, (i, _))| (i + 1) + 8 * j)
        //     .collect();
        // for index in new_lines_indices {
        //     s.insert_str(index, "        ");
        // }
        // println!("        {s}");
        e2.add_comp(Velocity::default());
    }

    #[test]
    pub fn component_enum() {
        let world = World::new();
        #[apply(Component)]
        enum TestEnumComp {
            A(i32, i32),
            B { str: String },
        }
        world.register_components::<TestEnumComp>();
        let e = world.add_entity().add_comp(TestEnumComp::A(11, 12));
        e.comp(|test_enum: &TestEnumComp| {
            assert!(matches!(test_enum, TestEnumComp::A(_, _)));
            if let TestEnumComp::A(i1, i2) = test_enum {
                assert_eq!(*i1, 11);
                assert_eq!(*i2, 12);
            }
        });
    }
}

