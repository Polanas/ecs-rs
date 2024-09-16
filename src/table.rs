use std::{
    any::TypeId,
    cell::{Cell, RefCell, RefMut},
    collections::BTreeSet,
    hash::Hash,
    rc::Rc,
};

use bevy_ptr::{OwningPtr, Ptr, PtrMut};

use bevy_utils::{dbg, HashMap};

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
    static TABLE_ID: Cell<usize> = Cell::new(0);
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

pub struct Table {
    storages: Vec<StorageCell>,
    storage_indices: HashMap<Identifier, usize>,
    components: Identifiers,
    entity_indices: Vec<usize>,
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
            .filter(|id| registry_ref.layouts.contains_key(*id))
            .copied()
            .collect();
        //SAFETY: creating a blob vec
        let storages: Vec<_> = unsafe {
            components
                .iter()
                //so that we don't create storage for the root archetype
                .flat_map(|id| registry_ref.layouts.get(id).map(|l| (l, id)))
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
        }
    }

    pub fn component_ids(&self) -> &Identifiers {
        &self.components
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

    pub fn push_entity(&mut self, index: usize) -> TableRow {
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
        //SAFETY: out of bounds checked, correct align checked, everthing else checked
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
        for storage in self.storages.iter() {
            let mut storage = storage.borrow_mut();
            // assert!(row < storage.len());
            if row >= storage.len() {
                continue;
            }
            //SAFETY: out of bounds checked
            let _ = unsafe { storage.0.swap_remove_and_forget_unchecked(row) };
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

    pub fn entity_indices(&self) -> &[usize] {
        &self.entity_indices[..]
    }

    pub fn component_id<T: 'static>(&self) -> Option<Identifier> {
        self.registry
            .borrow()
            .identifiers
            .get(&TypeId::of::<T>())
            .cloned()
    }

    pub fn debug_print(&self, archetypes: &Archetypes) {
        println!("Table {:?} {{", self.id.0);
        let registry = self.registry.borrow();
        for component in self.components.iter() {
            let type_name = if let (Some(relation), Some(target)) = (
                archetypes.relation_entity(*component),
                archetypes.target_entity(*component),
            ) {
                let relation_name = registry
                    .type_names
                    .get(&relation.low32())
                    .map(|n| n.as_str())
                    .unwrap_or("Relation");
                let target_name = registry
                    .type_names
                    .get(&target.low32())
                    .map(|n| n.as_str())
                    .unwrap_or("Target");
                &format!("({relation_name}, {target_name})")
            } else if let Some(name) = registry.type_names.get(&component.low32()) {
                name
            } else {
                "No name"
            };
            println!("    {},", type_name);
        }
        // println!("    hash: {},", self.components.table_hash(archetypes));
        println!("    len: {}\n}}", self.count);
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
            let old_table = old_archetype.table();
            let new_table = new_archetype.table().clone();
            let (arhetype_row, table_row) =
                new_archetype.push_entity(entity.low32() as usize, ArchetypeAdd::ArchetypeAndTable);
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
            .add_type_id(TypeId::of::<Name>(), component);
        registry
            .borrow_mut()
            .layouts
            .insert(component, Layout::new::<Name>());
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
            .add_type_id(TypeId::of::<Position>(), component);
        registry
            .borrow_mut()
            .layouts
            .insert(component, Layout::new::<Position>());
        let components = BTreeSet::from([component]);
        let mut table = Table::new(&components, registry.clone());
        for i in 0..500 {
            let entity = Identifier::from(i);
            table.push_entity(entity.low32() as usize);
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
            .add_type_id(TypeId::of::<Position>(), component);
        let entity = Identifier::from(0);
        registry
            .borrow_mut()
            .layouts
            .insert(component, Layout::new::<Position>());
        let components = BTreeSet::from([component]);
        let mut table = Table::new(&components, registry.clone());
        table.push_entity(entity.low32() as usize);
        table.push_component::<Position>(component, Position { x: 10, y: 20 });
        let storage = table.storage(component).unwrap().clone();
        let storage_ref = storage.borrow();
        let pos = storage_ref.component(TableRow(0));
        let pos: &Position = unsafe { pos.deref() };
        assert_eq!(pos.x, 10);
        assert_eq!(pos.y, 20);
    }
}
