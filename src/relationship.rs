use std::{collections::BTreeSet, rc::Rc};

use crate::{
    archetype::ArchetypeRow,
    archetypes::{Archetypes, WILDCARD_32, WILDCARD_RELATIONSHIP},
    components::component::{AbstractComponent, ChildOf},
    entity::Entity,
    identifier::Identifier,
    world::{archetypes, archetypes_mut},
    wrappers::ArchetypeCell,
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Relationship(pub(crate) Identifier);

impl Relationship {
    pub fn from_id(id: Identifier) -> Self {
        if !id.is_relationship() {
            panic!("expected relationship");
        }
        Self(id)
    }

    pub fn new<R: AbstractComponent, T: AbstractComponent>() -> Self {
        archetypes_mut(|a| a.relationship_id_typed::<R, T>().into())
    }

    pub fn new_ent(relation: Entity, target: Entity) -> Self {
        let relationship = Archetypes::relationship_id(relation.0, target.0);
        relationship.into()
    }

    pub fn new_mixed<R: AbstractComponent>(target: Entity) -> Self {
        archetypes_mut(|a| {
            let relation_id = a.component_id::<R>();
            let relationship = Archetypes::relationship_id(relation_id, target.0);
            relationship.into()
        })
    }

    pub fn get_new(id: Identifier) -> Option<Self> {
        if !id.is_relationship() {
            None
        } else {
            Some(Self(id))
        }
    }

    pub fn id(&self) -> Identifier {
        self.0
    }

    pub fn relation(&self) -> Entity {
        Entity(archetypes(|a| a.relation_entity(self.0)).unwrap())
    }

    pub fn target(&self) -> Entity {
        Entity(archetypes(|a| a.target_entity(self.0).unwrap()))
    }
}

#[derive()]
pub struct RelationshipsIter {
    components: Vec<Identifier>,
    index: usize,
}

impl RelationshipsIter {
    pub fn new(archetype: &ArchetypeCell) -> Self {
        Self {
            components: archetype
                .borrow()
                .components_ids_set()
                .iter()
                .cloned()
                .collect(),
            index: 0,
        }
    }
}

impl Iterator for RelationshipsIter {
    type Item = Relationship;

    fn next(&mut self) -> Option<Self::Item> {
        let component: Identifier = loop {
            if self.index == self.components.len() {
                return None;
            }

            let component = self.components[self.index];
            self.index += 1;
            if !component.is_relationship() {
                continue;
            } else {
                break component;
            }
        };

        Some(component.into())
    }
}

pub struct FindRelationshipsIter {
    index: usize,
    relation: u32,
    target: u32,
    relationship: Identifier,
    archetype: ArchetypeCell,
}

impl FindRelationshipsIter {
    pub fn from_archetype(
        archetype: &ArchetypeCell,
        relation: Identifier,
        target: Identifier,
    ) -> Self {
        Self {
            index: 0,
            relation: relation.low32(),
            target: target.low32(),
            relationship: Archetypes::relationship_id(relation, target),
            archetype: archetype.clone(),
        }
    }
    pub fn from_component(archetype: &ArchetypeCell, component: Identifier) -> Self {
        Self {
            index: 0,
            relation: component.low32(),
            target: component.second(),
            relationship: component,
            archetype: archetype.clone(),
        }
    }
}

impl Iterator for FindRelationshipsIter {
    type Item = Relationship;

    fn next(&mut self) -> Option<Self::Item> {
        let component: Identifier = loop {
            let archetype = self.archetype.borrow();
            let components = archetype.components_ids();
            if self.index == components.len() {
                return None;
            }
            let component = components[self.index];
            self.index += 1;
            //TODO : make it an option to include tags or something
            // if !component.is_relationship() {
            //     continue;
            // }
            if (component == WILDCARD_RELATIONSHIP)
                || (self.relation == WILDCARD_32 && component.second() == self.target)
                || (self.target == WILDCARD_32 && component.low32() == self.relation)
                || (component == self.relationship)
            {
                break component;
            }
        };
        Some(component.into())
    }
}

impl From<Identifier> for Relationship {
    fn from(value: Identifier) -> Self {
        Self(value)
    }
}
