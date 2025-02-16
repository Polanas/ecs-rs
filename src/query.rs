use std::{
    cell::RefCell,
    fmt::Debug,
    hash::{DefaultHasher, Hash, Hasher},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use packed_struct::PackedStruct;

use crate::identifier::IdentifierUnpacked;
use crate::world::archetypes;
pub use crate::{
    archetype::ArchetypeRow, components::component::EnumTag, relationship::RelationshipsIter,
};
use crate::{archetypes::ChildOf, entity::Entity, expect_fn::ExpectFnOption};
use crate::{
    archetypes::QueryStorage,
    borrow_traits::BorrowFn,
    components::component::AbstractComponent,
    filter_mask::FilterMask,
    identifier::Identifier,
    table::TableRow,
    world::{self, archetypes_mut},
};
use crate::{
    archetypes::{Archetypes, EnumTagId, Prefab},
    entity::WILDCARD,
};
#[derive(Debug, Clone, Copy, Default)]
pub enum FilterMaskHint {
    #[default]
    Regular,
    Not,
}

pub struct IdsIterator<'ids> {
    ids: &'ids [QueryIdentifier],
    index: usize,
}

impl Iterator for IdsIterator<'_> {
    type Item = Identifier;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.ids.len() {
            return None;
        }
        let item = Some(self.ids[self.index]);
        self.index += 1;
        item.map(|i| i.value)
    }
}

impl<'ids> IdsIterator<'ids> {
    pub fn new(ids: &'ids [QueryIdentifier]) -> Self {
        Self { ids, index: 0 }
    }
}

macro_rules! impl_query_filter {
    ($($params:ident),+) => {
        impl <$($params: QueryFilterData),+> QueryFilterData for ( $($params),+) {
            fn mask(mask: &mut FilterMask, hint: FilterMaskHint) {
                $(
                    $params::mask(mask, hint);
                )+
            }
        }
    };
}

impl_query_filter!(T0, T1);
impl_query_filter!(T0, T1, T2);
impl_query_filter!(T0, T1, T2, T3);
impl_query_filter!(T0, T1, T2, T3, T4);
impl_query_filter!(T0, T1, T2, T3, T4, T5);
impl_query_filter!(T0, T1, T2, T3, T4, T6, T7);
impl_query_filter!(T0, T1, T2, T3, T4, T6, T7, T8);
impl_query_filter!(T0, T1, T2, T3, T4, T6, T7, T8, T9);
impl_query_filter!(T0, T1, T2, T3, T4, T6, T7, T8, T9, T10);
impl_query_filter!(T0, T1, T2, T3, T4, T6, T7, T8, T9, T10, T11);
impl_query_filter!(T0, T1, T2, T3, T4, T6, T7, T8, T9, T10, T11, T12);

impl<T0: QueryFilterData> QueryFilterData for (T0,) {
    fn mask(mask: &mut FilterMask, hint: FilterMaskHint) {
        T0::mask(mask, hint)
    }
}

pub trait QueryFilterData {
    fn mask(mask: &mut FilterMask, hint: FilterMaskHint);
}

impl QueryFilterData for () {
    fn mask(_mask: &mut FilterMask, _hint: FilterMaskHint) {}
}

pub trait WorldQuery {
    type Item<'i>;
    fn fetch<'w>(
        storage: &'w Rc<RefCell<QueryStorage>>,
        archetype_index: usize,
        ids: &mut IdsIterator,
        table_row: TableRow,
        archetype_row: ArchetypeRow,
    ) -> Self::Item<'w>;
}
impl WorldQuery for () {
    type Item<'i> = ();

    fn fetch<'w>(
        _storage: &'w Rc<RefCell<QueryStorage>>,
        _archetype_index: usize,
        _ids: &mut IdsIterator,
        _table_row: TableRow,
        _archetype_row: ArchetypeRow,
    ) -> Self::Item<'w> {
    }
}
pub trait QueryData: WorldQuery {
    fn ids(ids: &mut RequiredIds);
}

impl<T: AbstractComponent> WorldQuery for Option<&T> {
    type Item<'i> = Option<Ref<'i, T>>;

    fn fetch<'w>(
        storage: &'w Rc<RefCell<QueryStorage>>,
        archetype_index: usize,
        ids: &mut IdsIterator,
        row: TableRow,
        _: ArchetypeRow,
    ) -> Self::Item<'w> {
        let storage = storage.borrow();
        let archetype = &storage.archetypes[archetype_index].borrow();
        let table = archetype.table().borrow();
        let id = ids.next().unwrap();
        let storage = table.storage(id)?.borrow();
        let component_ptr = storage.component(row);
        Some(Ref::new(unsafe { &*(component_ptr.as_ptr() as *mut T) }))
    }
}

impl<T: AbstractComponent> WorldQuery for Option<&mut T> {
    type Item<'i> = Option<Mut<'i, T>>;

    fn fetch<'w>(
        storage: &'w Rc<RefCell<QueryStorage>>,
        archetype_index: usize,
        ids: &mut IdsIterator,
        row: TableRow,
        _: ArchetypeRow,
    ) -> Self::Item<'w> {
        let storage = storage.borrow();
        let archetype = &storage.archetypes[archetype_index].borrow();
        let table = archetype.table().borrow();
        let id = ids.next().unwrap();
        let storage = table.storage(id)?.borrow();
        let component_ptr = storage.component(row);
        Some(Mut::new(unsafe {
            &mut *(component_ptr.as_ptr() as *mut T)
        }))
    }
}
impl<T: AbstractComponent> WorldQuery for &T {
    type Item<'i> = Ref<'i, T>;

    fn fetch<'w>(
        storage: &'w Rc<RefCell<QueryStorage>>,
        archetype_index: usize,
        ids: &mut IdsIterator,
        row: TableRow,
        _: ArchetypeRow,
    ) -> Self::Item<'w> {
        let storage = storage.borrow();
        let archetype = &storage.archetypes[archetype_index].borrow();
        let table = archetype.table().borrow();
        let id = ids.next().unwrap();
        //TODO: find a way to replace wildcard data ids to actual ids
        let storage = table.storage(id).unwrap().borrow();
        let component_ptr = storage.component(row);
        Ref::new(unsafe { &*(component_ptr.as_ptr() as *mut T) })
    }
}

impl<T: AbstractComponent> WorldQuery for &mut T {
    type Item<'i> = Mut<'i, T>;

    fn fetch<'w>(
        storage: &'w Rc<RefCell<QueryStorage>>,
        archetype_index: usize,
        ids: &mut IdsIterator,
        row: TableRow,
        _: ArchetypeRow,
    ) -> Self::Item<'w> {
        let storage = storage.borrow();
        let archetype = &storage.archetypes[archetype_index].borrow();
        let table = archetype.table().borrow();
        let id = ids.next().unwrap();
        let storage = table.storage(id).unwrap().borrow();
        let component_ptr = storage.component(row);
        Mut::new(unsafe { &mut *(component_ptr.as_ptr() as *mut T) })
    }
}

impl<T: AbstractComponent> QueryData for Option<&T> {
    fn ids(ids: &mut RequiredIds) {
        archetypes_mut(|archetypes| {
            let component = archetypes.component_id::<T>();
            ids.push(QueryIdentifier::new(
                component,
                IdOptionalType::Optional,
                IdAccessType::Ref,
            ));
        })
    }
}
impl<T: AbstractComponent> QueryData for Option<&mut T> {
    fn ids(ids: &mut RequiredIds) {
        archetypes_mut(|archetypes| {
            let component = archetypes.component_id::<T>();
            ids.push(QueryIdentifier::new(
                component,
                IdOptionalType::Optional,
                IdAccessType::Mut,
            ));
        })
    }
}
impl<T: AbstractComponent> QueryData for &T {
    fn ids(ids: &mut RequiredIds) {
        archetypes_mut(|archetypes| {
            let component = archetypes.component_id::<T>();
            ids.push(QueryIdentifier::new(
                component,
                IdOptionalType::Required,
                IdAccessType::Ref,
            ));
        })
    }
}
impl<T: AbstractComponent> QueryData for &mut T {
    fn ids(ids: &mut RequiredIds) {
        archetypes_mut(|archetypes| {
            let component = archetypes.component_id::<T>();
            ids.push(QueryIdentifier::new(
                component,
                IdOptionalType::Required,
                IdAccessType::Mut,
            ));
        })
    }
}

impl QueryData for &mut Entity {
    fn ids(_: &mut RequiredIds) {}
}

impl WorldQuery for &mut Entity {
    type Item<'i> = Entity;

    fn fetch<'w>(
        storage: &'w Rc<RefCell<QueryStorage>>,
        archetype_index: usize,
        _: &mut IdsIterator,
        _: TableRow,
        archetype_row: ArchetypeRow,
    ) -> Self::Item<'w> {
        let storage = storage.borrow();
        let archetype = &storage.archetypes[archetype_index].borrow();
        archetypes(|archetypes| {
            Entity::new(
                archetypes
                    .record_by_index(archetype.entity_indices()[archetype_row.0])
                    .unwrap()
                    .entity,
            )
        })
    }
}
impl QueryData for &Entity {
    fn ids(_: &mut RequiredIds) {}
}

impl WorldQuery for &Entity {
    type Item<'i> = Entity;

    fn fetch<'w>(
        storage: &'w Rc<RefCell<QueryStorage>>,
        archetype_index: usize,
        _: &mut IdsIterator,
        _: TableRow,
        archetype_row: ArchetypeRow,
    ) -> Self::Item<'w> {
        let storage = storage.borrow();
        let archetype = &storage.archetypes[archetype_index].borrow();
        archetypes(|archetypes| {
            Entity::new(
                archetypes
                    .record_by_index(archetype.entity_indices()[archetype_row.0])
                    .unwrap()
                    .entity,
            )
        })
    }
}

macro_rules! impl_query_data {
    (
        $($params:ident),+
    ) => {
        impl <$($params: QueryData),+> QueryData for ($($params),+,) {
            fn ids(ids: &mut RequiredIds) {
                $(
                    $params::ids(ids);
                )+
            }
        }
        impl <$($params: QueryData),+> WorldQuery for ($($params),+,) {
            #[allow(unused_parens)]
            type Item<'i> = ($(
                    $params::Item<'i>
            ),+);
            fn fetch<'w>(
                storage: &'w Rc<RefCell<QueryStorage>>,
                archetype_index: usize,
                ids: &mut IdsIterator,
                table_row: TableRow,
                archetype_row: ArchetypeRow,
            ) -> Self::Item<'w> {
                ($(
                    $params::fetch(storage, archetype_index, ids, table_row, archetype_row)
                ),+)
            }
        }
    };
}
impl_query_data!(T0);
impl_query_data!(T0, T1);
impl_query_data!(T0, T1, T2);
impl_query_data!(T0, T1, T2, T3);
impl_query_data!(T0, T1, T2, T3, T4);
impl_query_data!(T0, T1, T2, T3, T4, T5);
impl_query_data!(T0, T1, T2, T3, T4, T5, T6);
impl_query_data!(T0, T1, T2, T3, T4, T5, T6, T7);
impl_query_data!(T0, T1, T2, T3, T4, T5, T6, T7, T8);
impl_query_data!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_query_data!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_query_data!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_query_data!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);

#[derive(Debug, Clone, Copy, Hash)]
pub enum IdOptionalType {
    Optional,
    Required,
}

#[derive(Debug, Clone, Copy, Hash)]
pub enum IdAccessType {
    Ref,
    Mut,
}
#[derive(Debug, Clone, Copy)]
pub struct QueryIdentifier {
    pub value: Identifier,
    pub optional_type: IdOptionalType,
    pub access_type: IdAccessType,
}

impl std::hash::Hash for QueryIdentifier {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
        self.optional_type.hash(state);
        self.access_type.hash(state);
    }
}

impl PartialEq for QueryIdentifier {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for QueryIdentifier {}

impl Ord for QueryIdentifier {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

impl PartialOrd for QueryIdentifier {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.value.cmp(&other.value))
    }
}

impl QueryIdentifier {
    pub fn new(
        value: Identifier,
        optional_type: IdOptionalType,
        access_type: IdAccessType,
    ) -> Self {
        Self {
            value,
            optional_type,
            access_type,
        }
    }

    pub fn with_new_id(&self, id: Identifier) -> Self {
        Self {
            value: id,
            optional_type: self.optional_type,
            access_type: self.access_type,
        }
    }

    pub fn is_optional(&self) -> bool {
        matches!(self.optional_type, IdOptionalType::Optional)
    }
}

#[derive(Clone, Hash)]
pub struct RequiredIds {
    pub values: Vec<QueryIdentifier>,
}

impl RequiredIds {
    pub fn new() -> Self {
        Self { values: vec![] }
    }

    pub fn join(&mut self, other: &RequiredIds) {
        for id in &other.values {
            self.values.push(*id)
        }
    }

    pub fn sort(&mut self) {
        self.values.sort();
    }

    pub fn push(&mut self, id: QueryIdentifier) {
        self.values.push(id)
    }
}

impl Default for RequiredIds {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Ref<'a, T> {
    value: &'a T,
}

impl<T> Debug for Ref<'_, T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

trait NewTrait<'a, T> where
    T: Clone, {
    fn clone(&self) -> T;
}

impl<'a, T> NewTrait<'a, T> for Ref<'a, T>
where
    T: Clone,
{
    fn clone(&self) -> T {
        self.value.clone()
    }
}
impl<'a, T> Ref<'a, T> {
    pub fn new(value: &'a T) -> Self {
        Self { value }
    }
}

impl<T> Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

pub struct Mut<'a, T> {
    value: &'a mut T,
}

impl<T> Mut<'_, T>
where
    T: Clone,
{
    fn clone(&self) -> T {
        self.value.clone()
    }
}

impl<T> Debug for Mut<'_, T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

// impl<'a,T> Drop for Mut<'a,T> {
//     fn drop(&mut self) {
//         archetypes_mut(|a| {
//
//         });
//     }
// }

impl<'a, T> Mut<'a, T> {
    pub fn new(value: &'a mut T) -> Self {
        Self { value }
    }
}

impl<T> DerefMut for Mut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}
impl<T> Deref for Mut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

pub struct QueryIterator<'w, D: QueryData, F: QueryFilterData> {
    state: &'w QueryState<D, F>,
    storage: &'w Rc<RefCell<QueryStorage>>,
    archetype_index: usize,
    entity_index: usize,
}

impl<D: QueryData, F: QueryFilterData> Drop for QueryIterator<'_, D, F> {
    fn drop(&mut self) {
        archetypes_mut(|a| a.unlock());
    }
}

pub struct Query<D: QueryData, F: QueryFilterData = ()> {
    pub state: QueryState<D, F>,
    pub storage: Rc<RefCell<QueryStorage>>,
}

impl<D: QueryData, F: QueryFilterData> Query<D, F> {
    pub fn get_first(&mut self) -> Option<D::Item<'_>> {
        self.iter().next()
    }
    pub fn first(&mut self) -> D::Item<'_> {
        self.iter().next().unwrap_or_else(|| {
            panic!(
                "query {0} with filter {1} expected to contain at least one entity",
                tynm::type_name::<D>(),
                tynm::type_name::<F>()
            )
        })
    }
    pub fn matches_entity(&self, entity: Entity) -> bool {
        archetypes_mut(|a| {
            let record = a.record(entity.0).unwrap();
            let archetype = a.archetype_from_record(&record).unwrap().clone();
            self.state.mask.matches_archetype(a, &archetype)
        })
    }
    pub fn is_empty(&self) -> bool {
        self.storage.borrow().archetypes.is_empty()
    }
}

impl<D: QueryData, F: QueryFilterData> Query<D, F> {
    pub fn iter(&mut self) -> QueryIterator<D, F> {
        archetypes_mut(|a| a.lock());
        QueryIterator {
            state: &self.state,
            storage: &self.storage,
            archetype_index: 0,
            entity_index: 0,
        }
    }
}
pub struct QueryState<D: QueryData, F: QueryFilterData = ()> {
    pub mask: FilterMask,
    pub data: PhantomData<(D, F)>,
    pub ids: RequiredIds,
}
#[derive(Clone, Copy, Debug)]
pub struct QueryComoponentId(pub u32);

fn id_or_relation(archetypes: &mut Archetypes, id: Identifier) -> Identifier {
    if id.is_relationship() {
        archetypes.relation_entity(id).unwrap()
    } else {
        id
    }
}

fn id_or_target(archetypes: &mut Archetypes, id: Identifier) -> Identifier {
    if id.is_relationship() {
        archetypes.target_entity(id).unwrap()
    } else {
        id
    }
}

//TODO: add support of mutiple archetypes per entity
impl<D: QueryData, F: QueryFilterData> QueryState<D, F> {
    pub fn new() -> Self {
        let mut ids = RequiredIds::new();
        D::ids(&mut ids);
        let mut mask = FilterMask::new();
        F::mask(&mut mask, Default::default());
        Self {
            data: PhantomData,
            ids,
            mask,
        }
    }

    pub fn build(mut self) -> Query<D, F> {
        let mut hasher = DefaultHasher::new();
        self.mask
            .push_not(archetypes_mut(|a| a.component_id::<Prefab>()));

        let mut sorted_ids = self.ids.values.clone();
        sorted_ids.sort_by_key(|id| id.value);
        let sorted_ids = RequiredIds { values: sorted_ids };

        self.ids.hash(&mut hasher);
        self.mask.hash(&mut hasher);
        let hash = hasher.finish();
        let storage = archetypes_mut(|archetypes| {
            archetypes
                .query_storage(&sorted_ids, &self.mask, hash)
                .clone()
        });
        Query::new(self, storage)
    }

    pub fn term_relation<T: AbstractComponent>(mut self, term_index: usize) -> Self {
        let term = self.ids.values[term_index];
        archetypes_mut(|archetypes| {
            let relation = archetypes.component_id::<T>();
            let target = id_or_target(archetypes, term.value);
            let relationship = Archetypes::relationship_id(relation, target);
            self.ids.values[term_index] = term.with_new_id(relationship);
        });
        self
    }

    pub fn term_target<T: AbstractComponent>(mut self, term_index: usize) -> Self {
        let term = self.ids.values[term_index];
        archetypes_mut(|archetypes| {
            let relation = id_or_relation(archetypes, term.value);
            let target = archetypes.component_id::<T>();
            let relationship = Archetypes::relationship_id(relation, target);
            self.ids.values[term_index] = term.with_new_id(relationship);
        });
        self
    }

    pub fn set_relation<R: AbstractComponent>(mut self, id: QueryComoponentId) -> Self {
        let relation = archetypes_mut(|archetypes| archetypes.component_id::<R>());
        if id.0 as usize >= self.ids.values.len() {
            panic!(
                "expected component id between 0 and {}, got {}",
                self.ids.values.len(),
                id.0
            );
        }
        let component_id = &mut self.ids.values[id.0 as usize];
        if component_id.value.is_relationship() {
            panic!("expected component not to be a relationship");
        }
        let target = component_id.value.low32();
        let relationship = Archetypes::relationship_id(
            relation,
            IdentifierUnpacked {
                low32: target,
                ..Default::default()
            }
            .pack()
            .unwrap()
            .into(),
        );
        component_id.value = relationship;
        self
    }

    pub fn set_target<T: AbstractComponent>(mut self, id: QueryComoponentId) -> Self {
        let target = archetypes_mut(|archetypes| archetypes.component_id::<T>());
        if id.0 as usize >= self.ids.values.len() {
            panic!(
                "expected component id between 0 and {}, got {}",
                self.ids.values.len(),
                id.0
            );
        }
        let component_id = &mut self.ids.values[id.0 as usize];
        if component_id.value.is_relationship() {
            panic!("expected component not to be a relationship");
        }
        let relation = component_id.value.low32();
        let relationship = Archetypes::relationship_id(
            IdentifierUnpacked {
                low32: relation,
                ..Default::default()
            }
            .pack()
            .unwrap()
            .into(),
            target,
        );
        component_id.value = relationship;
        self
    }

    pub fn without_any_children_of(mut self, parent: Entity) -> Self {
        archetypes_mut(|archetypes| {
            let childof_id = archetypes.component_id::<ChildOf>();
            let relationship = Archetypes::relationship_id(childof_id, parent.0);
            self.mask.push_any_not(relationship);
        });
        self
    }

    pub fn with_any_children_of(mut self, parent: Entity) -> Self {
        archetypes_mut(|archetypes| {
            let childof_id = archetypes.component_id::<ChildOf>();
            let relationship = Archetypes::relationship_id(childof_id, parent.0);
            self.mask.push_any_has(relationship);
        });
        self
    }

    pub fn without_children_of(mut self, parent: Entity) -> Self {
        archetypes_mut(|archetypes| {
            let childof_id = archetypes.component_id::<ChildOf>();
            let relationship = Archetypes::relationship_id(childof_id, parent.0);
            self.mask.push_not(relationship);
        });
        self
    }

    pub fn with_children_of(mut self, parent: Entity) -> Self {
        archetypes_mut(|archetypes| {
            let childof_id = archetypes.component_id::<ChildOf>();
            let relationship = Archetypes::relationship_id(childof_id, parent.0);
            self.mask.push_has(relationship);
        });
        self
    }

    pub fn with_comp<T: AbstractComponent>(mut self) -> Self {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            self.mask.push_has(archetypes.component_id::<T>());
        });
        self
    }

    pub fn without_comp<T: AbstractComponent>(mut self) -> Self {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            self.mask.push_not(archetypes.component_id::<T>());
        });
        self
    }

    pub fn with_any_comp<T: AbstractComponent>(mut self) -> Self {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            self.mask.push_any_has(archetypes.component_id::<T>());
        });
        self
    }

    pub fn without_any_comp<T: AbstractComponent>(mut self) -> Self {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|archetypes| {
            self.mask.push_any_has(archetypes.component_id::<T>());
        });
        self
    }
    pub fn with_tag<T: AbstractComponent>(mut self) -> Self {
        assert!(std::mem::size_of::<T>() == 0);
        archetypes_mut(|archetypes| {
            self.mask.push_has(archetypes.component_id::<T>());
        });
        self
    }

    pub fn without_tag<T: AbstractComponent>(mut self) -> Self {
        assert!(std::mem::size_of::<T>() == 0);
        archetypes_mut(|archetypes| {
            self.mask.push_not(archetypes.component_id::<T>());
        });
        self
    }

    pub fn with_any_tag<T: AbstractComponent>(mut self) -> Self {
        assert!(std::mem::size_of::<T>() == 0);
        archetypes_mut(|archetypes| {
            self.mask.push_any_has(archetypes.component_id::<T>());
        });
        self
    }

    pub fn without_any_tag<T: AbstractComponent>(mut self) -> Self {
        assert!(std::mem::size_of::<T>() == 0);
        archetypes_mut(|archetypes| {
            self.mask.push_any_has(archetypes.component_id::<T>());
        });
        self
    }

    pub fn with_rel<R: AbstractComponent, T: AbstractComponent>(mut self) -> Self {
        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            self.mask.push_has(relationship);
        });
        self
    }

    pub fn with_enum_tag<T: EnumTag>(mut self, tag: T) -> Self {
        archetypes_mut(|archetypes| {
            let enum_tag_id = archetypes.component_id::<EnumTagId>();
            let enum_type_id = archetypes.component_id::<T>();
            let relationship = Archetypes::relationship_id(enum_type_id, enum_tag_id);
            let wildcard_relationship = Archetypes::relationship_id(enum_type_id, WILDCARD.into());
            self.mask.push_has(wildcard_relationship);
            self.mask.push_states((relationship, tag.id()));
        });
        self
    }

    pub fn without_rel<R: AbstractComponent, T: AbstractComponent>(mut self) -> Self {
        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            self.mask.push_not(relationship);
        });
        self
    }

    pub fn with_any_rel<R: AbstractComponent, T: AbstractComponent>(mut self) -> Self {
        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            self.mask.push_any_has(relationship);
        });
        self
    }

    pub fn without_any_rel<R: AbstractComponent, T: AbstractComponent>(mut self) -> Self {
        archetypes_mut(|archetypes| {
            let relationship = archetypes.relationship_id_typed::<R, T>();
            self.mask.push_any_not(relationship);
        });
        self
    }

    pub fn without_any_mixed_rel<T: AbstractComponent>(mut self, target: Entity) -> Self {
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<T>();
            let relationship = Archetypes::relationship_id(target.0, relation_id);
            self.mask.push_any_not(relationship);
        });
        self
    }
    pub fn with_any_mixed_rel<T: AbstractComponent>(mut self, target: Entity) -> Self {
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<T>();
            let relationship = Archetypes::relationship_id(target.0, relation_id);
            self.mask.push_any_has(relationship);
        });
        self
    }
    pub fn without_mixed_rel<T: AbstractComponent>(mut self, target: Entity) -> Self {
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<T>();
            let relationship = Archetypes::relationship_id(target.0, relation_id);
            self.mask.push_not(relationship);
        });
        self
    }
    pub fn with_mixed_rel<T: AbstractComponent>(mut self, target: Entity) -> Self {
        archetypes_mut(|archetypes| {
            let relation_id = archetypes.component_id::<T>();
            let relationship = Archetypes::relationship_id(target.0, relation_id);
            self.mask.push_has(relationship);
        });
        self
    }

    pub fn without_any_ent_rel(mut self, relation: Entity, target: Entity) -> Self {
        let relationship = Archetypes::relationship_id(relation.0, target.0);
        self.mask.push_any_not(relationship);
        self
    }

    pub fn with_any_ent_rel(mut self, relation: Entity, target: Entity) -> Self {
        let relationship = Archetypes::relationship_id(relation.0, target.0);
        self.mask.push_any_has(relationship);
        self
    }

    pub fn without_ent_rel(mut self, relation: Entity, target: Entity) -> Self {
        let relationship = Archetypes::relationship_id(relation.0, target.0);
        self.mask.push_not(relationship);
        self
    }

    pub fn with_ent_rel(mut self, relation: Entity, target: Entity) -> Self {
        let relationship = Archetypes::relationship_id(relation.0, target.0);
        self.mask.push_has(relationship);
        self
    }

    pub fn without_any_ent_tag(mut self, tag: Entity) -> Self {
        self.mask.push_any_not(tag.0);
        self
    }

    pub fn with_any_ent_tag(mut self, tag: Entity) -> Self {
        self.mask.push_any_has(tag.0);
        self
    }

    pub fn without_ent_tag(mut self, tag: Entity) -> Self {
        self.mask.push_not(tag.0);
        self
    }

    pub fn with_ent_tag(mut self, tag: Entity) -> Self {
        self.mask.push_has(tag.0);
        self
    }
}

impl<D: QueryData, F: QueryFilterData> Default for QueryState<D, F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'w, D: QueryData, F: QueryFilterData> Iterator for QueryIterator<'w, D, F> {
    type Item = D::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        let storage = self.storage.borrow();
        let archetypes = &storage.archetypes;
        let record = loop {
            let archetype = archetypes.get(self.archetype_index)?;

            if archetype.is_empty() {
                self.archetype_index += 1;
                continue;
            }

            if self.entity_index == archetype.len() {
                self.entity_index = 0;
                self.archetype_index += 1;
                continue;
            }

            let record = world::archetypes(|archetypes| {
                archetypes
                    .record_by_index(archetype.borrow_fn(|a| a.entity_indices()[self.entity_index]))
                    .map(|r| r)
            });
            let Some(record) = record else {
                panic!(
                    "could not find record of entity with id {:?}",
                    archetype.borrow_fn(|a| a.entity_indices()[self.entity_index])
                );
                // self.entity_index += 1;
                // continue;
            };
            // std::cell::Ref::map(record, |r| r.)
            // let core:cell::Ref { value: Some(record) } = record else {
            //     continue;
            // };
            // .expect_fn(|| {
            //     log::error!("couldn't get a record while iterating entities")
            // })
            // });

            if !record.entity.is_active() {
                self.entity_index += 1;
                continue;
            }

            let has_enum_tags = self
                .state
                .mask
                .states
                .iter()
                .all(|(component_id, enum_id)| {
                    archetype.borrow_fn(|archetype| {
                        archetype.table().borrow_fn(|table| {
                            let Some(storage) = table.storage(*component_id) else {
                                return false;
                            };
                            storage.borrow_fn(|storage| {
                                let component = storage.component(record.table_row);
                                let component = unsafe { &*(component.as_ptr() as *mut EnumTagId) };
                                component.0 == *enum_id
                            })
                        })
                    })
                });

            if !has_enum_tags {
                self.entity_index += 1;
                continue;
            }

            self.entity_index += 1;
            break record;
        };
        let mut ids = IdsIterator::new(&self.state.ids.values[..]);
        drop(storage);
        Some(D::fetch(
            self.storage,
            self.archetype_index,
            &mut ids,
            record.table_row,
            record.archetype_row,
        ))
    }
}
impl<D: QueryData, F: QueryFilterData> Query<D, F> {
    pub fn new(state: QueryState<D, F>, storage: Rc<RefCell<QueryStorage>>) -> Self {
        Self { state, storage }
    }
}

//TODO: delelte this
impl QueryData for () {
    fn ids(_: &mut RequiredIds) {}
}
