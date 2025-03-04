use std::{
    any::TypeId,
    cell::{Ref, RefCell},
    rc::Rc,
};

use smol_str::{SmolStr, ToSmolStr};

use crate::{
    archetypes::{
        self, Archetypes, EntityKind, InstanceOf, MyTypeRegistry, Prefab, StateOperation, ENTITY_ID,
    },
    components::{component::AbstractComponent, register::RegisterComponentQuery},
    entity::Entity,
    entity_parser::ParseError,
    events::{self, CurrentSystemId, Event, EventReader, Events},
    expect_fn::ExpectFnResult,
    locals::{LocalId, LocalQueryMut, SystemLocals},
    lua_api::LuaApi,
    on_change_callbacks::{OnAddCallback, OnRemoveCallback},
    plugin::Plugin,
    query::{QueryData, QueryFilterData, QueryState},
    resources::ResourceQuery,
    systems::{
        IntoSystem, IntoSystems, StateGetter, System, SystemId, SystemStage, SystemState, Systems,
    },
    table::TableId,
};

#[derive(Default)]
pub struct World {
    currently_running_systems: bool,
}

impl Clone for World {
    fn clone(&self) -> Self {
        Self {
            currently_running_systems: self.currently_running_systems,
        }
    }
}

pub fn archetypes<F, U>(f: F) -> U
where
    F: FnOnce(&Archetypes) -> U,
{
    ARCHETYPES.with(|a| f(a.borrow().as_ref().unwrap()))
}

pub fn archetypes_mut<F, U>(f: F) -> U
where
    F: FnOnce(&mut Archetypes) -> U,
{
    ARCHETYPES.with(|a| f(a.borrow_mut().as_mut().unwrap()))
}

pub fn drop_archetypes() {
    ARCHETYPES.with(|a| *a.borrow_mut() = None);
}

thread_local! {
    pub static ARCHETYPES: RefCell<Option<Archetypes>> = const { RefCell::new(None) };
}

impl World {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        ARCHETYPES.with(|a| {
            *a.borrow_mut() = Some(Archetypes::new());
        });
        let world = Self {
            currently_running_systems: false,
        };
        world.add_resource(CurrentSystemId::new(SystemId(0)));
        world.add_resource(SystemLocals::new());
        world
    }

    pub fn lua_api(&self) -> Rc<RefCell<LuaApi>> {
        archetypes(|archetypes| archetypes.lua_api().clone())
    }

    pub fn debug_tables_info(&self) -> Vec<(TableId, String)> {
        archetypes(|a| a.debug_tables_info())
    }

    pub fn debug_tables_components_info(&self) -> Vec<(TableId, String)> {
        archetypes(|a| a.debug_tables_components_info())
    }

    pub fn type_registry(&self) -> Rc<RefCell<MyTypeRegistry>> {
        archetypes(|a| a.type_registry_rc().clone())
    }

    pub fn entity_by_global_name(&self, name: &str) -> Option<Entity> {
        archetypes_mut(|a| a.entity_by_global_name(name.to_smolstr())).map(|id| id.into())
    }

    pub fn register_components<T: RegisterComponentQuery>(&self) {
        T::register();
    }

    pub fn deserialize_entity(&self, json: &str) -> Result<Entity, ParseError> {
        archetypes_mut(|a| a.deserialize_entity(json))
    }

    pub fn send_event<T: Event>(&self, event: T) {
        self.resources::<&mut Events<T>>(|events| {
            events.push(event);
        });
    }

    pub fn event_reader<T: Event>(&self) -> Rc<RefCell<EventReader<T>>> {
        self.resources_ret::<(&CurrentSystemId, &mut Events<T>), _>(|(system_id, events)| {
            events.event_reader(system_id.value)
        })
    }

    pub fn add_event_type<T: Event>(&self) -> Self {
        let events = Events::<T>::new();
        self.add_resource(events);
        self.add_systems(events::default_cleanup_system::<T>, SystemStage::Last);
        self.clone()
    }

    pub fn comp_entity<T: AbstractComponent>(&self) -> Entity {
        archetypes_mut(|a| Entity::new(a.component_id::<T>()))
    }

    pub fn add_systems<I, S: System + 'static>(
        &self,
        into: impl IntoSystems<I, System = S>,
        stage: SystemStage,
    ) -> Self {
        let systems = into.into_systems();
        archetypes_mut(|a| {
            a.systems()
                .try_borrow_mut()
                .expect("attemt to add a new system while inside another system (or systems is borrowed for some other reason)")
                .add_systems(systems, stage);
        });
        self.clone()
    }

    pub fn add_plugins<P: Plugin>(&self, plugin: P) -> Self {
        plugin.build(self);
        self.clone()
    }

    pub fn on_comp_add<T: AbstractComponent>(&self, callback: impl Fn(Entity, World) + 'static) {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|a| {
            let id = a.component_id::<T>();
            a.insert_add_callback(id, Box::new(callback));
        })
    }

    pub fn on_comp_remove<T: AbstractComponent>(&self, callback: impl Fn(Entity, World) + 'static) {
        assert!(std::mem::size_of::<T>() > 0);
        archetypes_mut(|a| {
            let id = a.component_id::<T>();
            a.insert_remove_callback(id, Box::new(callback));
        })
    }

    pub fn on_tag_add<T: AbstractComponent>(callback: impl OnAddCallback) {
        assert!(std::mem::size_of::<T>() == 0);
        archetypes_mut(|a| {
            let id = a.component_id::<T>();
            a.insert_add_callback(id, Box::new(callback));
        })
    }

    pub fn on_tag_remove<T: AbstractComponent>(callback: impl OnRemoveCallback) {
        assert!(std::mem::size_of::<T>() == 0);
        archetypes_mut(|a| {
            let id = a.component_id::<T>();
            a.insert_remove_callback(id, Box::new(callback));
        })
    }

    pub fn set_state<T: SystemState>(&self, state: T) -> Self {
        if !self.currently_running_systems {
            archetypes_mut(|a| {
                a.systems().borrow_mut().set_state(state);
            });

            return self.clone();
        }
        archetypes_mut(|a| {
            a.state_operations().borrow_mut().push(StateOperation {
                type_id: TypeId::of::<T>(),
                state_id: state.id(),
                state: Rc::new(RefCell::new(state)),
            });
        });
        self.clone()
    }

    pub fn get_state<T: SystemState>(&self) -> Option<StateGetter<T>> {
        archetypes_mut(|a| a.systems().borrow().get_state::<T>())
    }

    pub fn state<T: SystemState>(&self) -> StateGetter<T> {
        let systems = archetypes_mut(|a| a.systems().clone());
        let systems = systems.borrow();
        systems.get_state::<T>().unwrap()
    }

    pub fn run(&mut self, context: &egui::Context) {
        self.resources::<&mut SystemLocals>(|locals| locals.reset_ids());
        self.remove_empty_entities();
        let systems = archetypes_mut(|a| a.systems().clone());
        self.currently_running_systems = true;
        let mut systems = systems.borrow_mut();
        systems.run(self, context);
        self.currently_running_systems = false;
        self.process_state_operations(&mut systems);
    }

    fn remove_empty_entities(&self) {
        for entity in self
            .query::<&Entity>()
            .with_ent_tag(Entity::new(ENTITY_ID))
            .build()
            .iter()
        {
            entity.remove();
        }
    }

    fn process_state_operations(&mut self, systems: &mut Systems) {
        archetypes_mut(|a| {
            for op in a.state_operations().borrow_mut().drain(..) {
                systems.set_state_raw(op.state, op.type_id, op.state_id);
            }
        })
    }

    //TODO: add optional version of this method: get_resources
    pub fn resources<T: ResourceQuery>(&self, f: impl for<'i> FnOnce(T::Item<'i>)) {
        let resources = archetypes(|a| a.resources().clone());
        f(T::fetch(&resources));
    }

    pub fn resources_ret<T: ResourceQuery, R>(
        &self,
        f: impl for<'i> FnOnce(T::Item<'i>) -> R,
    ) -> R {
        let resources = archetypes(|a| a.resources().clone());
        f(T::fetch(&resources))
    }

    pub fn add_resource<T: 'static>(&self, resource: T) -> Self {
        archetypes_mut(|a| a.add_resource(resource));
        self.clone()
    }

    pub fn get_or_add_resource_mut<T: 'static>(
        &self,
        init: impl FnOnce() -> T,
        get: impl FnOnce(&mut T),
    ) {
        if !self.resource_exists::<T>() {
            self.add_resource(init());
        } else {
            let resources = archetypes(|a| a.resources().clone());
            get(<&mut T as ResourceQuery>::fetch(&resources));
        }
    }

    pub fn get_or_add_resource<T: AbstractComponent>(
        &self,
        init: impl FnOnce() -> T,
        get: impl for<'i> FnOnce(&T),
    ) {
        if !self.resource_exists::<T>() {
            self.add_resource(init());
        }
        let resources = archetypes(|a| a.resources().clone());
        get(<&T as ResourceQuery>::fetch(&resources));
    }

    pub fn remove_resource<T: 'static>(&self) -> Self {
        archetypes_mut(|a| a.remove_resource::<T>());
        self.clone()
    }

    pub fn resource_exists<T: 'static>(&self) -> bool {
        archetypes_mut(|a| a.resource_exists::<T>())
    }

    pub fn add_instance_of(&self, prefab: &Entity) -> Entity {
        let entity = prefab.deep_clone();
        entity.remove_tag::<Prefab>();
        entity.add_mixed_tag_rel::<InstanceOf>(prefab)
    }

    pub fn add_entity_named(&self, name: &str) -> Entity {
        let id = archetypes_mut(|a| a.add_entity(EntityKind::Regular));
        let entity = Entity::new(id);
        entity.set_name(name);
        entity
    }

    pub fn add_entity(&self) -> Entity {
        let id = archetypes_mut(|a| a.add_entity(EntityKind::Regular));
        Entity::new(id)
    }

    pub fn add_prefab_named(&self, name: &str) -> Entity {
        let mut prefab = self.add_entity();
        prefab.set_name(name);
        prefab.add_tag::<Prefab>()
    }

    pub fn add_prefab(&self) -> Entity {
        let mut prefab = self.add_entity();
        prefab.add_tag::<Prefab>()
    }

    pub fn query<D: QueryData>(&self) -> QueryState<D, ()> {
        QueryState::new()
    }

    pub fn empty_query(&self) -> QueryState<(), ()> {
        QueryState::new()
    }

    pub fn query_filtered<D: QueryData, F: QueryFilterData>(&self) -> QueryState<D, F> {
        QueryState::new()
    }

    pub fn empty_query_filtered<F: QueryFilterData>(&self) -> QueryState<(), F> {
        QueryState::new()
    }
}

pub(crate) struct WorldInner {}

impl WorldInner {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for WorldInner {
    fn default() -> Self {
        Self::new()
    }
}
