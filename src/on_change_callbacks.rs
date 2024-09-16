use bevy_utils::HashMap;

use crate::{entity::Entity, identifier::Identifier, world::World};

pub trait OnAddCallback: 'static {
    fn run(&self, entity: Entity, world: World);
}

impl<T> OnAddCallback for T
where
    T: Fn(Entity, World) + 'static,
{
    fn run(&self, entity: Entity, world: World) {
        self(entity, world);
    }
}

pub trait OnRemoveCallback: 'static {
    fn run(&self, entity: Entity, world: World);
}

impl<T> OnRemoveCallback for T
where
    T: Fn(Entity, World) + 'static,
{
    fn run(&self, entity: Entity, world: World) {
        self(entity, world);
    }
}

pub struct OnChangeCallbacks {
    add_callbacks: HashMap<Identifier, Box<dyn OnAddCallback>>,
    remove_callbacks: HashMap<Identifier, Box<dyn OnRemoveCallback>>,
}

impl OnChangeCallbacks {
    pub fn new() -> Self {
        Self {
            add_callbacks: HashMap::new(),
            remove_callbacks: HashMap::new(),
        }
    }

    pub fn insert_add_callback(&mut self, component: Identifier, callback: Box<dyn OnAddCallback>) {
        self.add_callbacks.insert(component, callback);
    }

    pub fn insert_remove_callback(
        &mut self,
        component: Identifier,
        callback: Box<dyn OnRemoveCallback>,
    ) {
        self.remove_callbacks.insert(component, callback);
    }

    pub fn run_add_callback(&self, component: Identifier, entity: Identifier) {
        let Some(callback) = self.add_callbacks.get(&component) else {
            return;
        };
        callback.run(Entity(entity), World::default());
    }

    pub fn run_remove_callback(&self, component: Identifier, entity: Identifier) {
        let Some(callback) = self.remove_callbacks.get(&component) else {
            return;
        };
        callback.run(Entity(entity), World::default());
    }
}

impl Default for OnChangeCallbacks {
    fn default() -> Self {
        Self::new()
    }
}
