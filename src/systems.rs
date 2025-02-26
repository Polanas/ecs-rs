use std::{
    any::{Any, TypeId},
    cell::{Cell, RefCell},
    marker::PhantomData,
    rc::Rc,
};

use bevy_reflect::Reflect;
use bevy_utils::hashbrown::HashMap;
use macro_rules_attribute::apply;
use serde::{Deserialize, Serialize};

use crate::{events::CurrentSystemId, world::World};

#[macro_export]
macro_rules! System {
    (
        $( #[$meta:meta] )*
    //  ^~~~attributes~~~~^
        $vis:vis struct $name:ident (
            $(
                $( #[$field_meta:meta] )*
    //          ^~~~field attributes~~~~^
                $field_vis:vis $field_ty:ty
    //          ^~~~~~a single field~~~~~~^
            ),*
        $(,)? );
    ) => {
        $( #[$meta] )*
        $vis struct $name (
            $(
                $( #[$field_meta] )*
                $field_vis $field_ty
            ),*
        );

        impl $crate::systems::IntoSystems<$crate::world::World> for $name {
            type System = Self;
            fn into_systems(self) -> $crate::systems::SystemWithState<Self::System> {
                $crate::systems::SystemWithState {
                    system: self,
                    should_run: None,
                    states: bevy_utils::hashbrown::HashMap::new(),
                }
            }
        }
    };
    {
        $( #[$meta:meta] )*
        $vis:vis struct $name:ident {
            $(
                $( #[$field_meta:meta] )*
                $field_vis:vis $field_name:ident : $field_ty:ty
            ),*
        $(,)? }
    } => {
        $( #[$meta] )*
        $vis struct $name {
            $(
                $( #[$field_meta] )*
                $field_vis $field_name : $field_ty
            ),*
        }

        impl $crate::systems::IntoSystems<$crate::world::World> for $name {
            type System = Self;
            fn into_systems(self) -> $crate::systems::SystemWithState<Self::System> {
                $crate::systems::SystemWithState {
                    system: self,
                    should_run: None,
                    states: bevy_utils::hashbrown::HashMap::new(),
                }
            }
        }
    }
}

pub trait System {
    fn run(&mut self, world: &World, context: &egui::Context);
    fn systems_vec(self) -> Vec<Box<dyn System>>
    where
        Self: Sized + 'static,
    {
        vec![Box::new(self)]
    }
}

pub trait IntoSystem<Input> {
    type System: System + 'static;

    fn into_system(self) -> SystemWithState<Self::System>;
    fn should_run(self, f: impl ShouldRunFn) -> SystemWithState<Self::System>
    where
        Self: Sized;
    fn with_state<S: StateData>(self, data: S) -> SystemWithState<Self::System>;
}

impl<F: FnMut(&World) + 'static> System for MyFunctionSystem<World, F> {
    fn run(&mut self, world: &World, _context: &egui::Context) {
        (self.f)(world);
    }

    fn systems_vec(self) -> Vec<Box<dyn System>> {
        vec![Box::new(self)]
    }
}

impl<F: FnMut(&World) + 'static> IntoSystem<World> for F {
    type System = MyFunctionSystem<World, Self>;

    fn into_system(self) -> SystemWithState<Self::System> {
        SystemWithState {
            system: MyFunctionSystem {
                f: self,
                marker: Default::default(),
            },
            should_run: None,
            states: HashMap::new(),
        }
    }

    fn should_run(self, f: impl ShouldRunFn) -> SystemWithState<Self::System>
    where
        Self: Sized,
    {
        SystemWithState {
            system: MyFunctionSystem {
                f: self,
                marker: Default::default(),
            },
            should_run: Some(Box::new(f)),
            states: HashMap::new(),
        }
    }

    fn with_state<S: StateData>(self, data: S) -> SystemWithState<Self::System> {
        let mut states = HashMap::new();
        data.add_state_id(&mut states);
        SystemWithState {
            system: MyFunctionSystem {
                f: self,
                marker: Default::default(),
            },
            should_run: None,
            states,
        }
    }
}

impl<F: FnMut(&World, &egui::Context) + 'static> System
    for MyFunctionSystem<(World, egui::Context), F>
{
    fn run(&mut self, world: &World, context: &egui::Context) {
        (self.f)(world, context);
    }

    fn systems_vec(self) -> Vec<Box<dyn System>> {
        vec![Box::new(self)]
    }
}

impl<F: FnMut(&World, &egui::Context) + 'static> IntoSystem<(World, egui::Context)> for F {
    type System = MyFunctionSystem<(World, egui::Context), Self>;

    fn into_system(self) -> SystemWithState<Self::System> {
        SystemWithState {
            system: MyFunctionSystem {
                f: self,
                marker: Default::default(),
            },
            should_run: None,
            states: HashMap::new(),
        }
    }

    fn should_run(self, f: impl ShouldRunFn) -> SystemWithState<Self::System>
    where
        Self: Sized,
    {
        SystemWithState {
            system: MyFunctionSystem {
                f: self,
                marker: Default::default(),
            },
            should_run: Some(Box::new(f)),
            states: HashMap::new(),
        }
    }

    fn with_state<S: StateData>(self, data: S) -> SystemWithState<Self::System> {
        let mut states = HashMap::new();
        data.add_state_id(&mut states);
        SystemWithState {
            system: MyFunctionSystem {
                f: self,
                marker: Default::default(),
            },
            should_run: None,
            states,
        }
    }
}
pub trait ShouldRunFn: 'static {
    fn should_run(&mut self, world: &World) -> bool;
}
impl<F: FnMut(&World) -> bool + 'static> ShouldRunFn for F {
    fn should_run(&mut self, world: &World) -> bool {
        (self)(world)
    }
}

impl<F: FnMut(&World) + 'static> IntoSystem<World> for SystemWithState<MyFunctionSystem<World, F>> {
    type System = MyFunctionSystem<World, F>;

    fn into_system(self) -> SystemWithState<Self::System> {
        SystemWithState {
            system: self.system,
            should_run: self.should_run,
            states: HashMap::new(),
        }
    }

    fn should_run(mut self, f: impl ShouldRunFn) -> SystemWithState<Self::System>
    where
        Self: Sized,
    {
        self.should_run = Some(Box::new(f));
        self
    }

    fn with_state<S: StateData>(mut self, data: S) -> SystemWithState<Self::System> {
        data.add_state_id(&mut self.states);
        self
    }
}
impl<F: FnMut(&World, &egui::Context) + 'static> IntoSystem<(World, egui::Context)>
    for SystemWithState<MyFunctionSystem<(World, egui::Context), F>>
{
    type System = MyFunctionSystem<(World, egui::Context), F>;

    fn into_system(self) -> SystemWithState<Self::System> {
        SystemWithState {
            system: self.system,
            should_run: self.should_run,
            states: HashMap::new(),
        }
    }

    fn should_run(mut self, f: impl ShouldRunFn) -> SystemWithState<Self::System>
    where
        Self: Sized,
    {
        self.should_run = Some(Box::new(f));
        self
    }

    fn with_state<S: StateData>(mut self, data: S) -> SystemWithState<Self::System> {
        data.add_state_id(&mut self.states);
        self
    }
}

pub struct SystemWithState<S: System> {
    pub system: S,
    pub should_run: Option<Box<dyn ShouldRunFn>>,
    pub states: HashMap<TypeId, EnumId>,
}

pub trait IntoSystems<Input> {
    type System: System + 'static;
    fn into_systems(self) -> SystemWithState<Self::System>;
}

#[macro_export]
macro_rules! call_16_times {
    ($target:ident) => {
        $target!(T1);
        $target!(T1, T2);
        $target!(T1, T2, T3);
        $target!(T1, T2, T3, T4);
        $target!(T1, T2, T3, T4, T5);
        $target!(T1, T2, T3, T4, T5, T6);
        $target!(T1, T2, T3, T4, T5, T6, T7);
        $target!(T1, T2, T3, T4, T5, T6, T7, T8);
        $target!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
        $target!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
        $target!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
        $target!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
        $target!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
        $target!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
        $target!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
        $target!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);
    };
}

macro_rules! impl_sytem_tuple {
    ($($param:ident),+) => {
        impl <$($param: System + 'static),+> System for ($($param),+,) {
            fn run(&mut self, world: &World, context: &egui::Context) {
                #[allow(non_snake_case)]
                let ($($param),+,) = self;
                $(
                    $param.run(world, context);
                )+
            }

            fn systems_vec(self) -> Vec<Box<dyn System>> {
                #[allow(non_snake_case)]
                let ($($param),+,) = self;
                let mut systems = vec![];
                $(
                    systems.append(&mut $param.systems_vec());
                )+
                systems
            }
        }
    };
}
call_16_times!(impl_sytem_tuple);

impl<F: FnMut(&World) + 'static> IntoSystems<World>
    for SystemWithState<MyFunctionSystem<World, F>>
{
    type System = MyFunctionSystem<World, F>;

    fn into_systems(self) -> SystemWithState<Self::System> {
        self
    }
}

impl<F: FnMut(&World, &egui::Context) + 'static> IntoSystems<(World, egui::Context)>
    for SystemWithState<MyFunctionSystem<(World, egui::Context), F>>
{
    type System = MyFunctionSystem<(World, egui::Context), F>;

    fn into_systems(self) -> SystemWithState<Self::System> {
        self
    }
}
impl<F: FnMut(&World) + 'static> IntoSystems<World> for F {
    type System = MyFunctionSystem<World, Self>;

    fn into_systems(self) -> SystemWithState<Self::System> {
        SystemWithState {
            system: MyFunctionSystem {
                f: self,
                marker: Default::default(),
            },
            should_run: None,
            states: HashMap::new(),
        }
    }
}
impl<F: FnMut(&World, &egui::Context) + 'static> IntoSystems<(World, egui::Context)> for F {
    type System = MyFunctionSystem<(World, egui::Context), Self>;

    fn into_systems(self) -> SystemWithState<Self::System> {
        SystemWithState {
            system: MyFunctionSystem {
                f: self,
                marker: Default::default(),
            },
            should_run: None,
            states: HashMap::new(),
        }
    }
}
macro_rules! impl_tuple_helper_input {
    (begin, $macro:ident, ($last_input:ident, $last_param:ident) $(,)?) => {};
    (begin, $macro:ident, ($input:ident,$param:ident), $(($rest_input:ident,$rest_param:ident)),+ $(,)?) => {
        $macro!($(($rest_input, $rest_param)),+);
    };
    ($macro:ident, ($input:ident,$param:ident), $(($rest_input:ident,$rest_param:ident)),+ $(,)?) => {
        $macro!(($input,$param), $(($rest_input, $rest_param)),+);
    };
}
macro_rules! impl_into_systems_tuples {
    ($(($input:ident, $param:ident)),+ $(,)?) => {
        impl_tuple_helper_input!(
            begin,
            impl_into_systems_tuples,
            $(($input,$param)),+
        );
        impl <$($input, $param: IntoSystems<$input>),+> IntoSystems<($($input),+,)> for ($($param),+,)
        {
            type System = ($($param::System),+,);

            fn into_systems(self) -> SystemWithState<Self::System> {
                #[allow(non_snake_case)]
                let ($($param),+,) = self;
                SystemWithState {
                    system:
                        ($($param.into_systems().system),+,),
                    should_run: None,
                    states: HashMap::new(),
                }
            }
        }
    };
}

impl_tuple_helper_input!(
    impl_into_systems_tuples,
    (I1, T1),
    (I2, T2),
    (I3, T3),
    (I4, T4),
    (I5, T5),
    (I6, T6),
    (I7, T7),
    (I8, T8),
    (I9, T9),
    (I10, T10),
    (I11, T11),
    (I12, T12),
    (I13, T13),
    (I14, T14),
    (I15, T15),
    (I16, T16),
);

fn test() {
    #[derive(Debug, Hash)]
    enum State {
        S1,
        S2,
    }
    impl_system_state!(State);
    fn add_systems<I, S: System>(into: impl IntoSystems<I, System = S>) {
        let systems = into.into_systems();
    }
    fn system1(world: &World, e: &egui::Context) {}
    fn system2(world: &World) {}
    add_systems((|w: &World| {},));
    // add_systems::<
    //     ((World, egui::Context), World),
    //     (
    //         MyFunctionSystem<(World, egui::Context), _>,
    //         MyFunctionSystem<World, _>,
    //     ),
    // >((
    //     system1.my_run_if(|w: &World| true).my_with_state(State::S1),
    //     system2.my_run_if(|w: &World| true).my_with_state(State::S2),
    // ));
    add_systems((
        system1.should_run(|w: &World| true).with_state(State::S1),
        system2.should_run(|w: &World| true).with_state(State::S2),
    ));
}
pub struct MyFunctionSystem<Input, F> {
    f: F,
    //that's strange but ok
    marker: PhantomData<fn() -> Input>,
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

impl StateData for () {
    fn add_state_id(&self, _: &mut HashMap<TypeId, EnumId>) {}
}

impl<T: SystemState> StateData for T {
    fn add_state_id(&self, state_ids: &mut HashMap<TypeId, EnumId>) {
        state_ids.insert(TypeId::of::<T>(), self.id());
    }
}

pub trait StateData {
    fn add_state_id(&self, state_ids: &mut HashMap<TypeId, EnumId>);
}

pub trait SystemState: 'static {
    fn id(&self) -> EnumId;
}

pub type EnumId = u64;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum SystemStage {
    PreInit,
    Init,
    PostInit,
    First,
    PreUpdate,
    Update,
    PostUpdate,
    Last,
}

impl SystemStage {
    pub fn id(&self) -> EnumId {
        match self {
            SystemStage::PreInit => 0,
            SystemStage::Init => 1,
            SystemStage::PostInit => 2,
            SystemStage::First => 3,
            SystemStage::PreUpdate => 4,
            SystemStage::Update => 5,
            SystemStage::PostUpdate => 6,
            SystemStage::Last => 7,
        }
    }
}

pub struct SystemData {
    pub stage: SystemStage,
    pub state_ids: HashMap<TypeId, Option<EnumId>>,
    pub should_run: Option<Box<dyn ShouldRunFn>>,
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
#[apply(FromIntoLua)]
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

    pub fn add_systems<S: System + 'static>(
        &mut self,
        systems: SystemWithState<S>,
        stage: SystemStage,
    ) {
        let data = SystemData {
            stage,
            state_ids: self.system_data_ids(&systems.states),
            should_run: systems.should_run,
            systems: systems
                .system
                .systems_vec()
                .into_iter()
                .map(|s| (s, next_system_id()))
                .collect(),
        };
        self.systems.push(data);
        self.systems.sort_by_key(|s| s.stage.id());
    }

    pub fn init(&mut self, world: &World, context: &egui::Context) {
        self.systems.retain_mut(|s| {
            if matches!(
                s.stage,
                SystemStage::PreInit | SystemStage::Init | SystemStage::PostInit
            ) {
                s.systems
                    .iter_mut()
                    .for_each(|(s, _)| s.run(world, context));
                return false;
            }
            true
        });
    }

    pub fn run(&mut self, world: &World, context: &egui::Context) {
        //TODO: why not make separate add and init methods?
        self.systems.retain_mut(|s| {
            if matches!(
                s.stage,
                SystemStage::PreInit | SystemStage::Init | SystemStage::PostInit
            ) {
                s.systems
                    .iter_mut()
                    .for_each(|(s, _)| s.run(world, context));
                false
            } else {
                true
            }
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
                        || CurrentSystemId::new(*id),
                        |current_id| {
                            current_id.value = *id;
                        },
                    );
                    system.run(world, context);
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
