pub use std::{fmt::Debug, hash::Hash, os::unix::process::parent_id};

use crate::{
    archetypes::{
        self, Archetypes, ComponentGetter, EntityNameGetter, EntityRecord, InstanceOf, NameLeft,
        TableReusage, Wildcard, WILDCARD_RELATIONSHIP,
    },
    children_iter::ChildrenRecursiveIter,
    components::{
        component::{AbstractComponent, ChildOf, EnumTag},
        component_bundle::ComponentBundle,
        component_query::ComponentQuery,
    },
    identifier::Identifier,
    query::{Query, QueryState},
    relationship::{FindRelationshipsIter, Relationship, RelationshipsIter},
    world::{archetypes, archetypes_mut},
};

#[derive(Clone, Copy)]
pub struct Entity(pub(crate) Identifier);

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

impl From<Identifier> for Entity {
    fn from(value: Identifier) -> Self {
        Entity(value)
    }
}

pub const WILDCARD: Entity = Entity(WILDCARD_RELATIONSHIP);

impl Entity {
    pub fn id(&self) -> Identifier {
        self.0
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
    pub fn name_parent(&self) -> Entity {
        if let Some(parent) = self.find_rel::<ChildOf, Wildcard>() {
            parent.0.into()
        } else {
            WILDCARD
        }
    }
    pub fn set_name(&self, name: &str) -> Self {
        self.name().set(name);
        *self
    }
    pub fn has_name(&self) -> bool {
        let parent = self.name_parent();
        archetypes(|archetypes| archetypes.entity_has_name(NameLeft::from_ids(self.0, parent.0)))
    }
    pub fn remove_name(&self) -> Self {
        let parent = self.name_parent();
        archetypes_mut(|archetypes| archetypes.remove_entity_name((self.0, parent.0).into()));
        *self
    }
    pub fn get_name(&self) -> Option<EntityNameGetter> {
        if !self.has_name() {
            return None;
        }
        let parent = self.name_parent();
        Some(EntityNameGetter::new((self.0, parent.0).into()))
    }
    pub fn name(&self) -> EntityNameGetter {
        let parent = self.name_parent();
        EntityNameGetter::new((self.0, parent.0).into())
    }
    pub fn parent(&self) -> Option<Entity> {
        self.find_rel::<ChildOf, Wildcard>().map(|r| r.target())
    }
    pub fn find_mixed_rels<R: AbstractComponent>(&self, target: Entity) -> FindRelationshipsIter {
        archetypes_mut(|archetypes| {
            let relation = archetypes.component_id::<R>();
            let record = archetypes.record(self.0).unwrap();
            let archetype = archetypes.archetype_from_record(&record).unwrap();
            FindRelationshipsIter::from_archetype(archetype, relation, target.0)
        })
    }
    pub fn find_ent_rels(&self, relation: Entity, target: Entity) -> FindRelationshipsIter {
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
    pub fn rels(&self) -> RelationshipsIter {
        archetypes(|archetypes| {
            let record = archetypes.record(self.0).unwrap();
            let archetype = archetypes.archetype_from_record(&record).unwrap();
            RelationshipsIter::new(archetype, record.archetype_row)
        })
    }
    pub fn has_relationship(&self, relationship: Relationship) -> bool {
        archetypes(|archetypes| archetypes.has_component(relationship.0, self.0))
    }
    pub fn add_child_of(&self, parent: Entity) -> Self {
        let name_parent = self.name_parent();
        let old_entity_and_parent = NameLeft::from_ids(self.into(), name_parent.into());
        self.add_mixed_tag_rel::<ChildOf>(parent);
        archetypes_mut(|archetypes| {
            if archetypes.name_by_entity(old_entity_and_parent).is_some() {
                let name = archetypes
                    .name_by_entity(old_entity_and_parent)
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
        *self
    }

    pub fn is_child_of(&self, parent: Entity) -> bool {
        self.has_mixed_rel::<ChildOf>(parent)
    }

    pub fn remove_child_of(&self, parent: Entity) {
        let old_entity_and_parent = NameLeft::from_ids(self.into(), parent.into());
        self.remove_mixed_rel::<ChildOf>(parent);
        archetypes_mut(|archetypes| {
            if archetypes.name_by_entity(old_entity_and_parent).is_some() {
                let name = archetypes
                    .name_by_entity(old_entity_and_parent)
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

    pub fn add_comp<T: ComponentBundle>(&self, bundle: T) -> Entity {
        assert!(std::mem::size_of::<T>() > 0);
        bundle.add(self);
        *self
    }

    pub fn get_or_add_comp<T: AbstractComponent>(
        &self,
        init: impl FnOnce() -> T,
    ) -> ComponentGetter<T> {
        assert!(std::mem::size_of::<T>() > 0);
        if !self.has_comp::<T>() {
            self.add_comp(init());
        }
        self.get_comp::<T>()
    }

    pub fn remove_comp<T: ComponentBundle>(&self) -> Entity {
        assert!(std::mem::size_of::<T>() > 0);
        T::remove(self);
        *self
    }

    pub fn has_enum_tag<T: EnumTag>(&self, tag: T) -> bool {
        archetypes_mut(|archetypes| archetypes.has_enum_tag(tag, self.0))
    }

    pub fn add_enum_tag<T: EnumTag>(&self, tag: T) -> Entity {
        archetypes_mut(|archetypes| {
            archetypes.add_enum_tag(self.0, tag).unwrap();
        });
        *self
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
        *self
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
        *self
    }

    pub fn add_rel_second<R: AbstractComponent, T: AbstractComponent>(&self, value: T) -> Self {
        assert!(std::mem::size_of::<R>() == 0);
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<R>();
            let target_id = archetypes.component_id::<T>();
            archetypes
                .add_data_relationship(self.0, relation_id, target_id, value)
                .unwrap();
        });

        *self
    }

    pub fn rel_second<R: AbstractComponent, T: AbstractComponent>(&self) -> ComponentGetter<T> {
        assert!(std::mem::size_of::<R>() == 0);
        assert!(std::mem::size_of::<T>() > 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes.get_component(relationship, self.0).unwrap()
        })
    }

    pub fn rel_second_mut<R: AbstractComponent, T: AbstractComponent>(&self) -> ComponentGetter<T> {
        assert!(std::mem::size_of::<R>() == 0);
        assert!(std::mem::size_of::<T>() > 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes.get_component(relationship, self.0).unwrap()
        })
    }

    pub fn add_rel_first<R: AbstractComponent, T: AbstractComponent>(&self, value: R) -> Self {
        assert!(std::mem::size_of::<R>() > 0);
        assert!(std::mem::size_of::<T>() == 0);
        {
            archetypes_mut(|archetypes| {
                let relation_id = archetypes.component_id::<R>();
                let target_id = archetypes.component_id::<T>();
                archetypes
                    .add_data_relationship(self.0, relation_id, target_id, value)
                    .unwrap();
            });
        }
        *self
    }

    pub fn rel_first<R: AbstractComponent, T: AbstractComponent>(&self) -> ComponentGetter<R> {
        assert!(std::mem::size_of::<R>() > 0);
        assert!(std::mem::size_of::<T>() == 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes.get_component(relationship, self.0).unwrap()
        })
    }

    pub fn rel_first_mut<R: AbstractComponent, T: AbstractComponent>(&self) -> ComponentGetter<R> {
        assert!(std::mem::size_of::<R>() > 0);
        assert!(std::mem::size_of::<T>() == 0);

        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes.get_component(relationship, self.0).unwrap()
        })
    }

    pub fn has_mixed_rel<R: AbstractComponent>(&self, target: Entity) -> bool {
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<R>();
            let relationship = Archetypes::relationship_id(relation_id, target.0);
            archetypes.has_component(relationship, self.0)
        })
    }

    pub fn add_mixed_rel<R: AbstractComponent>(&self, target: Entity, value: R) -> Self {
        assert!(std::mem::size_of::<R>() > 0);
        {
            archetypes_mut(|archetypes| {
                let relation_id = archetypes.component_id::<R>();
                archetypes
                    .add_data_relationship(self.0, relation_id, target.0, value)
                    .unwrap();
            });
        }
        *self
    }

    pub fn add_mixed_tag_rel<R: AbstractComponent>(&self, target: Entity) -> Self {
        assert!(std::mem::size_of::<R>() == 0);
        {
            archetypes_mut(|archetypes| {
                let relation_id = archetypes.component_id::<R>();
                archetypes
                    .add_relationship(self.0, relation_id, target.0)
                    .unwrap();
            });
        }
        *self
    }

    pub fn mixed_rel<R: AbstractComponent>(&self, target: Entity) -> ComponentGetter<R> {
        assert!(std::mem::size_of::<R>() > 0);
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<R>();
            let relationship = Archetypes::relationship_id(relation_id, target.0);
            archetypes.get_component(relationship, self.0).unwrap()
        })
    }

    pub fn mixed_rel_mut<R: AbstractComponent>(&self, target: Entity) -> ComponentGetter<R> {
        assert!(std::mem::size_of::<R>() > 0);
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<R>();
            let relationship = Archetypes::relationship_id(relation_id, target.0);
            archetypes.get_component(relationship, self.0).unwrap()
        })
    }

    pub fn remove_mixed_rel<R: AbstractComponent>(&self, target: Entity) -> Self {
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

        *self
    }

    pub fn add_tag<T: AbstractComponent>(&self) -> Entity {
        assert!(std::mem::size_of::<T>() == 0);
        archetypes_mut(|archetypes| {
            let tag = archetypes.component_id::<T>();
            archetypes.add_component_tag(self.0, tag).unwrap();
        });
        *self
    }

    pub fn has_tag<T: AbstractComponent>(&self) -> bool {
        assert!(std::mem::size_of::<T>() == 0);
        archetypes_mut(|archetypes| {
            let tag = archetypes.component_id::<T>();
            archetypes.has_component(tag, self.0)
        })
    }

    pub fn add_ent_tag(&self, tag: Entity) -> Entity {
        archetypes_mut(|archetypes| {
            archetypes.add_entity_tag(self.0, tag.0).unwrap();
        });
        *self
    }

    pub fn remove_ent_tag(&self, tag: Entity) -> Entity {
        archetypes_mut(|archetypes| {
            let _ = archetypes.remove_component(tag.0, self.0, TableReusage::Reuse);
        });
        *self
    }

    pub fn remove_ent_rel(&self, relation: Entity, target: Entity) -> Self {
        archetypes_mut(|archetypes| {
            let relationship = Archetypes::relationship_id(relation.0, target.0);
            let _ = archetypes.remove_component(relationship, self.0, TableReusage::Reuse);
        });
        *self
    }

    pub fn has_ent_rel(&self, relation: Entity, target: Entity) -> bool {
        archetypes_mut(|archetypes| {
            let relationship = Archetypes::relationship_id(relation.0, target.0);
            archetypes.has_component(relationship, self.0)
        })
    }

    pub fn add_ent_rel(&self, relation: Entity, target: Entity) -> Self {
        archetypes_mut(|archetypes| {
            archetypes
                .add_relationship(self.0, relation.0, target.0)
                .unwrap();
        });
        *self
    }

    pub fn add_rel<R: AbstractComponent, T: AbstractComponent>(&self) -> Self {
        assert!(std::mem::size_of::<T>() == 0);
        assert!(std::mem::size_of::<R>() == 0);
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<R>();
            let target_id = archetypes.component_id::<T>();
            archetypes
                .add_relationship(self.0, relation_id, target_id)
                .unwrap();
        });
        *self
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
        *self
    }
    ///Clones all of entities' components
    pub fn cloned(&self) -> Entity {
        archetypes_mut(|archetypes| Self(archetypes.clone_entity(self.0).unwrap()))
    }

    pub fn has_rel<R: AbstractComponent, T: AbstractComponent>(&self) -> bool {
        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            archetypes.has_component(relationship, self.0)
        })
    }

    pub fn has_ent_tag(&self, tag: Entity) -> bool {
        archetypes_mut(|archetypes| archetypes.has_component(tag.0, self.0))
    }

    pub fn get_comp<T: AbstractComponent>(&self) -> ComponentGetter<T> {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes.get_component(id, self.0).unwrap_or_else(|| {
                panic!(
                    "expected entity to have component {0}",
                    tynm::type_name::<T>()
                )
            })
        })
    }

    pub fn comp<T: AbstractComponent>(&self, f: impl FnOnce(&T)) -> Entity {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes
                .get_component(id, self.0)
                .unwrap_or_else(|| {
                    panic!(
                        "expected entity to have component {0}",
                        tynm::type_name::<T>()
                    )
                })
                .get(f);
        });
        *self
    }

    pub fn comp_mut<T: AbstractComponent>(&self, f: impl FnOnce(&mut T)) -> Entity {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes
                .get_component(id, self.0)
                .unwrap_or_else(|| {
                    panic!(
                        "expected entity to have component {0}",
                        tynm::type_name::<T>()
                    )
                })
                .get_mut(f);
        });
        *self
    }

    pub fn comps<T: ComponentQuery>(&self, f: impl FnOnce(T::Item<'_>)) -> Entity {
        f(T::fetch(self));
        *self
    }

    pub fn comps_ret<T: ComponentQuery, R>(&self, f: impl FnOnce(T::Item<'_>) -> R) -> R {
        f(T::fetch(self))
    }

    pub fn try_get_comp<T: AbstractComponent>(&self) -> Option<ComponentGetter<T>> {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes.get_component(id, self.0)
        })
    }

    pub fn has_children(&self) -> bool {
        !QueryState::<(), ()>::new()
            .with_children_of(*self)
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

    pub fn instance_of(&self, prefab: Entity) -> Entity {
        let entity = prefab.cloned();
        entity.add_mixed_tag_rel::<InstanceOf>(prefab);
        entity
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
        *self
    }

    pub fn toggle_active(&self) -> Entity {
        let is_active =
            archetypes_mut(|archetypes| archetypes.record_mut(self.0).unwrap().entity.is_active());
        self.set_active_recursive(!is_active);
        *self
    }

    pub fn activate(&self) -> Entity {
        self.set_active_recursive(true);
        *self
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
        *self
    }
}

#[cfg(test)]
mod tests {
    use std::hash::{DefaultHasher, Hasher};

    use archetypes::{Wildcard, ENTITY_ID};
    use bevy_reflect::{DynamicStruct, FromReflect, Reflect, Struct};

    use crate::components::test_components::{
        Apples, Begin, End, IsCool, Likes, Name, Oranges, Owes, Position, Velocity,
    };
    use crate::plugins::Plugin;
    use crate::systems::{AbstractSystemsWithStateData, States};
    use crate::{component_bundle, enum_tag, impl_system, impl_system_states};
    use crate::{
        query::QueryComoponentId,
        query_structs::{Not, With, WithRelation},
        systems::{SystemStage, SystemsData},
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

        world.add_plugins(TestPlugin);
        world.run();
    }

    #[test]
    pub fn reflect() {
        #[derive(Reflect)]
        struct Test {
            x: i32,
            b: String,
            c: f64,
        }
        let test = Test {
            x: 10,
            b: String::from("hi!"),
            c: 1.64,
        };
        let test_reflect: &dyn Struct = &test;
        // for field in test_reflect.iter_fields() {
        //     match field.reflect_ref() {
        //         bevy_reflect::ReflectRef::Struct(s) => {
        //
        //         }
        //         _ => (),
        //     }
        //     // let s: &dyn Struct = field.as_any().try;
        // }
    }
    #[test]
    pub fn comps() {
        let world = World::new();
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

        world.run();
        world.run();
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
        impl_component! {
            #[derive(Default, Debug)]
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
        impl_component! {
            #[derive(Default, Debug)]
            pub struct Velocity(i32, i32);
        }
        let world = World::new();
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
        let parent = world.add_entity();
        world
            .add_entity()
            .add_comp(Position::new(1, 2))
            .add_child_of(parent);
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
        component_bundle! {
            struct Extra {
                name: Name,
            }
        }
        component_bundle! {
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
        let e = world
            .add_entity()
            .add_comp((Position::new(1, 2), Velocity::new(3, 4)));

        let pos = e.get_comp::<Position>().get(|c| *c);
        let vel = e.get_comp::<Velocity>().get(|c| *c);

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

        let e = world.add_entity();
        e.get_or_add_comp(|| Position::new(1, 2));
        e.get_comp::<Position>().get_mut(|p| {
            assert_eq!(p.x, 1);
            assert_eq!(p.y, 2);

            p.x += 1;
        });

        e.get_or_add_comp(|| Position::new(1, 2)).get(|p| {
            assert_eq!(p.x, 2);
        });
    }

    #[test]
    fn on_component_add() {
        let world = World::new();

        world.on_comp_add::<Position>(|entity: Entity, _| {
            entity.get_comp::<Position>().get_mut(|p| p.x += 1);
        });

        let e = world
            .add_entity()
            .add_comp::<Position>(Position { x: 0, y: 0 });

        e.get_comp::<Position>().get(|p| {
            assert_eq!(p.x, 1);
        });
    }

    #[test]
    fn adding_components_inside_query() {
        let world = World::new();
        let e1 = world.add_entity_named("e1").add_tag::<IsCool>();
        let e2 = world
            .add_entity_named("e2")
            .add_tag::<IsCool>()
            .add_comp::<Position>(Position { x: 1, y: 2 });

        let mut count = 0;
        for e in world
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
            e.get_comp::<Position>().get(|p| {
                assert_eq!(p.x, 11);
                assert_eq!(p.y, 16);
            });
            e.get_comp::<Velocity>().get(|p| {
                assert_eq!(p.x, 1);
                assert_eq!(p.y, 5);
            });
        }
    }

    #[test]
    fn querying_empty_entities() {
        let world = World::new();
        let e = world.add_entity();
        world.add_entity().add_comp(Position { x: 1, y: 2 });
        let mut query = world
            .query::<&Entity>()
            .with_ent_tag(Entity(ENTITY_ID))
            .build();
        assert_eq!(1, query.iter().count());

        query.iter().for_each(|e| e.remove());

        assert!(!e.is_alive());
    }

    #[test]
    fn replacing_components() {
        let world = World::new();
        let e1 = world.add_entity().add_comp(Position { x: 1, y: 1 });
        world.add_entity().add_comp(Position { x: 2, y: 2 });

        e1.add_comp(Position { x: 3, y: 3 });

        e1.get_comp::<Position>().get(|p| {
            assert_eq!(p.x, 3);
            assert_eq!(p.y, 3);
        });
    }

    #[test]
    fn replacing_relationships() {
        let world = World::new();
        let e1 = world
            .add_entity()
            .add_rel_second::<Begin, _>(Position { x: 1, y: 1 });
        world
            .add_entity()
            .add_rel_second::<Begin, _>(Position { x: 2, y: 2 });

        e1.add_rel_second::<Begin, _>(Position { x: 3, y: 3 });

        e1.rel_second::<Begin, Position>().get(|p| {
            assert_eq!(p.x, 3);
            assert_eq!(p.y, 3);
        });
    }

    #[test]
    fn removing_children() {
        let world = World::new();
        let e = world.add_entity();
        let child = world.add_entity().add_child_of(e);
        let grand_child = world.add_entity().add_child_of(child);
        for child in e.children().iter() {
            child.remove();
        }

        assert!(!child.is_alive());
        assert!(!grand_child.is_alive());
        let child = world.add_entity_named("child").add_child_of(e);
        child.remove_child_of(e);

        child.name().get(|n| {
            assert_eq!(n, "child");
        })
    }

    #[test]
    fn recursive_deactivation() {
        let world = World::new();
        let e = world.add_entity();
        let child = world.add_entity().add_child_of(e);

        e.diactivate();
        assert!(!child.is_active());
    }
    #[test]
    fn entity_tags() {
        let world = World::new();
        let e = world.add_entity_named("e");
        let tag = world.add_entity_named("tag");
        e.add_ent_tag(tag);
        assert!(e.has_ent_tag(tag));

        tag.remove();
        assert!(!e.has_ent_tag(tag))
    }
    #[test]
    fn colliding_names() {
        let world = World::new();
        let e = world.add_entity();
        let child_bob = world.add_entity().add_child_of(e).set_name("Bob");
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
        enum_tag! {
            #[derive(Debug, Eq, PartialEq)]
            enum PlayerState {
                Walking,
                Falling,
            }
        }

        let world = World::new();
        let e = world.add_entity().add_enum_tag(PlayerState::Walking);

        assert!(e.has_any_enum_tag::<PlayerState>());
        assert!(e.has_enum_tag(PlayerState::Walking));
        assert!(!e.has_enum_tag(PlayerState::Falling));
        assert_eq!(e.enum_tag::<PlayerState>(), PlayerState::Walking);

        e.add_enum_tag(PlayerState::Falling);
        assert!(e.has_enum_tag(PlayerState::Falling));
        assert_eq!(e.enum_tag::<PlayerState>(), PlayerState::Falling);

        enum_tag! {
            #[derive(Debug, Eq, PartialEq)]
            enum Progerss {
                Beginner,
                Pro,
            }
        }

        e.add_enum_tag(Progerss::Pro);

        let mut query = world
            .query::<()>()
            .with_enum_tag(PlayerState::Falling)
            .with_enum_tag(Progerss::Pro)
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
        impl_system!(CustomSystem, states);
        impl CustomSystem {
            fn run(&mut self, _world: &World, states: &States) {
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
                display_game_menu_system.with_state(GameState::InMainMenu),
                // .run_if(|w| true),
                SystemStage::Begin,
            )
            .add_systems(
                update_positions_system.with_state(GameState::InGame),
                SystemStage::Update,
            )
            .add_systems(
                update_world_active.with_state((GameState::InGame, WorldState::Active)),
                SystemStage::Update,
            )
            .add_systems(CustomSystem { value: 10 }, SystemStage::Begin);
        world.run();
    }

    #[test]
    fn children() {
        let world = World::new();
        let e = world.add_entity_named("e");
        assert!(!e.has_children());

        let child1 = world.add_entity().add_child_of(e).set_name("child1");
        //TODO: figure out why this fails
        //that's because I dont update query archetypes
        assert!(e.has_children());
        let child2 = world.add_entity().add_child_of(e).set_name("child2");
        let grand_child = world.add_entity();
        let grand_grand_child = world.add_entity();
        grand_child.add_child_of(child1).set_name("grand child");
        grand_grand_child
            .add_child_of(grand_child)
            .set_name("grand-grand child");

        assert!(child1.is_child_of(e));
        assert!(child2.is_child_of(e));

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
        let likes = world.add_entity();
        let apples = world.add_entity();
        let parent = world.add_entity();
        let e = world
            .add_entity()
            .add_rel::<Likes, Apples>()
            .add_ent_rel(likes, apples)
            .add_child_of(parent)
            .add_comp(Velocity { x: 10, y: 10 })
            .add_rel_second::<Begin, _>(Velocity { x: 10, y: 10 });
        let mut count = 0;
        for rel in e.rels() {
            count += 1;
            assert!(e.has_relationship(rel));
        }
        assert_eq!(count, 4);
    }

    #[test]
    fn tag_only_query() {
        let world = World::new();
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
        let entity = world.add_entity();
        entity.add_ent_tag(tag);
    }

    use super::*;
    #[test]
    fn query_that_has_it_all() {
        //prepare thyself
        let world = World::new();
        let bob = world.add_entity();
        let hates = world.add_entity();
        let idkwhat = world.add_entity();
        let entity_tag = world.add_entity();
        let child = world.add_entity();
        let god_entity = world
            .add_entity()
            .add_comp(Position { x: 10, y: 20 })
            .add_comp(Position::new(3, 4))
            .add_tag::<IsCool>()
            .add_ent_tag(entity_tag)
            .add_ent_rel(hates, bob)
            .add_rel::<Likes, Apples>()
            .add_rel_first::<Owes, Apples>(Owes { amount: 10 })
            .add_rel_second::<Begin, Position>(Position::new(1, 2))
            .add_mixed_rel(idkwhat, Position::new(5, 6));

        child.add_child_of(god_entity);

        // let query = world.query_filtered()
    }

    #[test]
    fn data_relation_query() {
        let world = World::new();
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
        let e1 = world.add_entity().add_comp(Position { x: 1, y: 2 });
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
        let e1 = world.add_entity().add_comp(Position { x: 1, y: 2 });
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
        let e1 = world.add_entity().add_comp(Position { x: 1, y: 2 });
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

        let pos_sum_1 = e1.get_comp::<Position>().get(|p| p.x + p.y);
        let pos_sum_2 = e2.get_comp::<Position>().get(|p| p.x + p.y);
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
        let prefab = world
            .add_prefab()
            .add_comp(Velocity { x: 10, y: 20 })
            .add_rel::<Likes, Oranges>();

        let instance = world.add_entity().instance_of(prefab);
        assert!(instance.has_mixed_rel::<InstanceOf>(prefab));
        instance.get_comp::<Velocity>().get(|p| {
            assert_eq!(p.x, 10);
            assert_eq!(p.y, 20);
        });
        assert!(instance.has_rel::<Likes, Oranges>());
    }

    #[test]
    fn everthing_at_once_cloned() {
        let world = World::new();
        let bob = world.add_entity();
        let hates = world.add_entity();
        let idkwhat = world.add_entity();
        let entity_tag = world.add_entity();
        let entity = world
            .add_entity()
            .add_comp(Position { x: 10, y: 20 })
            .add_comp(Position::new(3, 4))
            .add_tag::<IsCool>()
            .add_ent_tag(entity_tag)
            .add_ent_rel(hates, bob)
            .add_rel::<Likes, Apples>()
            .add_rel_first::<Owes, Apples>(Owes { amount: 10 })
            .add_rel_second::<Begin, Position>(Position::new(1, 2))
            .add_mixed_rel(idkwhat, Position::new(5, 6));

        let entity = entity.cloned();

        assert!(entity.has_comp::<Position>());
        assert!(entity.has_tag::<IsCool>());
        assert!(entity.has_ent_tag(entity_tag));
        assert!(entity.has_rel::<Likes, Apples>());
        assert!(entity.has_rel::<Owes, Apples>());
        assert!(entity.has_rel::<Begin, Position>());
        assert!(entity.has_ent_rel(hates, bob));
        assert!(entity.has_mixed_rel::<Position>(idkwhat));

        entity.get_comp::<Position>().get(|pos| {
            assert_eq!(pos.x, 3);
            assert_eq!(pos.y, 4);
        });
        // entity.rel_first::<Owes, Begin>().try_get(|stuff| {});
        entity.rel_first::<Owes, Apples>().get(|ows| {
            assert_eq!(ows.amount, 10);
        });
        entity.rel_second::<Begin, Position>().get(|pos| {
            assert_eq!(pos.x, 1);
            assert_eq!(pos.y, 2);
        });
        entity.mixed_rel::<Position>(idkwhat).get(|pos| {
            assert_eq!(pos.x, 5);
            assert_eq!(pos.y, 6);
        });
        entity.remove_comp::<Position>();
        assert!(!entity.has_comp::<Position>());
        entity.remove_tag::<IsCool>();
        assert!(!entity.has_tag::<IsCool>());
        entity.remove_ent_tag(entity_tag);
        assert!(!entity.has_ent_tag(entity_tag));
        entity.remove_ent_rel(hates, bob);
        assert!(!entity.has_ent_rel(hates, bob));
        entity.remove_rel::<Likes, Apples>();
        assert!(!entity.has_rel::<Likes, Apples>());
        entity.remove_rel::<Owes, Apples>();
        assert!(!entity.has_rel::<Owes, Apples>());
        entity.remove_rel::<Begin, Position>();
        assert!(!entity.has_rel::<Begin, Position>());
        entity.remove_mixed_rel::<Position>(idkwhat);
        assert!(!entity.has_mixed_rel::<Position>(idkwhat));
        entity.remove_mixed_rel::<Likes>(bob);
        assert!(!entity.has_mixed_rel::<Likes>(bob));
    }

    #[test]
    fn wildcard() {
        let world = World::new();
        let entity = world.add_entity();
        entity.add_rel::<Likes, Apples>();
        assert!(entity.has_rel::<Likes, Wildcard>());
        assert!(entity.has_rel::<Wildcard, Apples>());

        entity.remove_rel::<Likes, Apples>();
        let is = world.add_entity();
        let helicopter = world.add_entity();

        entity.add_ent_rel(is, helicopter);
        assert!(entity.has_ent_rel(WILDCARD, helicopter));
        assert!(entity.has_ent_rel(is, WILDCARD));
    }
    #[test]
    fn everthing_at_once() {
        let world = World::new();
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
                    .add_ent_tag(entity_tag)
                    .add_ent_rel(hates, bob)
                    .add_rel::<Likes, Apples>()
                    .add_rel_first::<Owes, Apples>(Owes { amount: 10 })
                    .add_rel_second::<Begin, Position>(Position::new(1, 2))
                    .add_mixed_rel(idkwhat, Position::new(5, 6))
                    .add_mixed_tag_rel::<Likes>(bob),
            )
        }

        let entity = entity.unwrap();

        assert!(entity.has_comp::<Position>());
        assert!(entity.has_tag::<IsCool>());
        assert!(entity.has_ent_tag(entity_tag));
        assert!(entity.has_rel::<Likes, Apples>());
        assert!(entity.has_rel::<Owes, Apples>());
        assert!(entity.has_rel::<Begin, Position>());
        assert!(entity.has_ent_rel(hates, bob));
        assert!(entity.has_mixed_rel::<Position>(idkwhat));
        assert!(entity.has_mixed_rel::<Likes>(bob));

        entity.get_comp::<Position>().get(|pos| {
            assert_eq!(pos.x, 3);
            assert_eq!(pos.y, 4);
        });
        // entity.rel_first::<Owes, Begin>().try_get(|stuff| {});
        entity.rel_first::<Owes, Apples>().get(|ows| {
            assert_eq!(ows.amount, 10);
        });
        entity.rel_second::<Begin, Position>().get(|pos| {
            assert_eq!(pos.x, 1);
            assert_eq!(pos.y, 2);
        });
        entity.mixed_rel::<Position>(idkwhat).get(|pos| {
            assert_eq!(pos.x, 5);
            assert_eq!(pos.y, 6);
        });
        entity.remove_comp::<Position>();
        assert!(!entity.has_comp::<Position>());
        entity.remove_tag::<IsCool>();
        assert!(!entity.has_tag::<IsCool>());
        entity.remove_ent_tag(entity_tag);
        assert!(!entity.has_ent_tag(entity_tag));
        entity.remove_ent_rel(hates, bob);
        assert!(!entity.has_ent_rel(hates, bob));
        entity.remove_rel::<Likes, Apples>();
        assert!(!entity.has_rel::<Likes, Apples>());
        entity.remove_rel::<Owes, Apples>();
        assert!(!entity.has_rel::<Owes, Apples>());
        entity.remove_rel::<Begin, Position>();
        assert!(!entity.has_rel::<Begin, Position>());
        entity.remove_mixed_rel::<Position>(idkwhat);
        assert!(!entity.has_mixed_rel::<Position>(idkwhat));
        entity.remove_mixed_rel::<Likes>(bob);
        assert!(!entity.has_mixed_rel::<Position>(idkwhat));
    }

    #[test]
    fn data_rels_first() {
        let world = World::new();
        let ann = world.add_entity();
        ann.add_rel_first::<Owes, Apples>(Owes { amount: 10 });
        ann.add_rel_first::<Owes, Oranges>(Owes { amount: 10 });
        ann.remove_rel::<Owes, Apples>();
        assert!(!ann.has_rel::<Owes, Apples>());
        assert!(ann.has_rel::<Owes, Oranges>());
    }

    #[test]
    fn data_rels_second() {
        let world = World::new();
        let entity = world.add_entity();
        entity
            .add_rel_second::<Begin, _>(Position::new(1, 2))
            .add_rel_second::<End, _>(Position::new(3, 4));
        let p1 = entity.rel_second::<Begin, Position>().get(|c| *c);
        let p2 = entity.rel_second::<End, Position>().get(|c| *c);

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
        let entity = world.add_entity();
        entity.add_rel::<Likes, Apples>();
        entity.add_comp(Position { x: 10, y: 20 });

        let pos = entity.get_comp::<Position>().get(|c| *c);
        assert_eq!(pos.x, 10);
        assert_eq!(pos.y, 20);

        assert!(entity.has_rel::<Likes, Apples>());
    }
    #[test]
    fn adding_comps_to_comps() {
        impl_component! {
            struct Material {
                data: u32,
            }
        }
        impl_component! {
            struct Uniform {
                other_data: u32,
            }
        }
        impl_component! {
            struct HasMaterial {}
        }
        //for some reason has_components(COMPONENT_ID, HasMaterial) return true before IsComponent is added
        let world = World::new();
        world.comp_entity::<HasMaterial>();
        // let comp = world.comp_entity::<Uniform>();
        // comp.add_comp(Material { data: 5 });
        // dbg!("begin");
        // let e = world.comp_entity::<HasMaterial>();
        // dbg!(archetypes(|a| a.debug_print_archetypes()));
        // dbg!(archetypes(|a| a.record(e.into()).unwrap().arhetype_id));
        let entity = world.add_entity().add_tag::<HasMaterial>();
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
    fn serialization() {
        enum_tag! {
            #[derive(Debug, Eq, PartialEq)]
            enum MyEnumTag {
                StateOne
            }
        }

        let world = World::new();
        let entity_tag = world.add_entity();
        let child = world.add_entity();
        let entity = world
            .add_entity()
            .add_comp(Position::new(1, 2))
            .add_tag::<IsCool>()
            .add_ent_tag(entity_tag)
            .add_enum_tag(MyEnumTag::StateOne)
            .add_rel::<Likes, Apples>()
            .add_child_of(child)
            .add_rel_first::<Owes, Apples>(Owes { amount: 10 })
            .add_rel_second::<Begin, _>(Position::new(2, 3))
            .serialize()
            .unwrap();
        println!("{entity}");
    }
}

// let world = World::new();
// let uniform_entity = world.component_entity::<Uniform>();
// uniform_entity.add_comp(Material { data: 5 });
// let e = world.add_entity().add_comp(Uniform {
//     color: Vec4::ONE,
//     scale: Vec2::ONE,
//     offset: Vec2::ZERO,
// }).add_tag::<HasMaterial>();
// dbg!(e);
// let mut query = world.query_filtered::<&Entity, With<HasMaterial>>().build();
// for e in query.iter() {
//     dbg!(e);
// }
