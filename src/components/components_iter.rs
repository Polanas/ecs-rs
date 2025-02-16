use std::{cell::RefCell, marker::PhantomData, rc::Rc};

use bevy_reflect::Reflect;

use crate::{
    archetypes::{EntityRecord, MyTypeRegistry}, entity::Entity, identifier::Identifier, wrappers::ArchetypeCell
};

pub struct ComponentsReflectIterMut<'i> {
    record: EntityRecord,
    _marker: &'i PhantomData<()>,
    type_registry: Rc<RefCell<MyTypeRegistry>>,
    archetype: ArchetypeCell,
    index: usize,
}

impl<'i> ComponentsReflectIterMut<'i> {
    pub fn new(
        archetype: ArchetypeCell,
        record: EntityRecord,
        marker: &'i PhantomData<()>,
        type_registry: Rc<RefCell<MyTypeRegistry>>,
    ) -> Self {
        Self {
            record,
            archetype,
            index: 0,
            _marker: marker,
            type_registry,
        }
    }
}

impl<'i> Iterator for ComponentsReflectIterMut<'i> {
    type Item = (Entity, Option<&'i mut dyn Reflect>);

    fn next(&mut self) -> Option<Self::Item> {
        let archetype = self.archetype.borrow();
        let table = archetype.table().borrow();
        let type_registry = self.type_registry.borrow();
        let components = archetype.components_ids();
        if self.index == components.len() {
            return None;
        }
        let component = components[self.index];
        let functions = type_registry.functions.get(&component.stripped())?;
        let mut storage = table.storage(component)?.borrow_mut();
        let ptr = unsafe { storage.0.get_checked_mut(self.record.table_row.0) };
        self.index += 1;
        let reflect_mut = (functions.as_reflect_mut)(ptr);
        let reflect_ref = unsafe {
            reflect_mut.map(|r| &mut *(r as *mut _))
        };
        Some((Entity::new(component), reflect_ref))
    }
}
pub struct ComponentsReflectIter<'i> {
    record: EntityRecord,
    _marker: &'i PhantomData<()>,
    type_registry: Rc<RefCell<MyTypeRegistry>>,
    archetype: ArchetypeCell,
    index: usize,
}

impl<'i> ComponentsReflectIter<'i> {
    pub fn new(
        archetype: ArchetypeCell,
        record: EntityRecord,
        marker: &'i PhantomData<()>,
        type_registry: Rc<RefCell<MyTypeRegistry>>,
    ) -> Self {
        Self {
            record,
            archetype,
            index: 0,
            _marker: marker,
            type_registry,
        }
    }
}

impl<'i> Iterator for ComponentsReflectIter<'i> {
    type Item = (Entity, Option<&'i dyn Reflect>);

    fn next(&mut self) -> Option<Self::Item> {
        let archetype = self.archetype.borrow();
        let table = archetype.table().borrow();
        let type_registry = self.type_registry.borrow();
        let components = archetype.components_ids();
        if self.index == components.len() {
            return None;
        }
        let component = components[self.index];
        let functions = type_registry.functions.get(&component.stripped())?;
        let storage = table.storage(component)?.borrow();
        let ptr = unsafe { storage.0.get_checked(self.record.table_row.0) };
        self.index += 1;
        let reflect_ref = (functions.as_reflect_ref)(ptr);
        let reflect_ref = unsafe {
            reflect_ref.map(|r| &*(r as *const _))
        };
        Some((Entity::new(component), reflect_ref))
    }
}
