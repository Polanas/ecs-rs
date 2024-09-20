use std::{
    alloc::Layout,
    any::{Any, TypeId},
    cell::{Ref, RefCell, RefMut},
    collections::{BTreeSet, VecDeque},
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
};

use anyhow::{bail, Result};
use bevy_ptr::{Ptr, PtrMut};
use bevy_reflect::Reflect;
use bevy_utils::{hashbrown::HashMap, HashSet};
use bimap::BiHashMap;
use packed_struct::PackedStruct;
use smol_str::{format_smolstr, SmolStr, ToSmolStr};
use thiserror::Error;

use crate::{
    archetype::{Archetype, ArchetypeAdd, ArchetypeId, ArchetypeRow},
    children_iter::{self, ChildrenRecursiveIterRef, Depth},
    components::{
        component::{AbstractComponent, ChildOf, EnumTag},
        component_hash::ComponentsHash,
        temp_components::TempComponentsStorage,
    },
    entity::{Entity, WILDCARD},
    filter_mask::FilterMask,
    identifier::{Identifier, IdentifierHigh32, IdentifierUnpacked, WildcardKind},
    on_change_callbacks::{OnAddCallback, OnChangeCallbacks, OnRemoveCallback},
    query::RequiredIds,
    relationship::FindRelationshipsIter,
    systems::{EnumId, Systems},
    table::{Storage, Table, TableRow},
    world::{archetypes, archetypes_mut},
    wrappers::{ArchetypeCell, TableCell},
};
pub const TEMP_CAPACITY: usize = 32;
pub const COMPONENT_CAPACITY: usize = 256;
pub const RELATIONSHIPS_CAPACITY: usize = 8;
pub const ENTITY_ID: Identifier = Identifier([0; 8]);
pub const COMPONENT_ID: Identifier = Identifier([1, 0, 0, 0, 0, 0, 0, 0]);
pub const WILDCARD_32: u32 = u32::MAX;
pub const WILDCARD_25: u32 = u32::MAX >> 7;
pub const ENTITIES_START_CAPACITY: usize = 512;
//max low32, max high32, is_relationship
pub const WILDCARD_RELATIONSHIP: Identifier = Identifier([255, 255, 255, 255, 255, 255, 255, 129]);

#[derive(Debug, Clone, Error)]
pub enum ComponentTypeError {
    #[error("Expected entity {0} to be alive")]
    EntityNotAlive(SmolStr),
    #[error("Expected entity {0} to be a component")]
    EntityNotComponent(SmolStr),
    #[error("Expected tag relationship {0} to have an existing target")]
    NoTargetInTagRelationship(SmolStr),
    #[error("Expected component {0} to be a relationship by exclusion")]
    EntityNotRelationship(SmolStr),
    #[error("Expected relationship {0} to have both a relation and target")]
    NoRelationOrTarget(SmolStr),
}
#[derive(Debug, Clone, Copy)]
pub enum RelationshipDataPosition {
    First,
    Second,
}

#[derive(Debug, Clone, Copy)]
pub enum ComponentType {
    RegularComponent,
    ComponentTag,
    EntityTag,
    EnumTag,
    RelationshipComponentTag,
    MixedRelationshipTag,
    DataRelationship(RelationshipDataPosition),
}
#[derive(Debug)]
pub enum OperationType {
    AddComponent {
        component_id: Identifier,
        table_reusage: TableReusage,
    },
    RemoveComponent(Identifier),
    RemoveEntity,
}

#[derive(Debug)]
pub struct ArchetypeOperation {
    entity: Identifier,
    op_type: OperationType,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct StrippedIdentifier(Identifier);

impl From<Identifier> for StrippedIdentifier {
    fn from(value: Identifier) -> Self {
        let unpacked = value.unpack();
        Self(
            IdentifierUnpacked {
                low32: unpacked.low32,
                high32: IdentifierHigh32 {
                    second: unpacked.high32.second,
                    is_relationship: unpacked.high32.is_relationship,
                    ..Default::default()
                },
            }
            .pack()
            .unwrap()
            .into(),
        )
    }
}

impl StrippedIdentifier {
    pub fn low32(&self) -> u32 {
        self.0.low32()
    }
}
type CloneFn = fn(Ptr<'_>, RefMut<Storage>);
type SerializeFn = fn(Ptr<'_>) -> serde_json::Result<serde_json::Value>;
type DeserializeFn = fn(serde_json::Value, RefMut<Storage>) -> serde_json::Result<()>;
type AsReflectRefFn = fn(Ptr<'_>, f: &dyn Fn(Option<&dyn Reflect>));
type AsReflectMutFn = fn(PtrMut<'_>, f: &dyn Fn(Option<&mut dyn Reflect>));

pub struct Functions {
    clone: CloneFn,
    serialize: SerializeFn,
    deserialize: DeserializeFn,
    as_reflect_ref: AsReflectRefFn,
    as_reflect_mut: AsReflectMutFn,
}

pub struct MyTypeRegistry {
    pub layouts: HashMap<StrippedIdentifier, Layout>,
    pub functions: HashMap<Identifier, Functions>,
    pub type_ids_data: HashMap<StrippedIdentifier, (TypeId, SmolStr)>,
    pub identifiers: HashMap<TypeId, Identifier>,
    pub identifiers_by_names: HashMap<SmolStr, Identifier>,
    pub components: HashSet<Identifier>,
    pub tags: HashSet<StrippedIdentifier>,
}

pub enum ComponentAddState {
    New,
    AlreadyExisted,
}

impl_component! {
    #[derive(Debug)]
    pub struct Component {
        pub size: Option<usize>,
        pub is_type: bool,
    }
}
impl_component! {
    pub struct Wildcard {}
}
impl_component! {
    #[derive(Copy)]
    pub struct EnumTagId(pub EnumId);
}
impl_component! {
    pub struct Prefab {}
}
impl_component! {
    pub struct InstanceOf {}
}

#[derive(Debug)]
pub enum EntityKind {
    Regular,
    Component(Component),
}

#[derive(Debug, Clone, Copy)]
pub enum TableReusage {
    Reuse,
    New,
}

type Records = Rc<RefCell<Vec<Option<EntityRecord>>>>;
type NamesMap = BiHashMap<NameLeft, NameRight>;

pub struct EntityNameGetter {
    entity: NameLeft,
}

impl EntityNameGetter {
    pub fn new(entity: NameLeft) -> Self {
        Self { entity }
    }

    pub fn get<F, U>(&self, f: F) -> U
    where
        F: FnOnce(&str) -> U,
    {
        archetypes(|a| f(a.name_by_entity(self.entity).unwrap()))
    }

    pub fn set(&self, name: &str) {
        archetypes_mut(|a| a.set_entity_name(self.entity, name.into()));
    }

    pub fn set_fn<F>(&self, f: F)
    where
        F: FnOnce() -> String,
    {
        archetypes_mut(|a| {
            a.set_entity_name(self.entity, f().into());
        });
    }
}

pub struct ComponentGetter<T: AbstractComponent> {
    phantom_data: PhantomData<T>,
    component: Identifier,
    record_index: usize,
    records: Records,
    table: TableCell,
}

impl<T: AbstractComponent> ComponentGetter<T> {
    pub fn new(entity: Identifier, component: Identifier, archetypes: &Archetypes) -> Option<Self> {
        let record = archetypes.record(entity)?;
        let archetype = archetypes.archetype_from_record(&record)?;
        let table = archetype.borrow().table().clone();

        Some(Self {
            phantom_data: PhantomData,
            records: archetypes.records.clone(),
            component,
            record_index: entity.low32() as usize,
            table: table.clone().into(),
        })
    }

    pub fn get<F, U>(&self, f: F) -> U
    where
        F: FnOnce(&T) -> U,
    {
        f(self.get_component())
    }

    pub fn get_mut<F, U>(&mut self, f: F) -> U
    where
        F: FnOnce(&mut T) -> U,
    {
        f(self.get_component_mut())
    }

    fn get_component(&self) -> &T {
        let records = self.records.borrow();
        let record = &records[self.record_index].unwrap();
        let table = self.table.borrow();
        let storage = table.storage(self.component).unwrap();
        let storage_mut = storage.borrow();
        let component = storage_mut.component(record.table_row);
        //ooo spooky
        unsafe { &*(component.as_ptr() as *mut T) }
    }

    fn get_component_mut(&mut self) -> &mut T {
        let records = self.records.borrow();
        let record = &records[self.record_index].unwrap();
        let table = self.table.borrow();
        let storage = table.storage(self.component).unwrap();
        let storage_mut = storage.borrow_mut();
        let component = storage_mut.component(record.table_row);
        //ooo spooky
        unsafe { &mut *(component.as_ptr() as *mut T) }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EntityRecord {
    pub archetype_row: ArchetypeRow,
    pub table_row: TableRow,
    pub arhetype_id: ArchetypeId,
    pub entity: Identifier,
}

impl MyTypeRegistry {
    pub fn new() -> Self {
        Self {
            type_ids_data: HashMap::new(),
            identifiers: HashMap::new(),
            components: HashSet::new(),
            tags: HashSet::new(),
            layouts: HashMap::new(),
            functions: HashMap::new(),
            identifiers_by_names: HashMap::new(),
        }
    }

    pub fn add_type_id(&mut self, type_id: TypeId, id: Identifier, name: &str) {
        self.identifiers.insert(type_id, id);
        self.type_ids_data
            .insert(id.stripped(), (type_id, name.to_smolstr()));
        self.identifiers_by_names.insert(name.to_smolstr(), id);
    }
}

impl Default for MyTypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

type ArchetypeVec = Vec<ArchetypeCell>;
type ArchetypeSet = HashSet<ArchetypeCell>;

type TableVec = Vec<TableCell>;

pub struct QueryStorage {
    pub archetypes: Vec<ArchetypeCell>,
    pub mask: FilterMask,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NameRight {
    name: SmolStr,
    parent_id: usize,
}

impl From<(String, Identifier)> for NameRight {
    fn from(value: (String, Identifier)) -> Self {
        Self {
            name: value.0.to_smolstr(),
            parent_id: value.1.low32() as usize,
        }
    }
}

impl NameRight {
    pub fn new(name: SmolStr, parent_id: usize) -> Self {
        Self { name, parent_id }
    }
    pub fn to_entity_and_parent(&self, entity: usize) -> NameLeft {
        NameLeft::new(entity, self.parent_id)
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NameLeft {
    entity_index: usize,
    parent_index: usize,
}

impl From<(Identifier, Identifier)> for NameLeft {
    fn from(value: (Identifier, Identifier)) -> Self {
        Self {
            entity_index: value.0.low32() as usize,
            parent_index: value.1.low32() as usize,
        }
    }
}

impl NameLeft {
    pub fn from_ids(entity_id: Identifier, parent_id: Identifier) -> Self {
        Self {
            entity_index: entity_id.low32() as usize,
            parent_index: parent_id.low32() as usize,
        }
    }
    pub fn new(entity_index: usize, parent_index: usize) -> Self {
        Self {
            entity_index,
            parent_index,
        }
    }
    pub fn to_name_and_parent(&self, name: SmolStr) -> NameRight {
        NameRight::new(name, self.parent_index)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct UniqueName {
    parent_index: usize,
    name: SmolStr,
}

impl UniqueName {
    pub fn new(parent_index: usize, name: SmolStr) -> Self {
        Self { parent_index, name }
    }
    pub fn from_ids(parent: Identifier, name: SmolStr) -> Self {
        Self {
            parent_index: parent.low32() as usize,
            name,
        }
    }
}

pub struct StateOperation {
    pub type_id: TypeId,
    pub state_id: EnumId,
    pub state: Rc<RefCell<dyn Any>>,
}

pub type Resources = HashMap<TypeId, Rc<RefCell<dyn Any>>>;
type Operations = Vec<ArchetypeOperation>;
type Storages = HashMap<u64, Rc<RefCell<QueryStorage>>>;

pub struct Archetypes {
    query_storages: Storages,
    records: Records,
    type_registry: Rc<RefCell<MyTypeRegistry>>,
    archetypes: Vec<ArchetypeCell>,
    archetypes_by_hashes: HashMap<u64, ArchetypeVec>,
    tables_by_hashes: HashMap<u64, TableVec>,
    names: NamesMap,
    unique_names: HashSet<UniqueName>,
    archetypes_by_ids: HashMap<Identifier, ArchetypeSet>,
    unused_ids: VecDeque<Identifier>,
    entity_id: u32,
    children_pool: Rc<RefCell<Vec<(Entity, Depth)>>>,
    entities_pool: Rc<RefCell<Vec<Identifier>>>,
    operations: Rc<RefCell<Operations>>,
    operatoins_pool: Rc<RefCell<Operations>>,
    resources: Rc<RefCell<Resources>>,
    locked: bool,
    locked_depth: u32,
    systems: Rc<RefCell<Systems>>,
    temp_components: TempComponentsStorage,
    callbacks: Rc<RefCell<OnChangeCallbacks>>,
    state_operations: Rc<RefCell<Vec<StateOperation>>>,
}

impl Archetypes {
    pub fn new() -> Self {
        let mut archetypes = Self {
            records: RefCell::new(vec![None; ENTITIES_START_CAPACITY]).into(),
            archetypes: vec![],
            type_registry: Rc::new(MyTypeRegistry::new().into()),
            archetypes_by_ids: HashMap::new(),
            archetypes_by_hashes: HashMap::new(),
            tables_by_hashes: HashMap::new(),
            unused_ids: VecDeque::new(),
            entity_id: 0,
            query_storages: HashMap::new(),
            names: BiHashMap::new(),
            children_pool: RefCell::new(vec![]).into(),
            operations: RefCell::new(vec![]).into(),
            locked: false,
            locked_depth: 0,
            operatoins_pool: RefCell::new(vec![]).into(),
            entities_pool: RefCell::new(vec![]).into(),
            resources: RefCell::new(HashMap::new()).into(),
            unique_names: HashSet::new(),
            systems: RefCell::new(Systems::new()).into(),
            temp_components: TempComponentsStorage::new(),
            callbacks: RefCell::new(OnChangeCallbacks::new()).into(),
            state_operations: RefCell::new(vec![]).into(),
        };
        {
            let mut registry = archetypes.type_registry.borrow_mut();
            registry
                .layouts
                .insert(ENTITY_ID.stripped(), Layout::new::<Entity>());
            registry
                .layouts
                .insert(COMPONENT_ID.stripped(), Layout::new::<Component>());
            registry.add_type_id(TypeId::of::<()>(), ENTITY_ID, "Entity");
            registry.add_type_id(TypeId::of::<Component>(), COMPONENT_ID, "Component");
            registry.add_type_id(TypeId::of::<Wildcard>(), WILDCARD_RELATIONSHIP, "Wildcard");
        }
        let mut entity_archetype_components = BTreeSet::new();
        entity_archetype_components.insert(ENTITY_ID);
        let table = Table::new(
            &entity_archetype_components,
            archetypes.type_registry.clone(),
        )
        .into();
        archetypes.add_archetype(&table, &entity_archetype_components);
        archetypes.register_component::<InstanceOf>();
        archetypes.register_component::<EnumTagId>();
        archetypes.register_component::<ChildOf>();
        archetypes.register_component::<Prefab>();
        archetypes
    }

    pub fn systems(&mut self) -> &Rc<RefCell<Systems>> {
        &self.systems
    }

    pub fn insert_add_callback(&mut self, component: Identifier, callback: Box<dyn OnAddCallback>) {
        self.callbacks
            .borrow_mut()
            .insert_add_callback(component, callback);
    }

    pub fn insert_remove_callback(
        &mut self,
        component: Identifier,
        callback: Box<dyn OnRemoveCallback>,
    ) {
        self.callbacks
            .borrow_mut()
            .insert_remove_callback(component, callback);
    }

    pub fn debug_print_entities(&self) {
        let records = self.records.borrow();
        for record in records.iter().flatten() {
            let name = self.debug_id_name(record.entity).unwrap_or("Entity".into());
            println!("id: {}, name: {},", record.entity.low32(), name);
        }
    }

    pub fn lock(&mut self) {
        self.locked_depth += 1;
        self.locked = true;
    }

    pub fn unlock(&mut self) {
        self.locked_depth = u32::max(0, self.locked_depth - 1);
        if self.locked_depth > 0 {
            return;
        }

        self.locked = false;

        for operation in self.operations.clone().borrow_mut().drain(..) {
            if !self.is_entity_alive(operation.entity) {
                continue;
            }

            match operation.op_type {
                OperationType::AddComponent {
                    component_id,
                    table_reusage,
                } => {
                    let table_row = self.record(operation.entity).unwrap().table_row.0;
                    let (archetype, add_state) = self
                        .add_component(component_id, operation.entity, table_reusage)
                        .unwrap();
                    let component = self.temp_components.remove_comp(component_id);

                    let mut archetype = archetype.borrow_mut();
                    match add_state {
                        ComponentAddState::New => {
                            archetype.push_component_ptr(component_id, component);
                        }
                        ComponentAddState::AlreadyExisted => {
                            let table_mut = archetype.table().borrow_mut();
                            let mut storage = table_mut.storage(component_id).unwrap().borrow_mut();
                            storage.replace_unchecked_ptr(table_row, component);
                        }
                    }
                }
                OperationType::RemoveComponent(component) => {
                    let table_reusage = if self.is_component_empty(component) {
                        TableReusage::Reuse
                    } else {
                        TableReusage::New
                    };
                    self.remove_component(component, operation.entity, table_reusage)
                        .unwrap();
                }
                OperationType::RemoveEntity => {
                    let pool = self.entities_pool.clone();
                    let pool: &mut _ = &mut pool.borrow_mut();
                    self.remove_entity(operation.entity, 0.into(), pool)
                        .unwrap();
                }
            }
        }
    }

    pub fn entities_pool_rc(&self) -> &Rc<RefCell<Vec<Identifier>>> {
        &self.entities_pool
    }

    pub fn id_by_record_index(&self, index: usize) -> Option<Identifier> {
        self.record_by_index(index).map(|r| r.entity)
    }

    pub fn debug_id_name(&self, id: Identifier) -> Option<SmolStr> {
        let record = self.record(id)?;
        let parent = {
            let child_of_rel = self.find_rels::<ChildOf, Wildcard>(&record).unwrap().next();
            if let Some(child_of_rel) = child_of_rel {
                self.target_entity(child_of_rel.0).unwrap()
            } else {
                WILDCARD.0
            }
        };
        if let Some(name) = self.name_by_entity(NameLeft {
            entity_index: id.low32() as _,
            parent_index: parent.low32() as _,
        }) {
            return Some(name.clone());
        }
        if self.is_id_component(id) {
            Some(self.debug_component_name(id)?)
        } else {
            Some(u64::from(id).to_smolstr())
        }
    }

    pub fn debug_component_name(&self, id: Identifier) -> Option<SmolStr> {
        use ComponentType as CT;
        match self.component_type(id).ok()? {
            CT::RegularComponent | ComponentType::ComponentTag => {
                Some(self.type_registry().type_ids_data[&id.stripped()].1.clone())
            }
            CT::EntityTag => Some(u64::from(id).to_smolstr()),
            CT::EnumTag => Some("TODO".to_smolstr()),
            CT::RelationshipComponentTag | CT::MixedRelationshipTag | CT::DataRelationship(_) => {
                self.relationship_component_name(id)
            }
        }
    }

    pub fn relationship_component_name(&self, id: Identifier) -> Option<SmolStr> {
        let (Some(relation), Some(target)) = (self.relation_entity(id), self.target_entity(id))
        else {
            return None;
        };
        Some(format_smolstr!(
            "({0}, {1})",
            self.debug_id_name(relation)?,
            self.debug_id_name(target)?
        ))
    }

    pub fn is_id_component(&self, id: Identifier) -> bool {
        self.has_component(COMPONENT_ID, id)
    }

    pub fn component_type(
        &self,
        component: Identifier,
    ) -> std::result::Result<ComponentType, ComponentTypeError> {
        let component_unpacked = component.unpack();
        if !self.is_entity_alive(component) {
            return Err(ComponentTypeError::EntityNotAlive("TODO".to_smolstr()));
        }
        if !self.has_component(COMPONENT_ID, component) {
            return Err(ComponentTypeError::EntityNotComponent("TODO".to_smolstr()));
        }
        let component_comp = self
            .get_component::<Component>(COMPONENT_ID, component)
            .unwrap()
            .get(|c| c.clone());
        if component_comp.size.map(|s| s > 0).unwrap_or(false)
            && !component_unpacked.high32.is_relationship
        {
            return Ok(ComponentType::RegularComponent);
        }
        if self.type_registry().tags.contains(&component.stripped()) {
            if component_unpacked.high32.is_relationship {
                let Some(target) = self.target_entity(component) else {
                    return Err(ComponentTypeError::NoTargetInTagRelationship(
                        "TODO".to_smolstr(),
                    ));
                };
                if self
                    .get_component::<Component>(COMPONENT_ID, target)
                    .is_some()
                {
                    return Ok(ComponentType::RelationshipComponentTag);
                }
                return Ok(ComponentType::MixedRelationshipTag);
            }
            if component_comp.is_type {
                return Ok(ComponentType::ComponentTag);
            }
            return Ok(ComponentType::EntityTag);
        }
        if !component_unpacked.high32.is_relationship {
            return Err(ComponentTypeError::EntityNotRelationship(
                "TODO".to_smolstr(),
            ));
        }
        let (Some(relation), Some(target)) = (
            self.relation_entity(component),
            self.target_entity(component),
        ) else {
            return Err(ComponentTypeError::NoRelationOrTarget("TODO".to_smolstr()));
        };
        //NOTE: equal entities might have different flags.
        //In this case, target has is_target flag, while the component id does not, as the
        //relationship was created after the component id was set.
        //Conclustion: while comparing components they should be stripped IF they might be parts of
        //a relationship
        if target.stripped() == self.component_id::<EnumTagId>().stripped() {
            return Ok(ComponentType::EnumTag);
        }
        if self
            .type_registry()
            .layouts
            .contains_key(&relation.stripped())
        {
            return Ok(ComponentType::DataRelationship(
                RelationshipDataPosition::First,
            ));
        }

        Ok(ComponentType::DataRelationship(
            RelationshipDataPosition::Second,
        ))
    }

    //Component variants for serialization
    //1) Regular components
    //2) Component tags |
    //                  | - combined into tags
    //3) Entity tags    |
    //4) Non-data relations
    //5) Data relations (2 variants)
    //6) Enum tags
    //TODO: add enum tags support, and maaybe introduce named entities which are always present, e.g.
    //#MainCamera
    pub fn serialize_entity(&self, entity: Identifier) -> Option<String> {
        let registry = self.type_registry.clone();
        let registry_ref = registry.borrow();
        let record = self.record(entity)?;
        let archetype = self.archetype_by_id(record.arhetype_id).clone();
        let archetype_ref = archetype.borrow();
        let components = archetype_ref.components_ids_set_rc().clone();

        let mut json_value = serde_json::json!({});
        let mut tags = serde_json::json!([]);

        for component in components.iter().copied() {
            use ComponentType as CT;
            let debug_name = self.debug_id_name(component).unwrap().to_string();
            match self.component_type(component).unwrap() {
                CT::DataRelationship(data_pos) => {
                    let serialize = registry_ref.functions.get(&component).unwrap().serialize;
                    let storage = archetype_ref
                        .table()
                        .borrow()
                        .storage(component)
                        .unwrap()
                        .clone();
                    let storage_mut = storage.borrow_mut();
                    let component_ptr: *mut u8 =
                        unsafe { storage_mut.0.get_checked(record.table_row.0).as_ptr() };

                    let component_value =
                        serialize(unsafe { Ptr::new(NonNull::new(component_ptr).unwrap()) })
                            .unwrap();
                    let insertion_pos = match data_pos {
                        RelationshipDataPosition::First => 1,
                        RelationshipDataPosition::Second => debug_name.find(',').unwrap() + 2,
                    };
                    let debug_name = {
                        let mut name = debug_name.clone();
                        name.insert(insertion_pos, '$');
                        name
                    };
                    let _ = json_value
                        .as_object_mut()
                        .unwrap()
                        .insert(debug_name, component_value);
                }
                CT::RegularComponent => {
                    let serialize = registry_ref.functions.get(&component).unwrap().serialize;
                    let storage = archetype_ref
                        .table()
                        .borrow()
                        .storage(component)
                        .unwrap()
                        .clone();
                    let storage_mut = storage.borrow_mut();
                    let component_ptr: *mut u8 =
                        unsafe { storage_mut.0.get_checked(record.table_row.0).as_ptr() };

                    let component_value =
                        serialize(unsafe { Ptr::new(NonNull::new(component_ptr).unwrap()) })
                            .unwrap();
                    let _ = json_value
                        .as_object_mut()
                        .unwrap()
                        .insert(debug_name, component_value);
                }
                CT::ComponentTag => tags.as_array_mut().unwrap().push(debug_name.into()),
                CT::EntityTag => {
                    //Do nothing, as entity tags are not garanteed to be persent over multiple
                    //sessions
                }
                CT::MixedRelationshipTag => {
                    //Do nothing for the same reason as above
                }
                CT::EnumTag => tags.as_array_mut().unwrap().push("TODO".into()),
                CT::RelationshipComponentTag => {
                    tags.as_array_mut().unwrap().push(debug_name.into())
                }
            }
        }
        if !tags.as_array().unwrap().is_empty() {
            json_value
                .as_object_mut()
                .unwrap()
                .insert("Tags".into(), tags);
        }
        Some(serde_json::to_string_pretty(&json_value).unwrap())
    }

    pub fn deserialize_entity(&mut self, json: &str) -> Entity {
        let entity = self.add_entity(EntityKind::Regular);
        let registry = self.type_registry.clone();
        let registry_ref = registry.borrow();
        let record = self.record(entity).unwrap();
        let archetype = self.archetype_by_id(record.arhetype_id).clone();
        let archetype_ref = archetype.borrow();
        let mut json_value = serde_json::from_str::<serde_json::Value>(json).unwrap();
        let json_object = json_value.as_object_mut().unwrap();
        for (name, value) in json_object {
            if name == "Tags" {
                let tags = value.as_array().unwrap();
                for tag in tags.iter() {
                    if let Some(tag) = registry_ref
                        .identifiers_by_names
                        .get(&tag.as_str().unwrap().to_smolstr())
                    {
                        self.add_entity_tag(entity, *tag).unwrap();
                    }

                }
            }
        }
        todo!();
        // for component in components.iter().copied() {
        //     let deserialize = registry_ref.functions.get(&component).unwrap().deserialize;
        //     let (new_archetype, _) = self
        //         .add_component(component, entity, table_reusage)
        //         .ok()?;
        //     let storage = archetype_ref
        //         .table()
        //         .borrow()
        //         .storage(component)
        //         .unwrap()
        //         .clone();
        //     let old_storage_mut = storage.borrow_mut();
        //     let component_ptr: *mut u8 =
        //         unsafe { old_storage_mut.0.get_checked(record.table_row.0).as_ptr() };
        //
        //     let component_value =
        //         serialize(unsafe { Ptr::new(NonNull::new(component_ptr).unwrap()) }).unwrap();
        //     let _ = json_value.as_object_mut().unwrap().insert(
        //         registry_ref.type_names[&component.low32()].clone(),
        //         component_value,
        //     );
        // }
    }

    pub fn clone_entity(&mut self, entity: Identifier) -> Option<Identifier> {
        let cloned_entity = self.add_entity(EntityKind::Regular);
        let old_record = self.record(entity)?;
        let old_archetype = self.archetype_by_id(old_record.arhetype_id).clone();
        let old_archetype_ref = old_archetype.borrow();
        let registry = self.type_registry.clone();
        let registry_ref = registry.borrow();
        let components = old_archetype_ref.components_ids_set_rc().clone();
        drop(old_archetype_ref);

        for component in components.iter().copied() {
            let table_reusage = if self.is_component_empty(component) {
                TableReusage::Reuse
            } else {
                TableReusage::New
            };
            let (cloned_archetype, _) = self
                .add_component(component, cloned_entity, table_reusage)
                .ok()?;

            if matches!(table_reusage, TableReusage::Reuse) {
                continue;
            }

            let old_archetype_ref = old_archetype.borrow();
            let cloned_archetype_ref = cloned_archetype.borrow();
            let clone_into = registry_ref.functions.get(&component).unwrap().clone;
            let old_storage = old_archetype_ref
                .table()
                .borrow()
                .storage(component)
                .unwrap()
                .clone();
            let old_storage_mut = old_storage.borrow_mut();
            let component_ptr: *mut u8 = unsafe {
                old_storage_mut
                    .0
                    .get_checked(old_record.table_row.0)
                    .as_ptr()
            };
            let cloned_storage = cloned_archetype_ref
                .table()
                .borrow()
                .storage(component)
                .unwrap()
                .clone();
            if Rc::ptr_eq(&cloned_storage, &old_storage) {
                clone_into(
                    unsafe { Ptr::new(NonNull::new(component_ptr).unwrap()) },
                    old_storage_mut,
                );
            } else {
                let cloned_storage_mut = cloned_storage.borrow_mut();
                clone_into(
                    unsafe { Ptr::new(NonNull::new(component_ptr).unwrap()) },
                    cloned_storage_mut,
                );
            }
            //TODO: should add callbacks fire when cloning entities?
            // self.callbacks
            //     .borrow_mut()
            //     .run_add_callback(component, cloned_entity);
        }
        Some(cloned_entity)
    }

    pub fn resource_exists<T: 'static>(&self) -> bool {
        self.resources.borrow().contains_key(&TypeId::of::<T>())
    }

    pub fn add_resource<T: 'static>(&self, resource: T) {
        let type_id = TypeId::of::<T>();
        let resource: Rc<RefCell<dyn Any>> = Rc::new(RefCell::new(resource));
        self.resources.borrow_mut().insert(type_id, resource);
    }

    pub fn remove_resource<T: 'static>(&mut self) {
        self.resources.borrow_mut().remove(&TypeId::of::<T>());
    }

    pub fn names(&self) -> &NamesMap {
        &self.names
    }

    pub fn set_entity_name(&mut self, left: NameLeft, name: SmolStr) {
        let unique_name = UniqueName::new(left.parent_index, name.clone());
        if self.unique_names.contains(&unique_name) {
            panic!("attempt to add an existing name '{}'", name,);
        }
        self.unique_names.insert(unique_name);
        self.names.insert(left, left.to_name_and_parent(name));
    }

    pub fn change_entity_name(&mut self, left: NameLeft, name: SmolStr) {
        let old_name = self.names.get_by_left(&left).map(|r| r.name.clone());
        if let Some(old_name) = old_name {
            let old_unique_name = UniqueName::new(left.parent_index, old_name.clone());
            let new_unique_name = UniqueName::new(left.parent_index, old_name.clone());
            self.unique_names.remove(&old_unique_name);
            self.unique_names.insert(new_unique_name);
        }
        self.names.insert(left, left.to_name_and_parent(name));
    }

    pub fn remove_entity_name(&mut self, left: NameLeft) {
        let name = self.names.get_by_left(&left).map(|r| r.name.clone());
        if let Some(name) = name {
            let unique_name = UniqueName::new(left.parent_index, name.clone());
            self.unique_names.remove(&unique_name);
        }
        self.names.remove_by_left(&left);
    }

    pub fn set_entity_name_parent(&mut self, left: NameLeft, parent: Identifier) {
        let name = self.names.get_by_left(&left).map(|r| r.name.clone());
        if let Some(name) = name {
            let old_unique_name = UniqueName::new(left.parent_index, name.clone());
            let new_unique_name = UniqueName::new(parent.low32() as usize, name.clone());
            self.unique_names.remove(&old_unique_name);
            self.unique_names.insert(new_unique_name);
        }
        if self.name_by_entity(left).is_some() {
            let (_, old_right) = self.names.remove_by_left(&left).unwrap();
            let entity = NameLeft::new(left.entity_index, parent.low32() as usize);
            self.names
                .insert(entity, entity.to_name_and_parent(old_right.name));
        }
    }

    pub fn entity_has_name(&self, entity: NameLeft) -> bool {
        self.names.contains_left(&entity)
    }

    pub fn name_by_entity(&self, entity: NameLeft) -> Option<&SmolStr> {
        self.names.get_by_left(&entity).map(|n| &n.name)
    }

    pub fn entity_by_name(&self, name: &str, parent: Identifier) -> Option<NameLeft> {
        self.names
            .get_by_right(&NameRight::new(name.to_smolstr(), parent.low32() as usize))
            .cloned()
    }

    pub fn is_component_empty(&self, component: Identifier) -> bool {
        !self
            .type_registry()
            .layouts
            .contains_key(&component.stripped())
    }

    pub fn type_registry(&self) -> Ref<MyTypeRegistry> {
        self.type_registry.borrow()
    }

    pub fn record_by_index(&self, index: usize) -> Ref<Option<EntityRecord>> {
        let records = self.records.borrow();
        Ref::map(records, |r| &r[index])
    }

    pub fn record(&self, entity: Identifier) -> Option<EntityRecord> {
        let records = self.records.borrow();
        let low32 = entity.low32();
        if low32 as usize >= records.len() {
            return None;
        }
        records[low32 as usize]
    }

    pub fn archetype_from_record(&self, record: &EntityRecord) -> Option<&ArchetypeCell> {
        self.archetypes.get(record.arhetype_id.0)
    }

    pub fn entity_id(&mut self) -> Identifier {
        if self.unused_ids.is_empty() {
            let id = IdentifierUnpacked {
                low32: self.entity_id,
                high32: IdentifierHigh32 {
                    is_active: true,
                    ..Default::default()
                },
            }
            .pack()
            .unwrap();
            self.entity_id += 1;
            return id.into();
        }

        let mut id = self.unused_ids.pop_back().unwrap();
        id.set_second(id.second() + 1);
        id
    }

    pub fn relation_entity(&self, relationship: Identifier) -> Option<Identifier> {
        self.record(relationship).map(|record| record.entity)
    }

    pub fn target_entity(&self, relationship: Identifier) -> Option<Identifier> {
        let target = relationship.unpack().high32.second;
        let id = IdentifierUnpacked {
            low32: target.into(),
            ..Default::default()
        };
        self.record(id.into()).map(|record| record.entity)
    }

    pub fn debug_print_tables(&self) {
        let mut tables: Vec<_> = self
            .archetypes
            .iter()
            .map(|a| a.borrow().table().clone())
            .collect();
        tables.sort_by_key(|a| a.borrow().id());
        Vec::dedup_by(&mut tables, |a, b| a.borrow().id() == b.borrow().id());
        for table in &tables {
            table.borrow().debug_print(self);
        }
    }

    pub fn relationship_name(&self, relationship: Identifier) -> Option<String> {
        todo!()
        // let relation = self.relation_entity(relationship)?;
        // let target = self.target_entity(relationship)?;
        // let registry = self.type_registry();
        // let relation_name = registry
        //     .type_names
        //     .get(&relation.low32())
        //     .map(|n| n.as_str())
        //     .unwrap_or("Relation");
        // let target_name = registry
        //     .type_names
        //     .get(&target.low32())
        //     .map(|n| n.as_str())
        //     .unwrap_or("Target");
        // format!("({relation_name}, {target_name})").into()
    }

    pub fn debug_print_archetypes(&self) {
        println!("Amount: {}", self.archetypes.len());
        for archetype in self.archetypes.iter() {
            archetype.borrow().debug_print(self);
        }
    }
    pub fn is_entity_alive(&self, entity: Identifier) -> bool {
        let id_unpacked = entity.unpack();
        if id_unpacked.high32.is_relationship {
            return true;
            // let (Some(relation), Some(target)) =
            //     (self.relation_entity(entity), self.target_entity(entity))
            // else {
            //     return false;
            // };
            // return self.is_entity_alive(relation) && self.is_entity_alive(target);
        }
        let Some(record) = self.record(entity) else {
            return false;
        };
        id_unpacked.high32.second == record.entity.unpack().high32.second
    }

    pub fn archetype_by_id(&self, id: ArchetypeId) -> &ArchetypeCell {
        &self.archetypes[id.0]
    }

    pub fn add_data_relationship<T: AbstractComponent>(
        &mut self,
        entity: Identifier,
        relation: Identifier,
        target: Identifier,
        value: T,
    ) -> Result<()> {
        assert!(std::mem::size_of::<T>() > 0);
        let mut relation_record = match self.record(relation) {
            Some(r) => r,
            None => bail!("expected valid relation record"),
        };
        let mut target_record = match self.record(target) {
            Some(r) => r,
            None => bail!("expected valid target record"),
        };
        let mut entity_record = match self.record(entity) {
            Some(r) => r,
            None => bail!("expected valid entity record"),
        };

        relation_record.entity.set_is_relation(true);
        target_record.entity.set_is_target(true);
        //TODO: consider removing this flag altogether
        entity_record.entity.set_has_relationships(true);

        *self.record_mut(entity) = Some(entity_record);
        *self.record_mut(relation) = Some(relation_record);
        *self.record_mut(target) = Some(target_record);

        let relationship = Archetypes::relationship_id(relation, target);
        {
            let mut type_registry = self.type_registry.borrow_mut();
            type_registry.functions.insert(
                relationship,
                Functions {
                    clone: T::clone_into,
                    serialize: T::serialize,
                    deserialize: T::deserialize,
                    as_reflect_ref: T::as_reflect_ref,
                    as_reflect_mut: T::as_reflect_mut,
                },
            );
            type_registry
                .layouts
                .insert(relationship.stripped(), Layout::new::<T>());
        }

        if self.locked {
            self.temp_components.add_comp(relationship, value);
            self.add_operation(
                entity,
                OperationType::AddComponent {
                    component_id: relationship,
                    table_reusage: TableReusage::New,
                },
            );
            return Ok(());
        }

        let (archetype, add_state) = self.add_component(relationship, entity, TableReusage::New)?;
        let mut archetype = archetype.borrow_mut();
        match add_state {
            ComponentAddState::New => {
                archetype.push_component::<T>(relationship, value);
            }
            ComponentAddState::AlreadyExisted => {
                let table_mut = archetype.table().borrow_mut();
                let mut storage = table_mut.storage(relationship).unwrap().borrow_mut();
                storage.replace_unchecked(entity_record.table_row.0, value);
            }
        }
        Ok(())
    }

    pub fn query_storage(
        &mut self,
        ids: &RequiredIds,
        mask: &FilterMask,
        hash: u64,
    ) -> Rc<RefCell<QueryStorage>> {
        if let Some(v) = self.query_storages.get(&hash) {
            return v.clone();
        }
        let new_storage = self.new_query_storage(ids, mask);
        self.query_storages.insert(hash, new_storage);
        self.query_storages.get(&hash).unwrap().clone()
    }

    fn new_query_storage(
        &mut self,
        ids: &RequiredIds,
        mask: &FilterMask,
    ) -> Rc<RefCell<QueryStorage>> {
        let archetypes = match ids.values.first().copied() {
            Some(f) => {
                let required_components: BTreeSet<_> = ids
                    .values
                    .iter()
                    .filter(|n| !n.is_optional())
                    .map(|n| n.value)
                    .collect();
                let mut archetypes: Vec<_> = self
                    .archetypes_with_id(f.value)
                    .iter()
                    .filter(|a| {
                        let binding = a.borrow();
                        let ids = binding.components_ids();
                        required_components
                            .iter()
                            .all(|req_id| match req_id.wildcard_kind() {
                                WildcardKind::Both => {
                                    panic!("expected valid query term, got wildcard instead")
                                }
                                WildcardKind::Relation => {
                                    ids.iter().any(|id| id.second() == req_id.second())
                                }
                                WildcardKind::Target => {
                                    ids.iter().any(|id| id.low32() == req_id.second())
                                }
                                WildcardKind::None => ids.iter().any(|id| *id == *req_id),
                            })
                    })
                    .cloned()
                    .collect();
                archetypes.retain(|a| mask.matches_archetype(self, a));
                archetypes
            }
            None => {
                //that's quite expensive, but should not happen that often
                let mut archetypes: Vec<_> = self.archetypes.to_vec();
                archetypes.retain(|a| mask.matches_archetype(self, a));
                archetypes
            }
        };
        let mut storage_mask = mask.clone();
        for id in ids.values.iter() {
            storage_mask.has.push(id.value);
        }
        Rc::new(
            QueryStorage {
                archetypes,
                mask: storage_mask,
            }
            .into(),
        )
    }

    pub fn has_component(&self, component: Identifier, entity: Identifier) -> bool {
        let Some(record) = self.record(entity) else {
            return false;
        };

        let Some(archetypes) = self.archetypes_by_ids.get(&component) else {
            return false;
        };
        //TODO: only works if archetypes don't ever get replaced/removed
        //solution: add archetypes_by_ids
        let archetype = self.archetype_by_id(record.arhetype_id);
        archetypes.contains::<ArchetypeCell>(archetype)
    }

    pub fn remove_component(
        &mut self,
        component: Identifier,
        entity: Identifier,
        table_reusage: TableReusage,
    ) -> Result<()> {
        if !self.is_entity_alive(entity) {
            bail!("expected entity to be alive")
        }
        let record = match self.record(entity) {
            Some(r) => r,
            None => bail!("expected initialized record"),
        };
        if !self.has_component(component, entity) {
            return Ok(());
        }

        if self.locked {
            self.add_operation(entity, OperationType::RemoveComponent(component));
            return Ok(());
        }

        let old_archetype = self.archetype_by_id(record.arhetype_id).clone();
        if old_archetype.borrow().components_ids().len() == 1 {
            let old = old_archetype.borrow_mut();
            let entity_archetype = self.entity_archetype().clone();
            let new = entity_archetype.borrow_mut();
            let new_id = new.id();
            let (archetype_row, table_row) = Table::move_entity(
                self,
                entity,
                record.archetype_row,
                record.table_row,
                new,
                old,
            );

            let entity_archetype = self.entity_archetype().clone();
            let mut new = entity_archetype.borrow_mut();
            new.table().borrow_mut().remove_drop(self, table_row);
            new.edge_mut(component).add = Some(old_archetype.borrow().id());
            *self.record_mut(entity) = Some(EntityRecord {
                archetype_row,
                table_row,
                arhetype_id: new_id,
                entity,
            });
            return Ok(());
        }
        let (old_id, old_table, mut old_edge_cloned) = {
            let mut old = old_archetype.borrow_mut();
            (old.id(), old.table().clone(), old.edge_cloned(component))
        };
        let reuse_table = matches!(table_reusage, TableReusage::Reuse);
        let new_archetype = match old_edge_cloned.add {
            Some(id) => self.archetype_by_id(id).clone(),
            None => {
                let mut new_components = old_archetype.borrow().components_ids_set().clone();
                new_components.remove(&component);
                let new_table = match reuse_table {
                    true => self
                        .table_by_components(&new_components)
                        .cloned()
                        .unwrap_or(old_table.into()),
                    false => Table::new(&new_components, self.type_registry.clone()).into(),
                };
                let new_archetype = self
                    .archetype_by_components(&new_components)
                    .cloned()
                    .unwrap_or_else(|| self.add_archetype(&new_table, &new_components).clone());
                old_edge_cloned.remove = Some(new_archetype.borrow().id());
                new_archetype.borrow_mut().edge_mut(component).add = Some(old_id);
                new_archetype
            }
        };
        old_archetype.borrow_mut().edge_mut(component).remove = old_edge_cloned.remove;
        let (new_achetype_row, new_table_row) = if !reuse_table {
            let old = old_archetype.borrow_mut();
            let new = new_archetype.borrow_mut();
            Table::move_entity(
                self,
                entity,
                record.archetype_row,
                record.table_row,
                new,
                old,
            )
        } else {
            old_archetype
                .borrow_mut()
                .remove_drop(self, record.archetype_row, None);
            let (archetype_row, _) = new_archetype
                .borrow_mut()
                .push_entity(entity.low32() as usize, ArchetypeAdd::ArchetypeOnly);
            (archetype_row, record.table_row)
        };
        *self.record_mut(entity) = Some(EntityRecord {
            archetype_row: new_achetype_row,
            table_row: new_table_row,
            arhetype_id: new_archetype.borrow().id(),
            entity,
        });
        Ok(())
    }

    pub fn add_entity(&mut self, kind: EntityKind) -> Identifier {
        let mut id = self.entity_id();
        let is_component = matches!(kind, EntityKind::Component(..));
        if is_component {
            id.set_second(u32::MAX - 1);
        }
        let entity_archetype = self.entity_archetype();
        let entity_archetype_id = entity_archetype.borrow().id();
        let (archetype_row, _) = entity_archetype
            .borrow_mut()
            .push_entity(id.low32() as usize, ArchetypeAdd::ArchetypeOnly);
        let record = EntityRecord {
            archetype_row,
            //TODO: investigate that
            table_row: 0.into(),
            arhetype_id: entity_archetype_id,
            entity: id,
        };
        self.records
            .borrow_mut()
            .insert(id.low32() as usize, Some(record));

        if let EntityKind::Component(component) = kind {
            self.add_component_typed(COMPONENT_ID, id, component)
                .unwrap();
        };
        id
    }

    pub fn children_pool(&self) -> &Rc<RefCell<Vec<(Entity, Depth)>>> {
        &self.children_pool
    }

    pub fn component_id<T: AbstractComponent>(&self) -> Identifier {
        let type_registry = self.type_registry.borrow();
        let type_id = TypeId::of::<T>();
        type_registry
            .identifiers
            .get(&type_id)
            .copied()
            .unwrap_or_else(|| {
                panic!("expected component {0} to be initialized", {
                    tynm::type_name::<T>()
                })
            })
    }

    pub fn register_component<T: AbstractComponent>(&mut self) {
        let type_id = TypeId::of::<T>();
        let type_id_ref = TypeId::of::<&T>();
        let type_id_mut = TypeId::of::<&mut T>();
        let component_name = tynm::type_name::<T>();
        let id = self.add_entity(EntityKind::Component(Component {
            size: Some(std::mem::size_of::<T>()),
            is_type: true,
        }));
        let mut type_registry = self.type_registry.borrow_mut();
        type_registry.add_type_id(type_id, id, &component_name);
        type_registry.add_type_id(type_id_ref, id, &component_name);
        type_registry.add_type_id(type_id_mut, id, &component_name);
        if std::mem::size_of::<T>() > 0 {
            type_registry
                .layouts
                .insert(id.stripped(), Layout::new::<T>());
            type_registry.functions.insert(
                id,
                Functions {
                    clone: T::clone_into,
                    serialize: T::serialize,
                    deserialize: T::deserialize,
                    as_reflect_ref: T::as_reflect_ref,
                    as_reflect_mut: T::as_reflect_mut,
                },
            );
        }
        if std::mem::size_of::<T>() == 0 {
            type_registry.tags.insert(id.into());
        }
    }

    pub fn add_relationship(
        &mut self,
        entity: Identifier,
        relation: Identifier,
        target: Identifier,
    ) -> Result<()> {
        let mut relation_record = match self.record(relation) {
            Some(r) => r,
            None => bail!("expected valid relation record"),
        };
        let mut target_record = match self.record(target) {
            Some(r) => r,
            None => bail!("expected valid target record"),
        };
        let mut entity_record = match self.record(entity) {
            Some(r) => r,
            None => bail!("expected valid entity record"),
        };

        relation_record.entity.set_is_relation(true);
        target_record.entity.set_is_target(true);
        //TODO: consider removing this flag altogether
        entity_record.entity.set_has_relationships(true);

        *self.record_mut(entity) = Some(entity_record);
        *self.record_mut(relation) = Some(relation_record);
        *self.record_mut(target) = Some(target_record);

        let relationship = Archetypes::relationship_id(relation, target);
        self.type_registry
            .borrow_mut()
            .tags
            .insert(relationship.into());

        self.add_component(relationship, entity, TableReusage::Reuse)?;
        Ok(())
    }

    pub fn add_component_tag(&mut self, entity: Identifier, tag: Identifier) -> Result<()> {
        self.add_entity_tag_inner(entity, tag, true)
    }

    pub fn add_entity_tag(&mut self, entity: Identifier, tag: Identifier) -> Result<()> {
        self.add_entity_tag_inner(entity, tag, false)
    }

    fn add_entity_tag_inner(
        &mut self,
        entity: Identifier,
        tag: Identifier,
        is_type: bool,
    ) -> Result<()> {
        if !self.is_entity_alive(entity) {
            bail!("expected alive entity");
        }
        if !self.is_entity_alive(tag) {
            bail!("expected alive tag");
        }
        let has_component_component = {
            let mut type_registry = self.type_registry.borrow_mut();
            let contains_tag = type_registry.components.contains(&tag);
            if !contains_tag {
                type_registry.components.insert(tag);
                type_registry.tags.insert(tag.into());
            }
            contains_tag
        };
        if !has_component_component {
            self.add_component_typed::<Component>(
                COMPONENT_ID,
                tag,
                Component {
                    size: None,
                    is_type,
                },
            )?;
        }
        self.modify_record(tag, |r| {
            r.unwrap().entity.set_is_tag(true);
        });
        self.add_component(tag, entity, TableReusage::Reuse)?;
        Ok(())
    }

    pub fn relationship_id_typed<R: AbstractComponent, T: AbstractComponent>(
        &mut self,
    ) -> Identifier {
        let relation_id = self.component_id::<R>();

        let target_id = self.component_id::<T>();
        IdentifierUnpacked {
            low32: relation_id.low32(),
            high32: IdentifierHigh32 {
                second: target_id.low32().into(),
                is_relationship: true,
                ..Default::default()
            },
        }
        .into()
    }

    pub fn relationship_id(relation: Identifier, target: Identifier) -> Identifier {
        IdentifierUnpacked {
            low32: relation.low32(),
            high32: IdentifierHigh32 {
                second: target.low32().into(),
                is_relationship: true,
                ..Default::default()
            },
        }
        .into()
    }

    pub fn get_component<T: AbstractComponent>(
        &self,
        component: Identifier,
        entity: Identifier,
    ) -> Option<ComponentGetter<T>> {
        if !self.has_component(component, entity) {
            return None;
        }
        ComponentGetter::new(entity, component, self)
    }

    pub fn add_enum_tag<T: EnumTag>(&mut self, entity: Identifier, value: T) -> Result<()> {
        let enum_type_id = self.component_id::<T>();
        let enum_tag_id = self.component_id::<EnumTagId>();
        self.add_data_relationship::<EnumTagId>(
            entity,
            enum_type_id,
            enum_tag_id,
            EnumTagId(value.id()),
        )?;
        Ok(())
    }

    pub fn get_enum_tag<T: EnumTag>(&self, entity: Identifier) -> Option<T> {
        let enum_tag_id = self.component_id::<EnumTagId>();
        let enum_type_id = self.component_id::<T>();
        let relationship = Archetypes::relationship_id(enum_type_id, enum_tag_id);
        let enum_id = self
            .get_component::<EnumTagId>(relationship, entity)?
            .get(|c| *c);
        T::from_id(enum_id.0)
    }

    pub fn remove_enum_tag<T: EnumTag>(&mut self, entity: Identifier) -> Result<()> {
        let enum_tag_id = self.component_id::<EnumTagId>();
        let enum_type_id = self.component_id::<T>();
        let relationship = Archetypes::relationship_id(enum_type_id, enum_tag_id);
        self.remove_component(relationship, entity, TableReusage::New)?;
        Ok(())
    }

    pub fn has_enum_tag<T: EnumTag>(&self, variant: T, entity: Identifier) -> bool {
        let enum_tag_id = self.component_id::<EnumTagId>();
        let enum_type_id = self.component_id::<T>();
        let relationship = Archetypes::relationship_id(enum_type_id, enum_tag_id);
        let Some(enum_id) = self
            .get_component::<EnumTagId>(relationship, entity)
            .map(|c| c.get(|c| *c))
        else {
            return false;
        };

        enum_id.0 == variant.id()
    }

    pub fn add_component_typed<T: AbstractComponent>(
        &mut self,
        component: Identifier,
        entity: Identifier,
        value: T,
    ) -> Result<()> {
        assert!(std::mem::size_of::<T>() > 0);
        if self.locked {
            self.temp_components.add_comp(component, value);
            self.add_operation(
                entity,
                OperationType::AddComponent {
                    component_id: component,
                    table_reusage: TableReusage::New,
                },
            );
            return Ok(());
        }
        let (archetype, add_state) = self.add_component(component, entity, TableReusage::New)?;
        let mut archetype = archetype.borrow_mut();
        match add_state {
            ComponentAddState::New => {
                // dbg!("new");
                archetype.push_component::<T>(component, value);
            }
            ComponentAddState::AlreadyExisted => {
                // dbg!("existed");
                let table_mut = archetype.table().borrow_mut();
                let mut storage = table_mut.storage(component).unwrap().borrow_mut();
                storage.replace_unchecked(self.record(entity).unwrap().table_row.0, value);
            }
        }
        Ok(())
    }

    pub fn entity_archetype(&self) -> &ArchetypeCell {
        &self.archetypes[0]
    }

    pub fn remove_entity(
        &mut self,
        entity: Identifier,
        depth: Depth,
        entities_pool: &mut Vec<Identifier>,
    ) -> Result<()> {
        if !self.is_entity_alive(entity) {
            bail!("expected alive entity");
        }

        let record = match self.record(entity) {
            Some(r) => r,
            None => bail!("expected initialized record"),
        };
        let archetype = match self.archetype_from_record(&record) {
            Some(a) => a.clone(),
            None => bail!("expected valid archetype"),
        };

        if self.locked {
            self.add_operation(entity, OperationType::RemoveEntity);
            return Ok(());
        }

        self.process_entity_deletion(&record, depth, entities_pool);
        archetype
            .borrow_mut()
            .remove_drop(self, record.archetype_row, Some(record.table_row));
        self.records.borrow_mut()[entity.low32() as usize] = None;
        self.unused_ids.push_back(entity);
        Ok(())
    }

    pub fn add_operation(&mut self, entity: Identifier, op_type: OperationType) {
        self.operations
            .borrow_mut()
            .push(ArchetypeOperation { entity, op_type });
    }

    pub fn children_recursive(&self, entity: Identifier) -> ChildrenRecursiveIterRef<'_> {
        ChildrenRecursiveIterRef::new(entity, self.children_pool.clone(), self)
    }

    pub fn process_entity_deletion(
        &mut self,
        record: &EntityRecord,
        depth: Depth,
        entities_pool: &mut Vec<Identifier>,
    ) {
        let entity = record.entity;
        if let Some(parent) = self
            .find_rels::<ChildOf, Wildcard>(record)
            .unwrap()
            .next()
            .and_then(|r| self.target_entity(r.id()))
        {
            self.remove_entity_name((entity, parent).into());
        }

        if depth.0 == 0 {
            let children = self.children_pool.clone();
            let children: &mut _ = &mut children.borrow_mut();
            children_iter::get_children_recursive(entity, self, children, 0.into());
            for (child, _) in children.drain(..) {
                let _ = self.remove_entity(child.into(), (depth.0 + 1).into(), entities_pool);
            }
        }

        self.remove_entity_name((entity, WILDCARD.0).into());
        let is_tag = {
            let registry = self.type_registry();
            registry.tags.contains(&entity.stripped())
        };
        if is_tag {
            self.remove_from_entities(entity);
        }
        if entity.is_relation() {
            let componenet = Archetypes::relationship_id(entity, WILDCARD.0);
            self.remove_from_entities(componenet);
        }
        if entity.is_target() {
            let component = Archetypes::relationship_id(WILDCARD.0, entity);
            self.remove_from_entities(component);
        }
    }

    pub fn remove_from_entities(&mut self, component: Identifier) {
        let Some(archetypes) = self.get_archetypes_with_id(component) else {
            return;
        };
        let operations_pool = self.operatoins_pool.clone();
        let operations_pool: &mut _ = &mut operations_pool.borrow_mut();
        operations_pool.clear();
        for archetype in archetypes {
            for entity in archetype.borrow().entity_indices() {
                let entity = self.record_by_index(*entity).unwrap().entity;
                //TODO: if an entity has only one component, deleting it will put the entity in
                //default archetype, making in inaccessible. Should they be cleared automatically?
                for component in FindRelationshipsIter::from_component(archetype, component) {
                    //we have already deleting all children
                    if component.0.low32() == self.component_id::<ChildOf>().low32() {
                        break;
                    }
                    operations_pool.push(ArchetypeOperation {
                        entity,
                        op_type: OperationType::RemoveComponent(component.0),
                    });
                }
            }
        }
        for op in operations_pool.drain(..) {
            let component = match op.op_type {
                OperationType::RemoveComponent(component) => component,
                _ => unreachable!(),
            };
            let table_reusage = if self.is_component_empty(component) {
                TableReusage::Reuse
            } else {
                TableReusage::New
            };
            self.remove_component(component, op.entity, table_reusage)
                .unwrap();
        }
    }

    pub fn find_rels<R: AbstractComponent, T: AbstractComponent>(
        &self,
        record: &EntityRecord,
    ) -> Option<FindRelationshipsIter> {
        let relation = self.component_id::<R>();
        let target = self.component_id::<T>();
        let archetype = self.archetype_from_record(record).unwrap();
        Some(FindRelationshipsIter::from_archetype(
            archetype, relation, target,
        ))
    }

    pub fn add_component(
        &mut self,
        component: Identifier,
        entity: Identifier,
        table_reusage: TableReusage,
    ) -> Result<(ArchetypeCell, ComponentAddState)> {
        if !self.is_entity_alive(entity) {
            bail!("expected entity to be alive")
        }
        let record = match self.record(entity) {
            Some(r) => r,
            None => bail!("expected initialized record"),
        };
        let old_archetype = self.archetype_by_id(record.arhetype_id).clone();
        let (old_id, old_table, mut old_edge_cloned) = {
            let mut old = old_archetype.borrow_mut();
            (old.id(), old.table().clone(), old.edge_cloned(component))
        };
        if self.has_component(component, entity) {
            return Ok((old_archetype, ComponentAddState::AlreadyExisted));
        }
        let reuse_table = matches!(table_reusage, TableReusage::Reuse);
        let new_archetype = match old_edge_cloned.add {
            Some(id) => self.archetype_by_id(id).clone(),
            None => {
                let mut new_components = old_archetype.borrow().components_ids_set().clone();
                new_components.insert(component);
                new_components.remove(&ENTITY_ID);
                let new_table = match reuse_table {
                    true => self
                        .table_by_components(&new_components)
                        .cloned()
                        .unwrap_or(old_table.into()),
                    false => Table::new(&new_components, self.type_registry.clone()).into(),
                };
                let new_archetype = self
                    .archetype_by_components(&new_components)
                    .cloned()
                    .unwrap_or_else(|| self.add_archetype(&new_table, &new_components).clone());
                old_edge_cloned.add = Some(new_archetype.borrow().id());
                new_archetype.borrow_mut().edge_mut(component).remove = Some(old_id);
                new_archetype
            }
        };
        old_archetype.borrow_mut().edge_mut(component).add = old_edge_cloned.add;
        let (new_achetype_row, new_table_row) = if !reuse_table {
            let old = old_archetype.borrow_mut();
            let new = new_archetype.borrow_mut();
            Table::move_entity(
                self,
                entity,
                record.archetype_row,
                record.table_row,
                new,
                old,
            )
        } else {
            old_archetype
                .borrow_mut()
                .remove_drop(self, record.archetype_row, None);
            let (archetype_row, _) = new_archetype
                .borrow_mut()
                .push_entity(entity.low32() as usize, ArchetypeAdd::ArchetypeOnly);
            (archetype_row, record.table_row)
        };
        *self.record_mut(entity) = Some(EntityRecord {
            archetype_row: new_achetype_row,
            table_row: new_table_row,
            arhetype_id: new_archetype.borrow().id(),
            entity,
        });
        Ok((new_archetype.clone(), ComponentAddState::New))
    }

    pub fn record_mut_by_index(&mut self, index: usize) -> RefMut<Option<EntityRecord>> {
        let records = self.records.borrow_mut();
        RefMut::map(records, |r| &mut r[index])
    }

    pub fn record_mut(&mut self, entity: Identifier) -> RefMut<Option<EntityRecord>> {
        let records = self.records.borrow_mut();
        RefMut::map(records, |r| &mut r[entity.low32() as usize])
    }

    pub fn modify_record<F>(&mut self, entity: Identifier, f: F)
    where
        F: FnOnce(&mut Option<EntityRecord>),
    {
        f(&mut self.record_mut(entity));
    }

    pub fn modify_record_by_index<F>(&mut self, index: usize, f: F)
    where
        F: FnOnce(&mut Option<EntityRecord>),
    {
        f(&mut self.record_mut_by_index(index));
    }

    pub fn archetype_by_components(
        &self,
        components: &BTreeSet<Identifier>,
    ) -> Option<&ArchetypeCell> {
        let archetypes = self.archetypes_by_hashes.get(&components.regular_hash())?;
        archetypes
            .iter()
            .find(|a| a.borrow().components_ids_set() == components)
    }

    pub fn table_by_components(&self, components: &BTreeSet<Identifier>) -> Option<&TableCell> {
        let tables = self.tables_by_hashes.get(&components.regular_hash())?;
        tables
            .iter()
            .find(|a| components == a.borrow().component_ids())
    }

    pub fn add_archetype(
        &mut self,
        table: &TableCell,
        components: &BTreeSet<Identifier>,
    ) -> ArchetypeCell {
        let regular_hash = components.regular_hash();
        let table_hash = components.table_hash(self);
        let archetype: ArchetypeCell = Archetype::new(table.clone().0, components.clone()).into();
        self.archetypes.push(archetype.clone());

        self.add_archetype_by_hash(archetype.clone(), regular_hash);
        self.add_table_by_hash(table.clone(), table_hash);

        for component in components.iter() {
            self.archetypes_with_id(*component)
                .insert(archetype.clone());

            let unpacked_id = component.unpack();
            if !unpacked_id.high32.is_relationship
                || *component == COMPONENT_ID
                || *component == ENTITY_ID
            {
                continue;
            }

            let relation = unpacked_id.low32;
            let target = unpacked_id.high32.second;
            let wildcard_target = IdentifierUnpacked {
                low32: WILDCARD_32,
                high32: IdentifierHigh32 {
                    second: target,
                    is_relationship: true,
                    ..Default::default()
                },
            }
            .pack()
            .unwrap();
            let wildcard_relation = IdentifierUnpacked {
                low32: relation,
                high32: IdentifierHigh32 {
                    second: WILDCARD_25.into(),
                    is_relationship: true,
                    ..Default::default()
                },
            }
            .pack()
            .unwrap();

            self.archetypes_with_id(wildcard_target.into())
                .insert(archetype.clone());
            self.archetypes_with_id(wildcard_relation.into())
                .insert(archetype.clone());
            self.archetypes_with_id(WILDCARD_RELATIONSHIP)
                .insert(archetype.clone());
        }
        for storage in self.query_storages.values() {
            let mut storage = storage.borrow_mut();
            if storage.mask.matches_archetype(self, &archetype) {
                storage.archetypes.push(archetype.clone());
            }
        }

        archetype
    }

    pub fn archetypes_with_id(&mut self, id: Identifier) -> &mut ArchetypeSet {
        self.archetypes_by_ids.entry(id).or_default()
    }

    pub fn get_archetypes_with_id(&self, id: Identifier) -> Option<&ArchetypeSet> {
        self.archetypes_by_ids.get(&id)
    }

    pub fn add_table_by_hash(&mut self, table: TableCell, hash: u64) {
        if let Some(tables) = self.tables_by_hashes.get_mut(&hash) {
            tables.push(Into::into(table.clone()));
            return;
        }

        let archetypes = vec![table.clone()];
        self.tables_by_hashes.insert(hash, archetypes);
    }

    pub fn add_archetype_by_hash(&mut self, archetype: ArchetypeCell, hash: u64) {
        if let Some(archetypes) = self.archetypes_by_hashes.get_mut(&hash) {
            archetypes.push(archetype.clone());
            return;
        }

        let archetypes = vec![archetype.clone()];
        self.archetypes_by_hashes.insert(hash, archetypes);
    }

    pub fn callbacks(&self) -> &Rc<RefCell<OnChangeCallbacks>> {
        &self.callbacks
    }

    pub fn resources(&self) -> &Rc<RefCell<Resources>> {
        &self.resources
    }

    pub fn state_operations(&self) -> &RefCell<Vec<StateOperation>> {
        &self.state_operations
    }
}

impl Default for Archetypes {
    fn default() -> Self {
        Self::new()
    }
}
