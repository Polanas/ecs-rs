use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    hash::Hash,
    rc::Rc,
};
use bevy_ptr::{OwningPtr, Ptr};
use bevy_utils::HashMap;

use crate::{
    archetypes::{
        Archetypes, MyTypeRegistry, NameLeft, COMPONENT_CAPACITY, COMPONENT_ID, ENTITY_ID,
        WILDCARD_25, WILDCARD_32,
    },
    entity::Entity,
    identifier::Identifier,
    table::{StorageCell, Table, TableRow},
    world::archetypes,
};

#[derive(Debug)]
pub enum ArchetypeAdd {
    ArchetypeAndTable,
    ArchetypeOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ArchetypeRow(pub usize);

impl From<usize> for ArchetypeRow {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ArchetypeId(pub usize);

thread_local! {
    static ARCHETYPE_ID: Cell<usize> = const{ Cell::new(0) };
}

fn archetype_id() -> ArchetypeId {
    let id = ARCHETYPE_ID.get();
    ARCHETYPE_ID.set(id + 1);
    id.into()
}

impl From<usize> for ArchetypeId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Edge {
    pub add: Option<ArchetypeId>,
    pub remove: Option<ArchetypeId>,
}

impl Edge {
    pub fn new_add(add: ArchetypeId) -> Self {
        Self {
            add: Some(add),
            remove: None,
        }
    }
}

pub struct Archetype {
    entity_indices: Vec<usize>,
    table: Rc<RefCell<Table>>,
    id: ArchetypeId,
    edges: HashMap<Identifier, Edge>,
    components: BTreeSet<Identifier>,
    components_vec: Vec<Identifier>,
    count: usize,
    registry: Rc<RefCell<MyTypeRegistry>>,
}

impl Archetype {
    pub fn new(
        table: Rc<RefCell<Table>>,
        components: BTreeSet<Identifier>,
        registry: Rc<RefCell<MyTypeRegistry>>,
    ) -> Self {
        let id = archetype_id();
        let mut entities = vec![];
        entities.reserve_exact(COMPONENT_CAPACITY);
        let components_vec: Vec<_> = components.iter().cloned().collect();
        Self {
            entity_indices: entities,
            components,
            components_vec,
            edges: HashMap::new(),
            table,
            id,
            count: 0,
            registry,
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push_entity(
        &mut self,
        index: usize,
        add_type: ArchetypeAdd,
    ) -> (ArchetypeRow, Option<TableRow>) {
        self.count += 1;
        let table_row = if matches!(add_type, ArchetypeAdd::ArchetypeAndTable) {
            Some(self.table().borrow_mut().push_entity(index))
        } else {
            None
        };
        self.entity_indices.push(index);
        (ArchetypeRow(self.count - 1), table_row)
    }

    pub fn remove_forget(
        &mut self,
        archetypes: &mut Archetypes,
        archetype_row: ArchetypeRow,
        table_row: Option<TableRow>,
    ) {
        if let Some(row) = table_row {
            self.table.borrow_mut().remove_forget(archetypes, row);
        }
        let row = archetype_row.0;
        self.count -= 1;
        let removed = self.entity_indices.swap_remove(row);
        let remove_id = if self.entity_indices.is_empty() || row == self.entity_indices.len() {
            removed
        } else {
            self.entity_indices[row]
        };
        archetypes.modify_record_by_index(remove_id, |r| {
            if let Some(r) = r {
                r.archetype_row = row.into();
            }
        });
    }

    pub fn push_component<T: 'static>(&mut self, component: Identifier, value: T) -> Option<()> {
        self.table.borrow_mut().push_component(component, value)
    }

    pub fn push_component_ptr(&mut self, component: Identifier, value: OwningPtr) {
        self.table.borrow_mut().push_component_ptr(component, value);
    }

    pub fn remove_drop(
        &mut self,
        archetypes: &mut Archetypes,
        archetype_row: ArchetypeRow,
        table_row: Option<TableRow>,
    ) {
        if let Some(row) = table_row {
            self.table.borrow_mut().remove_drop(archetypes, row);
        }
        let row = archetype_row.0;
        self.count -= 1;
        let removed = self.entity_indices.swap_remove(row);
        let remove_id = if self.entity_indices.is_empty() || row == self.entity_indices.len() {
            removed
        } else {
            self.entity_indices[row]
        };
        archetypes.modify_record_by_index(remove_id, |r| {
            if let Some(r) = r {
                r.archetype_row = row.into();
            }
        });
    }

    pub fn debug_print(&self, archetypes: &Archetypes) {
        println!("Archetype {:?} {{", self.id.0);
        let registry = self.registry.borrow();
        for component in self.components.iter() {
            if *component == ENTITY_ID {
                println!("    Entity,");
                continue;
            } else if *component == COMPONENT_ID {
                println!("    Component,");
                continue;
            }
            if let Some(name) = archetypes.debug_id_name(*component) {
                println!("    {name},");
                continue;
            }
            let type_name = if let (Some(relation), Some(target)) = (
                archetypes.relation_entity(*component),
                archetypes.target_entity(*component),
            ) {
                let relation_name = archetypes.debug_id_name(relation).unwrap_or_else(|| {
                    registry
                        .type_names
                        .get(&relation.low32())
                        .cloned()
                        .unwrap_or("Relation".to_string())
                });
                let target_name = archetypes.debug_id_name(target).unwrap_or_else(|| {
                    registry
                        .type_names
                        .get(&target.low32())
                        .cloned()
                        .unwrap_or("Target".to_string())
                });
                &format!("({relation_name}, {target_name})")
            } else if let Some(name) = registry.type_names.get(&component.low32()) {
                name
            } else {
                "No name"
            };
            println!("    {},", type_name);
        }
        // println!("    hash: {},", self.components.regular_hash());
        println!("    len: {}\n}}", self.count);
    }

    pub fn storages<T: 'static>(&self) -> Option<StorageCell> {
        let table = self.table.borrow();
        let id = table.component_id::<T>()?;
        table.storage(id).cloned()
    }

    pub fn components_ids_set(&self) -> &BTreeSet<Identifier> {
        &self.components
    }

    pub fn components_ids(&self) -> &[Identifier] {
        &self.components_vec[..]
    }

    pub fn table(&self) -> &Rc<RefCell<Table>> {
        &self.table
    }

    pub fn id(&self) -> ArchetypeId {
        self.id
    }

    pub fn edge_cloned(&mut self, id: Identifier) -> Edge {
        *self.edges.entry(id).or_default()
    }

    pub fn edge_mut(&mut self, id: Identifier) -> &mut Edge {
        self.edges.entry(id).or_default()
    }

    pub fn entity_indices(&self) -> &[usize] {
        &self.entity_indices
    }
}

impl PartialEq for Archetype {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Hash for Archetype {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
