use std::{
    any::TypeId,
    cell::{Cell, RefCell, RefMut},
    collections::BTreeSet,
    hash::Hash,
    ops::Range,
    ptr::NonNull,
    rc::Rc,
};

use bevy_ptr::{OwningPtr, Ptr, PtrMut};

use bevy_utils::HashMap;
use egui::TextBuffer;

use crate::{
    archetype::{Archetype, ArchetypeAdd, ArchetypeRow},
    archetypes::{
        Archetypes, MyTypeRegistry, COMPONENT_CAPACITY, ENTITY_ID, RELATIONSHIPS_CAPACITY,
    },
    blob_vec::BlobVec,
    identifier::Identifier,
};

type Identifiers = BTreeSet<Identifier>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct TableRow(pub usize);

impl From<usize> for TableRow {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct TableId(pub usize);

thread_local! {
    static TABLE_ID: Cell<usize> = const { Cell::new(0) };
}

fn table_id() -> TableId {
    let id = TABLE_ID.get();
    TABLE_ID.set(id + 1);
    id.into()
}

impl From<usize> for TableId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

pub struct Storage(pub BlobVec);

impl Storage {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    pub fn push<T: 'static>(&mut self, value: T) {
        OwningPtr::make(value, |v| unsafe { self.0.push(v) })
    }

    pub fn replace_unchecked<T: 'static>(&mut self, index: usize, value: T) {
        OwningPtr::make(value, |v| unsafe { self.0.replace_unchecked(index, v) });
    }

    pub fn replace_unchecked_ptr(&mut self, index: usize, value: OwningPtr) {
        unsafe { self.0.replace_unchecked(index, value) };
    }
}

impl From<BlobVec> for Storage {
    fn from(value: BlobVec) -> Self {
        Self(value)
    }
}

impl Storage {
    pub fn component_mut(&mut self, row: TableRow) -> PtrMut {
        unsafe { self.0.get_checked_mut(row.0) }
    }
    pub fn component(&self, row: TableRow) -> Ptr {
        unsafe { self.0.get_checked(row.0) }
    }
}

pub type StorageCell = Rc<RefCell<Storage>>;

#[derive(Debug, Clone, Copy, Default)]
///Used to avoid aliasing caused by multiple queries trying to access the same table mutably
pub enum BorrowState {
    #[default]
    NotUsed,
    Borrowed,
    BorrowedMut,
}

pub struct Table {
    borrow_state: BorrowState,
    storages: Vec<StorageCell>,
    //maybe replace with IdentifierStripped?
    storage_indices: HashMap<Identifier, usize>,
    components: Identifiers,
    //it's okay to only store id and not generation, as there will never be two entites with the
    //same id
    entity_indices: Vec<u32>,
    registry: Rc<RefCell<MyTypeRegistry>>,
    id: TableId,
    count: usize,
}

impl Hash for Table {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id().hash(state)
    }
}
impl Table {
    pub(crate) fn new(ids: &Identifiers, registry: Rc<RefCell<MyTypeRegistry>>) -> Self {
        let registry_ref = registry.borrow();
        let components: BTreeSet<_> = ids
            .iter()
            .filter(|id| **id != ENTITY_ID)
            .filter(|id| registry_ref.layouts.contains_key(&id.stripped()))
            .copied()
            .collect();
        //SAFETY: creating a blob vec
        let storages: Vec<_> = unsafe {
            components
                .iter()
                //so that we don't create storage for the root archetype
                .flat_map(|id| registry_ref.layouts.get(&id.stripped()).map(|l| (l, id)))
                .map(|(l, id)| {
                    let capacity = if id.is_relationship() {
                        RELATIONSHIPS_CAPACITY
                    } else {
                        COMPONENT_CAPACITY
                    };
                    BlobVec::new(*l, None, capacity)
                })
                .map(|v| Rc::new(RefCell::new(v.into())))
                .collect()
        };
        drop(registry_ref);
        let mut entities = vec![];
        entities.reserve_exact(COMPONENT_CAPACITY);
        Self {
            registry,
            storage_indices: components
                .iter()
                .copied()
                .enumerate()
                .map(|(i, id)| (id, i))
                .collect(),
            entity_indices: entities,
            components,
            storages,
            id: table_id(),
            count: 0,
            borrow_state: BorrowState::default()
        }
    }

    pub fn has_storage_typed<T: 'static>(&self, id: Identifier) -> bool {
        let registry = self.registry.borrow();
        let Some(identifier_by_type) = registry.identifiers.get(&TypeId::of::<T>()) else {
            return false;
        };
        if *identifier_by_type != id {
            return false;
        }
        if !self.has_storage(id) {
            return false;
        };
        true
    }

    pub fn has_storage(&self, id: Identifier) -> bool {
        let Some(storage_id) = self.storage_indices.get(&id) else {
            return false;
        };
        self.storages.get(*storage_id).is_some()
    }

    pub fn push_entity(&mut self, index: u32) -> TableRow {
        self.entity_indices.push(index);
        self.count += 1;
        TableRow(self.count - 1)
    }

    pub fn storage(&self, id: Identifier) -> Option<&StorageCell> {
        self.storages.get(*self.storage_indices.get(&id)?)
    }

    pub fn push_component<T: 'static>(&mut self, component: Identifier, value: T) -> Option<()> {
        let storage = self.storage(component)?;
        //SAFETY: out of bounds checked, correct align checked, everthing else checked
        unsafe {
            OwningPtr::make(value, |p| {
                storage.borrow_mut().0.push(p);
            });
        }
        Some(())
    }

    pub fn push_component_ptr(&mut self, component: Identifier, value: OwningPtr) -> Option<()> {
        let storage = self.storage(component)?;
        //SAFETY: out of bounds checked, correct align checked
        unsafe {
            storage.borrow_mut().0.push(value);
        }
        Some(())
    }

    pub fn remove_forget(&mut self, archetypes: &mut Archetypes, row: TableRow) {
        if self.storages.is_empty() {
            return;
        }
        let row = row.0;
        // for storage in self.storages.iter() {
        //     let mut storage = storage.borrow_mut();
        //     // assert!(row < storage.len());
        //     if row >= storage.len() {
        //         continue;
        //     }
        //     //SAFETY: out of bounds checked
        //     let _ = unsafe { storage.0.swap_remove_and_forget_unchecked(row) };
        // }

        self.count -= 1;
        let removed = self.entity_indices.swap_remove(row);
        let remove_id = if self.entity_indices.is_empty() || row == self.entity_indices.len() {
            removed
        } else {
            self.entity_indices[row]
        };
        archetypes.modify_record_by_index(remove_id, |r| {
            if let Some(r) = r {
                r.table_row = row.into();
            }
        });
    }

    pub fn remove_drop(&mut self, archetypes: &mut Archetypes, row: TableRow) {
        if self.storages.is_empty() && self.count == 0 {
            return;
        }
        let row = row.0;
        for storage in self.storages.iter() {
            let mut storage = storage.borrow_mut();
            assert!(row < storage.len());
            //SAFETY: out of bounds checked
            unsafe { storage.0.swap_remove_and_drop_unchecked(row) };
        }

        self.count -= 1;
        let removed = self.entity_indices.swap_remove(row);
        let remove_id = if self.entity_indices.is_empty() || row == self.entity_indices.len() {
            removed
        } else {
            self.entity_indices[row]
        };
        archetypes.modify_record_by_index(remove_id, |r| {
            if let Some(r) = r {
                r.table_row = row.into();
            }
        });
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.entity_indices.len() == 0
    }

    pub fn id(&self) -> TableId {
        self.id
    }

    pub fn entity_indices(&self) -> &[u32] {
        &self.entity_indices[..]
    }

    pub fn component_id<T: 'static>(&self) -> Option<Identifier> {
        self.registry
            .borrow()
            .identifiers
            .get(&TypeId::of::<T>())
            .cloned()
    }

    pub fn debug_info(&self, archetypes: &Archetypes) -> String {
        let mut components = String::new();
        let mut storages_len = String::new();
        for id in self.components.iter() {
            let debug_name = archetypes.debug_id_name(*id);
            components.push_str(&format!("{}, ", debug_name));
        }
        for (storage_id, index) in &self.storage_indices {
            let component_name = archetypes.debug_id_name(*storage_id);
            let storage = &self.storages[*index];
            let storage = storage.borrow();
            storages_len.push_str(&format!("\n        {}: {},", component_name, storage.len()));
        }
        let components_len = components.len();
        if !components.is_empty() {
            components.delete_char_range((components_len - 2)..(components_len));
        }
        let components = match components.is_empty() {
            true => "".to_string(),
            false => format!("\n    components: {},", components),
        };
        let table_string = format!(
            r"Table({}) {{
    len: {},{}
    storages_len: {{{}
    }},
}}",
            self.id().0,
            self.len(),
            components,
            storages_len,
        );
        table_string
    }

    pub fn debug_components_info(
        &self,
        archetypes: &Archetypes,
        entites_range: Range<usize>,
    ) -> String {
        let min = entites_range.start.max(0);
        let max = entites_range.end.min(self.len());
        let mut entities_info = String::new();
        for id in min..max {
            let entity_id = self.entity_indices[id];
            let record = archetypes
                .record_by_index(entity_id)
                .expect("entities inside archetypes should always be valid");
            let registry = archetypes.type_registry();
            //TODO: replace u32 ids with actual identifiers
            let mut components_info = String::new();
            for (storage_id, index) in &self.storage_indices {
                let component_name = archetypes.debug_id_name(*storage_id);
                let Some(functions) = &registry.functions.get(&storage_id.stripped()) else {
                    continue;
                };
                let storage = &self.storages[*index];
                let storage = storage.borrow();
                let ptr = unsafe { storage.0.get_checked(record.table_row.0).as_ptr() };
                let ptr = unsafe {
                    Ptr::new(
                        NonNull::new(ptr).expect("pointer to component data should NOT be null"),
                    )
                };
                let debug_string = (functions.to_debug_string)(ptr);
                components_info.push_str(&format!("\n    {},", debug_string));
            }
            let entity_name = archetypes.debug_id_name(record.entity);
            let entity_info = format!(
                r"{}: 
{{{}
}}, 
",
                entity_name, components_info
            );
            entities_info.push_str(&entity_info);
        }
        // if entities_info.is_empty() {
        return entities_info;
        // }
        //         format!(
        //             r"{{
        //     {entities_info},
        // }}"
        // )
    }

    pub fn storages(&self) -> &[Rc<RefCell<Storage>>] {
        &self.storages
    }

    pub fn storage_indices(&self) -> &HashMap<Identifier, usize> {
        &self.storage_indices
    }

    pub fn borrow_state(&self) -> BorrowState {
        self.borrow_state
    }

    pub fn borrow_state_mut(&mut self) -> &mut BorrowState {
        &mut self.borrow_state
    }

    pub fn component_ids(&self) -> &Identifiers {
        &self.components
    }
}

impl Table {
    pub fn move_entity(
        archetypes: &mut Archetypes,
        entity: Identifier,
        old_archetype_row: ArchetypeRow,
        old_table_row: TableRow,
        mut new_archetype: RefMut<Archetype>,
        mut old_archetype: RefMut<Archetype>,
    ) -> (ArchetypeRow, TableRow) {
        let (archetype_row, table_row) = {
            let (arhetype_row, table_row) =
                new_archetype.push_entity(entity.low32(), ArchetypeAdd::ArchetypeAndTable);

            let old_table = old_archetype.table();
            let new_table = new_archetype.table();
            let old = old_table.borrow();
            let new = new_table.borrow();
            for (id, old_index) in old.storage_indices.iter() {
                let Some(new_index) = new.storage_indices.get(id) else {
                    continue;
                };
                unsafe {
                    let old_storage = &old.storages[*old_index];
                    let mut old_storage_mut = old_storage.borrow_mut();
                    let value = old_storage_mut
                        .0
                        .swap_remove_and_forget_unchecked(old_table_row.0);
                    let new_storage = &new.storages[*new_index];
                    let mut new_storage_mut = new_storage.borrow_mut();
                    new_storage_mut.0.push(value);
                }
            }
            (arhetype_row, table_row)
        };
        old_archetype.remove_forget(archetypes, old_archetype_row, old_table_row.into());
        (archetype_row, table_row.unwrap())
    }
}

#[cfg(test)]
mod tests {
    use std::alloc::Layout;

    use crate::components::test_components::{Name, Position};

    use super::*;

    #[test]
    fn adding_heap_componets() {
        let registry = Rc::new(RefCell::new(MyTypeRegistry::new()));
        let component = Identifier::from(420);
        registry
            .borrow_mut()
            .add_type_id(TypeId::of::<Name>(), component, "Name");
        registry
            .borrow_mut()
            .layouts
            .insert(component.stripped(), Layout::new::<Name>());
        let components = BTreeSet::from([component]);
        let mut table = Table::new(&components, registry.clone());
        table.push_entity(0);
        table.push_component::<Name>(
            component,
            Name {
                value: "hello world".into(),
            },
        );
        let storage = table.storage(component).unwrap().clone();
        let storage_ref = storage.borrow();
        let name = storage_ref.component(TableRow(0));
        let name: &Name = unsafe { name.deref() };
        assert_eq!(name.value.as_str(), "hello world")
    }

    #[test]
    fn adding_many_componets() {
        let registry = Rc::new(RefCell::new(MyTypeRegistry::new()));
        let component = Identifier::from(420);
        registry
            .borrow_mut()
            .add_type_id(TypeId::of::<Position>(), component, "Position");
        registry
            .borrow_mut()
            .layouts
            .insert(component.stripped(), Layout::new::<Position>());
        let components = BTreeSet::from([component]);
        let mut table = Table::new(&components, registry.clone());
        for i in 0..500 {
            let entity = Identifier::from(i);
            table.push_entity(entity.low32());
            table.push_component::<Position>(component, Position { x: 1, y: 1 });
        }
        let storage = table.storage(component).unwrap().clone();
        let storage_ref = storage.borrow();
        let sum: i32 = (0..500)
            .map(|n| storage_ref.component(n.into()))
            .map(|p| unsafe { *p.deref::<Position>() })
            .map(|p| p.x + p.y)
            .sum();
        assert_eq!(sum, 1000);
    }

    #[test]
    fn adding_componets() {
        let registry = Rc::new(RefCell::new(MyTypeRegistry::new()));
        let component = Identifier::from(420);
        registry
            .borrow_mut()
            .add_type_id(TypeId::of::<Position>(), component, "Position");
        let entity = Identifier::from(0);
        registry
            .borrow_mut()
            .layouts
            .insert(component.stripped(), Layout::new::<Position>());
        let components = BTreeSet::from([component]);
        let mut table = Table::new(&components, registry.clone());
        table.push_entity(entity.low32());
        table.push_component::<Position>(component, Position { x: 10, y: 20 });
        let storage = table.storage(component).unwrap().clone();
        let storage_ref = storage.borrow();
        let pos = storage_ref.component(TableRow(0));
        let pos: &Position = unsafe { pos.deref() };
        assert_eq!(pos.x, 10);
        assert_eq!(pos.y, 20);
    }
}
