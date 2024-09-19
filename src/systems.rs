use std::{
    any::{Any, TypeId},
    cell::{Cell, RefCell},
    marker::PhantomData,
    rc::Rc,
};

use bevy_reflect::Reflect;
use bevy_utils::hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use crate::{events::CurrentSystemTypeId, world::World};

#[macro_export]
macro_rules! impl_system {
    ($t:ty, states) => {
        impl $crate::systems::System for $t {
            fn run(&mut self, world: &$crate::world::World, states: &$crate::systems::States) {
                Self::run(self, world, states);
            }
        }
    };
    ($t:ty) => {
        impl $crate::systems::System for $t {
            fn run(&mut self, world: &$crate::world::World, _states: &$crate::systems::States) {
                Self::run(self, world);
            }
        }
    };
}

#[macro_export]
macro_rules! impl_systems {
    (
        $($t:ty),+
    ) => {
        $(
            impl_system!($t);
        )+
    };
}

macro_rules! impl_system_data {
    (
        $(($params:ident, $fields:tt)),+
    ) =>
    {
        impl<$($params: SystemsData),+> SystemsData for ($($params),+,) {
            fn add_systems(self, systems: &mut Vec<(Box<dyn System>, SystemId)>) {
                $(
                    self.$fields.add_systems(systems);
                )+
            }

            // fn run_if<R: FnMut(&World) -> bool + 'static>(self, should_run: R) -> impl AbstractSystemsWithStateData {
            //     SystemsWithStateData {
            //         state_data: (),
            //         systems_data: self,
            //         should_run: Some(Box::new(should_run)),
            //     }
            // }

            fn with_state<S: StateData>(self, data: S) -> impl AbstractSystemsWithStateData
            where
                Self: Sized,
            {
                SystemsWithStateData {
                    state_data: data,
                    systems_data: self,
                    should_run: None,
                }
            }
        }
    };
}
impl_system_data!((T1, 0));
impl_system_data!((T1, 0), (T2, 1));
impl_system_data!((T1, 0), (T2, 1), (T3, 2));
impl_system_data!((T1, 0), (T2, 1), (T3, 2), (T4, 3));
impl_system_data!((T1, 0), (T2, 1), (T3, 2), (T4, 3), (T5, 4));
impl_system_data!((T1, 0), (T2, 1), (T3, 2), (T4, 3), (T5, 4), (T6, 5));
impl_system_data!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6)
);
impl_system_data!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7)
);
impl_system_data!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7),
    (T9, 8)
);
impl_system_data!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7),
    (T9, 8),
    (T10, 9)
);
impl_system_data!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7),
    (T9, 8),
    (T10, 9),
    (T11, 10)
);
impl_system_data!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7),
    (T9, 8),
    (T10, 9),
    (T11, 10),
    (T12, 11)
);

impl<T: System + 'static> AbstractSystemsWithStateData for T {
    fn add_state_ids(&self, _state_ids: &mut HashMap<TypeId, EnumId>) {}

    fn into_system_data(self, systems: &mut Systems, stage: SystemStage) -> SystemData {
        let ids = systems.system_data_ids(&HashMap::new());
        SystemData {
            stage,
            state_ids: ids,
            should_run: None,
            systems: vec![(Box::new(self), next_system_id())],
        }
    }

    fn run_if<R: FnMut(&World) -> bool + 'static>(
        self,
        should_run: R,
    ) -> impl AbstractSystemsWithStateData {
        SystemsWithStateData {
            state_data: (),
            systems_data: self,
            should_run: Some(Box::new(should_run)),
        }
    }

    fn add_systems(self, systems: &mut Vec<(Box<dyn System>, SystemId)>) {
        systems.push((Box::new(self), next_system_id()));
    }
}

macro_rules! impl_state_data {
    (
        $(($params:ident, $field:tt)),+
    ) =>
    {
        impl<$($params: StateData),+> StateData for ($($params),+,) {
            fn add_state_id(&self, state_ids: &mut HashMap<TypeId, EnumId>) {
                $(
                    self.$field.add_state_id(state_ids);
                )+
            }
        }
    };
}
impl_state_data!((T1, 0));
impl_state_data!((T1, 0), (T2, 1));
impl_state_data!((T1, 0), (T2, 1), (T3, 2));
impl_state_data!((T1, 0), (T2, 1), (T3, 2), (T4, 3));
impl_state_data!((T1, 0), (T2, 1), (T3, 2), (T4, 3), (T5, 4));
impl_state_data!((T1, 0), (T2, 1), (T3, 2), (T4, 3), (T5, 4), (T6, 5));
impl_state_data!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6)
);
impl_state_data!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7)
);
impl_state_data!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7),
    (T9, 8)
);
impl_state_data!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7),
    (T9, 8),
    (T10, 9)
);
impl_state_data!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7),
    (T9, 8),
    (T10, 9),
    (T11, 10)
);
impl_state_data!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7),
    (T9, 8),
    (T10, 9),
    (T11, 10),
    (T12, 11)
);

macro_rules! impl_systems_with_state {
    (
        $(($params:ident, $field:tt)),+
    ) =>
    {
        impl<$($params: SystemsData),+> AbstractSystemsWithStateData for ($($params),+,) {
            fn add_state_ids(&self, _: &mut HashMap<TypeId, EnumId>) {}

            fn add_systems(self, systems: &mut Vec<(Box<dyn System>, SystemId)>) {
                $(
                    self.$field.add_systems(systems);
                )+
            }

            fn into_system_data(self, systems: &mut Systems, stage: SystemStage) -> SystemData {
                let mut system_fns = vec![];
                let ids = systems.system_data_ids(&HashMap::new());
                AbstractSystemsWithStateData::add_systems(self, &mut system_fns);
                SystemData {
                    stage,
                    should_run: None,
                    systems: system_fns,
                    state_ids: ids,

                }
            }

            fn run_if<R: FnMut(&World) -> bool + 'static>(self, should_run: R) -> impl AbstractSystemsWithStateData {
                SystemsWithStateData {
                    state_data: (),
                    systems_data: self,
                    should_run: Some(Box::new(should_run)),
                }
            }
        }
    };
}

impl_systems_with_state!((T1, 0));
impl_systems_with_state!((T1, 0), (T2, 1));
impl_systems_with_state!((T1, 0), (T2, 1), (T3, 2));
impl_systems_with_state!((T1, 0), (T2, 1), (T3, 2), (T4, 3));
impl_systems_with_state!((T1, 0), (T2, 1), (T3, 2), (T4, 3), (T5, 4));
impl_systems_with_state!((T1, 0), (T2, 1), (T3, 2), (T4, 3), (T5, 4), (T6, 5));
impl_systems_with_state!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6)
);
impl_systems_with_state!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7)
);
impl_systems_with_state!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7),
    (T9, 8)
);
impl_systems_with_state!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7),
    (T9, 8),
    (T10, 9)
);
impl_systems_with_state!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7),
    (T9, 8),
    (T10, 9),
    (T11, 10)
);
impl_systems_with_state!(
    (T1, 0),
    (T2, 1),
    (T3, 2),
    (T4, 3),
    (T5, 4),
    (T6, 5),
    (T7, 6),
    (T8, 7),
    (T9, 8),
    (T10, 9),
    (T11, 10),
    (T12, 11)
);

impl StateData for () {
    fn add_state_id(&self, _: &mut HashMap<TypeId, EnumId>) {}
}

impl<T: SystemState> StateData for T {
    fn add_state_id(&self, state_ids: &mut HashMap<TypeId, EnumId>) {
        state_ids.insert(TypeId::of::<T>(), self.id());
    }
}

impl<F: SystemsData, S: StateData> AbstractSystemsWithStateData for SystemsWithStateData<F, S> {
    fn add_state_ids(&self, state_ids: &mut HashMap<TypeId, EnumId>) {
        self.state_data.add_state_id(state_ids);
    }

    fn into_system_data(self, systems: &mut Systems, stage: SystemStage) -> SystemData {
        let mut ids = HashMap::new();
        self.state_data.add_state_id(&mut ids);
        let ids = systems.system_data_ids(&ids);

        let mut systems = vec![];
        self.systems_data.add_systems(&mut systems);
        SystemData {
            stage,
            state_ids: ids,
            should_run: self.should_run,
            systems,
        }
    }

    fn run_if<R: FnMut(&World) -> bool + 'static>(
        self,
        should_run: R,
    ) -> impl AbstractSystemsWithStateData {
        SystemsWithStateData {
            state_data: self.state_data,
            systems_data: self.systems_data,
            should_run: Some(Box::new(should_run)),
        }
    }

    fn add_systems(self, systems: &mut Vec<(Box<dyn System>, SystemId)>) {
        self.systems_data.add_systems(systems);
    }
}

pub struct SystemsWithStateData<F: SystemsData, S: StateData> {
    state_data: S,
    systems_data: F,
    should_run: Option<Box<dyn ShouldRun>>,
}

pub trait System {
    fn run(&mut self, world: &World, states: &States);
}
pub trait ShouldRun {
    fn should_run(&mut self, world: &World) -> bool;
}
impl<T: FnMut(&World) -> bool> ShouldRun for T {
    fn should_run(&mut self, world: &World) -> bool {
        self(world)
    }
}

impl<T: FnMut(&World)> System for T {
    fn run(&mut self, world: &World, _states: &States) {
        self(world);
    }
}

// impl<T: FnMut(&World, &States)> System for T {
//     fn run(&mut self, world: &World, _states: &States) {
//         self(world, _states);
//     }
// }

pub trait StateData {
    fn add_state_id(&self, state_ids: &mut HashMap<TypeId, EnumId>);
}

pub trait SystemsData {
    fn with_state<S: StateData>(self, data: S) -> impl AbstractSystemsWithStateData
    where
        Self: Sized;
    fn add_systems(self, systems: &mut Vec<(Box<dyn System>, SystemId)>);
    // fn run_if<R: FnMut(&World) -> bool + 'static>(
    //     self,
    //     should_run: R,
    // ) -> impl AbstractSystemsWithStateData;
}

impl<T: System + 'static> SystemsData for T {
    fn with_state<S: StateData>(self, data: S) -> impl AbstractSystemsWithStateData
    where
        Self: Sized,
    {
        SystemsWithStateData {
            state_data: data,
            systems_data: self,
            should_run: None,
        }
    }

    fn add_systems(self, systems: &mut Vec<(Box<dyn System>, SystemId)>) {
        systems.push((Box::new(self), next_system_id()));
    }

    // fn run_if<R: FnMut(&World) -> bool + 'static>(
    //     self,
    //     should_run: R,
    // ) -> impl AbstractSystemsWithStateData {
    //     SystemsWithStateData {
    //         state_data: (),
    //         systems_data: self,
    //         should_run: Some(Box::new(should_run)),
    //     }
    // }
}

pub trait AbstractSystemsWithStateData {
    fn add_state_ids(&self, state_ids: &mut HashMap<TypeId, EnumId>);
    fn add_systems(self, systems: &mut Vec<(Box<dyn System>, SystemId)>);
    fn into_system_data(self, systems: &mut Systems, stage: SystemStage) -> SystemData;
    fn run_if<R: FnMut(&World) -> bool + 'static>(
        self,
        should_run: R,
    ) -> impl AbstractSystemsWithStateData;
}

pub trait SystemState: 'static {
    fn id(&self) -> EnumId;
}

pub type EnumId = u64;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum SystemStage {
    Init,
    Begin,
    PreUpdate,
    Update,
    PostUpdate,
    Last,
}

impl SystemStage {
    pub fn id(&self) -> EnumId {
        match self {
            SystemStage::Init => 0,
            SystemStage::Begin => 1,
            SystemStage::PreUpdate => 2,
            SystemStage::Update => 3,
            SystemStage::PostUpdate => 4,
            SystemStage::Last => 5,
        }
    }
}

pub struct SystemData {
    pub stage: SystemStage,
    pub state_ids: HashMap<TypeId, Option<EnumId>>,
    pub should_run: Option<Box<dyn ShouldRun>>,
    pub systems: Vec<(Box<dyn System>, SystemId)>,
}
type StatesMap = HashMap<TypeId, (EnumId, Rc<RefCell<dyn Any>>)>;
pub struct Systems {
    systems: Vec<SystemData>,
    states: Rc<RefCell<StatesMap>>,
}

pub struct StateGetter<T: 'static> {
    phantom_data: PhantomData<T>,
    state: Rc<RefCell<dyn Any>>,
}

impl<T: 'static> StateGetter<T> {
    fn new(state: Rc<RefCell<dyn Any>>) -> Self {
        Self {
            phantom_data: PhantomData,
            state,
        }
    }

    pub fn get<F, U>(&self, f: F) -> U
    where
        F: FnOnce(&T) -> U,
    {
        let state = self.state.borrow();
        let state = state.downcast_ref().unwrap();
        f(state)
    }

    pub fn get_mut<F, U>(&self, f: F) -> U
    where
        F: FnOnce(&mut T) -> U,
    {
        let mut state = self.state.borrow_mut();
        let state = state.downcast_mut().unwrap();
        f(state)
    }
}
pub struct States {
    #[allow(dead_code)]
    states: Rc<RefCell<StatesMap>>,
}

// impl<'a> States<'a> {
//     pub fn get_state<T: 'static>(&self) -> Option<StateGetter<T>> {
//         let type_id = TypeId::of::<T>();
//         self.states
//             .get(&type_id)
//             .map(|(_, state)| StateGetter::<T>::new(state.clone()))
//     }
//
//     pub fn state<T: 'static>(&self) -> StateGetter<T> {
//         let type_id = TypeId::of::<T>();
//         self.states
//             .get(&type_id)
//             .map(|(_, state)| StateGetter::<T>::new(state.clone()))
//             .unwrap()
//     }
// }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct SystemId(pub u64);

thread_local! {
    static SYSTEM_ID: Cell<u64> = const{ Cell::new(0) };
}

fn next_system_id() -> SystemId {
    let id = SYSTEM_ID.get();
    SYSTEM_ID.set(id + 1);
    SystemId(id)
}

impl Systems {
    pub fn new() -> Self {
        Self {
            systems: vec![],
            states: RefCell::new(HashMap::new()).into(),
        }
    }

    pub fn set_state_raw(
        &mut self,
        state: Rc<RefCell<dyn Any>>,
        type_id: TypeId,
        state_id: EnumId,
    ) {
        self.states.borrow_mut().insert(type_id, (state_id, state));

        for system in &mut self.systems {
            if !system.state_ids.contains_key(&type_id) {
                system.state_ids.insert(type_id, None);
            }
        }
    }

    pub fn set_state<T: SystemState>(&mut self, state: T) {
        let type_id = TypeId::of::<T>();
        let id = state.id();
        self.states
            .borrow_mut()
            .insert(type_id, (id, Rc::new(RefCell::new(state))));

        for system in &mut self.systems {
            if !system.state_ids.contains_key(&type_id) {
                system.state_ids.insert(type_id, None);
            }
        }
    }

    pub fn get_state<T: SystemState>(&self) -> Option<StateGetter<T>> {
        let type_id = TypeId::of::<T>();
        self.states
            .borrow()
            .get(&type_id)
            .map(|(_, state)| StateGetter::<T>::new(state.clone()))
    }

    pub fn has_state<T: SystemState>(&self) -> bool {
        let type_id = TypeId::of::<T>();
        self.states.borrow().contains_key(&type_id)
    }

    pub fn system_data_ids(
        &mut self,
        ids: &HashMap<TypeId, EnumId>,
    ) -> HashMap<TypeId, Option<EnumId>> {
        let mut ids: HashMap<_, _> = ids.iter().map(|(k, v)| (*k, Some(*v))).collect();
        for id in self.states.borrow().keys() {
            if !ids.contains_key(id) {
                ids.insert(*id, None);
            }
        }
        ids
    }

    pub fn add_systems<S: AbstractSystemsWithStateData + 'static>(
        &mut self,
        systems: S,
        stage: SystemStage,
    ) {
        let data = systems.into_system_data(self, stage);
        self.systems.push(data);
        self.systems.sort_by_key(|s| s.stage.id());
    }

    pub fn run(&mut self, world: &World) {
        let states = States {
            states: self.states.clone(),
        };
        self.systems.retain_mut(|s| {
            if s.stage == SystemStage::Init {
                s.systems
                    .iter_mut()
                    .for_each(|(s, _)| s.run(world, &states));
                return false;
            }
            true
        });
        for system_data in self.systems.iter_mut() {
            if system_data.state_ids.iter().all(|(k, v)| {
                let state = self.states.borrow().get(k).unwrap().0;
                v.map(|v| v == state).unwrap_or(true)
            }) && system_data
                .should_run
                .as_mut()
                .map(|f| f.should_run(world))
                .unwrap_or(true)
            {
                for (system, id) in system_data.systems.iter_mut() {
                    world.get_or_add_resource_mut(
                        || CurrentSystemTypeId::new(*id),
                        |current_id| {
                            current_id.value = *id;
                        },
                    );
                    system.run(world, &states);
                }
            }
        }
    }
}

impl Default for Systems {
    fn default() -> Self {
        Self::new()
    }
}
