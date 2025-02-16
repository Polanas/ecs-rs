use std::{
    any::{Any, TypeId},
    cell::RefCell,
    rc::Rc,
};

use bevy_utils::hashbrown::HashMap;
use num::{Integer, PrimInt};

use crate::{
    call_16_times, events::CurrentSystemId, expect_fn::ExpectFnOption, systems::SystemId,
    world::World,
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct LocalId(usize);
impl From<usize> for LocalId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
type LocalsStorage = Vec<Box<dyn Any + 'static>>;

pub struct SystemLocals {
    locals: HashMap<SystemId, (Rc<RefCell<LocalsStorage>>, LocalId)>,
}

impl SystemLocals {
    pub fn new() -> Self {
        Self {
            locals: HashMap::new(),
        }
    }

    pub fn reset_ids(&mut self) {
        self.locals.values_mut().for_each(|v| v.1 = LocalId(0));
    }

    pub fn current_system_id(&self) -> SystemId {
        World::default().resources_ret::<&CurrentSystemId, _>(|id_res| id_res.value)
    }

    pub fn get_mut<L: LocalQueryMut>(&mut self) -> L::Item<'_> {
        let mut ids = vec![];
        let system_id = self.current_system_id();
        L::add_ids(&mut ids);
        let locals = self
            .locals
            .entry(system_id)
            .or_insert_with(|| (RefCell::new(L::default()).into(), LocalId(0)));
        L::fetch(&locals.0, &mut LocalIdsIterator::new(&ids))
    }

    pub fn get_mut_or_default<L: LocalQueryMut>(&mut self) -> L::Item<'_> {
        let mut ids = vec![];
        let system_id = self.current_system_id();
        L::add_ids(&mut ids);
        let locals = self
            .locals
            .entry(system_id)
            .or_insert_with(|| (RefCell::new(L::default()).into(), LocalId(0)));
        L::fetch(&locals.0, &mut LocalIdsIterator::new(&ids))
    }
}

pub trait SystemLocal: 'static + Default {}
impl<T: Sized + 'static + Default> SystemLocal for T {}

pub struct LocalIdsIterator<'a> {
    ids: &'a [LocalId],
    index: usize,
}

impl<'a> LocalIdsIterator<'a> {
    pub fn new(ids: &'a [LocalId]) -> Self {
        Self { index: 0, ids }
    }
}

impl Iterator for LocalIdsIterator<'_> {
    type Item = LocalId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.ids.len() {
            return None;
        }
        let id = self.ids[self.index];
        self.index += 1;
        Some(id)
    }
}

macro_rules! impl_local_query_mut {
    ($($params:ident),+) => {
        impl <$($params: LocalQueryMut),+> LocalQueryMut for ($($params),+,) {
            type Item<'i> = ($($params::Item<'i>),+,);
            type Owned = ($($params::Owned),+,);

            fn fetch<'a>(locals: &'a Rc<RefCell<LocalsStorage>>, ids: &mut LocalIdsIterator) -> Self::Item<'a> {
                ($(
                    $params::fetch(locals, ids)
                ),+,)
            }

            fn add_ids(ids: &mut Vec<LocalId>) {
                $(
                    $params::add_ids(ids);
                )+
            }

            fn default() -> LocalsStorage {
                let mut defaults: LocalsStorage = vec![];
                $(
                    defaults.append(&mut $params::default());
                )+
                defaults
            }
        }
    };
}

call_16_times!(impl_local_query_mut);

pub trait LocalQueryMut {
    type Item<'i>;
    type Owned: 'static;
    fn fetch<'a>(
        locals: &'a Rc<RefCell<LocalsStorage>>,
        ids: &mut LocalIdsIterator,
    ) -> Self::Item<'a>;
    fn add_ids(ids: &mut Vec<LocalId>);
    fn default() -> LocalsStorage;
}

impl<T: SystemLocal> LocalQueryMut for &T {
    type Item<'i> = &'i T;
    type Owned = T;

    fn fetch<'a>(
        locals: &'a Rc<RefCell<LocalsStorage>>,
        ids: &mut LocalIdsIterator,
    ) -> Self::Item<'a> {
        let id = ids
            .next()
            .expect_fn(|| format!("expected id for local with type {}", tynm::type_name::<T>()));
        let locals = locals.borrow();
        let local = locals.get(id.0).expect_fn(|| {
            format!(
                "local of type {} and id {} should eixst by now",
                tynm::type_name::<T>(),
                id.0
            )
        });
        let local_casted = local.downcast_ref::<T>().expect_fn(|| {
            format!(
                "local with id {} should have the type {}",
                id.0,
                tynm::type_name::<T>()
            )
        });

        unsafe { &*(local_casted as *const T) }
    }

    fn add_ids(ids: &mut Vec<LocalId>) {
        let last = *ids.last().unwrap_or(&LocalId(0));
        ids.push(LocalId(last.0 + 1));
    }

    fn default() -> LocalsStorage {
        vec![Box::new(T::default())]
    }
}

impl<T: SystemLocal> LocalQueryMut for &mut T {
    type Item<'i> = &'i mut T;
    type Owned = T;

    fn fetch<'a>(
        locals: &'a Rc<RefCell<LocalsStorage>>,
        ids: &mut LocalIdsIterator,
    ) -> Self::Item<'a> {
        let id = ids
            .next()
            .expect_fn(|| format!("expected id for local with type {}", tynm::type_name::<T>()));
        let mut locals = locals.borrow_mut();
        let local = locals.get_mut(id.0).expect_fn(|| {
            format!(
                "local of type {} and id {} should eixst by now",
                tynm::type_name::<T>(),
                id.0
            )
        });
        let local_casted = local.downcast_mut::<T>().expect_fn(|| {
            format!(
                "local with id {} should have the type {}",
                id.0,
                tynm::type_name::<T>()
            )
        });

        unsafe { &mut *(local_casted as *mut T) }
    }

    fn add_ids(ids: &mut Vec<LocalId>) {
        if ids.is_empty() {
            ids.push(LocalId(0));
            return;
        }
        let last = *ids.last().expect("at least one element should be present");
        ids.push(LocalId(last.0 + 1));
    }

    fn default() -> LocalsStorage {
        vec![Box::new(T::default())]
    }
}
