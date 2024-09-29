use std::{cell::RefCell, rc::Rc};

use crate::{
    archetypes::{Archetypes, ChildOf},
    entity::Entity,
    identifier::Identifier,
    world::archetypes,
};

pub struct ChildrenRecursiveIter {
    pub entity: Identifier,
    index: usize,
    children: Rc<RefCell<Vec<(Entity, Depth)>>>,
}

impl ChildrenRecursiveIter {
    pub fn new(entity: Identifier, children_pool: Rc<RefCell<Vec<(Entity, Depth)>>>) -> Self {
        children_pool.borrow_mut().clear();

        Self {
            entity,
            index: 0,
            children: children_pool.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq)]
pub struct Depth(pub u32);

impl From<u32> for Depth {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<Depth> for u32 {
    fn from(value: Depth) -> Self {
        value.0
    }
}

pub fn get_children_recursive(
    entity: Identifier,
    archetypes: &Archetypes,
    children: &mut Vec<(Entity, Depth)>,
    depth: Depth,
) {
    let relation = archetypes.component_id::<ChildOf>();
    let relationship = Archetypes::relationship_id(relation, entity);
    let Some(archetypes_set) = archetypes.get_archetypes_with_id(relationship) else {
        return;
    };

    for archetype in archetypes_set.iter() {
        for entity_index in archetype.borrow().entity_indices() {
            let record = archetypes.record_by_index(*entity_index).unwrap();
            children.push((record.entity.into(), depth));
            get_children_recursive(record.entity, archetypes, children, (depth.0 + 1).into());
        }
    }
}

impl Drop for ChildrenRecursiveIter {
    fn drop(&mut self) {
        self.children.borrow_mut().clear();
    }
}

impl Iterator for ChildrenRecursiveIter {
    type Item = (Entity, Depth);

    fn next(&mut self) -> Option<Self::Item> {
        let children: &mut _ = &mut self.children.borrow_mut();
        if children.is_empty() {
            archetypes(|a| {
                get_children_recursive(self.entity, a, children, 0.into());
            });
        }
        if children.is_empty() || self.index == children.len() {
            return None;
        }

        self.index += 1;
        Some(children[self.index - 1])
    }
}

pub struct ChildrenRecursiveIterRef<'a> {
    pub entity: Identifier,
    index: usize,
    children: Rc<RefCell<Vec<(Entity, Depth)>>>,
    archetypes: &'a Archetypes,
}

impl<'a> ChildrenRecursiveIterRef<'a> {
    pub fn new(
        entity: Identifier,
        children_pool: Rc<RefCell<Vec<(Entity, Depth)>>>,
        archetypes: &'a Archetypes,
    ) -> Self {
        children_pool.borrow_mut().clear();

        Self {
            entity,
            index: 0,
            children: children_pool.clone(),
            archetypes,
        }
    }
}

impl Drop for ChildrenRecursiveIterRef<'_> {
    fn drop(&mut self) {
        self.children.borrow_mut().clear();
    }
}

impl<'a> Iterator for ChildrenRecursiveIterRef<'a> {
    type Item = (Entity, Depth);

    fn next(&mut self) -> Option<Self::Item> {
        let children: &mut _ = &mut self.children.borrow_mut();
        if children.is_empty() {
            get_children_recursive(self.entity, self.archetypes, children, 0.into());
        }
        if children.is_empty() || self.index == children.len() {
            return None;
        }

        self.index += 1;
        Some(children[self.index - 1])
    }
}
