use std::{alloc::Layout, marker::PhantomData};

use bevy_ptr::OwningPtr;
use bevy_utils::HashMap;

use crate::{archetypes::TEMP_CAPACITY, blob_vec::BlobVec, identifier::Identifier, table::Storage};

use super::component::AbstractComponent;

pub struct TempComponentsStorage {
    pub storages: HashMap<Identifier, Storage>,
}

impl TempComponentsStorage {
    pub fn new() -> Self {
        Self {
            storages: HashMap::new(),
        }
    }

    pub fn add_comp<T: AbstractComponent>(&mut self, component: Identifier, value: T) -> usize {
        let storage = &mut self.storage::<T>(component);
        storage.push(value);
        storage.len() - 1
    }

    pub fn remove_comp(&mut self, component: Identifier) -> OwningPtr {
        let storage = self.get_storage(component);
        unsafe { storage.0.swap_remove_and_forget_unchecked(0) }
    }

    pub fn storage<T: AbstractComponent>(&mut self, component: Identifier) -> &mut Storage {
        let layout = Layout::new::<T>();
        self.storages
            .entry(component)
            .or_insert(unsafe { Storage(BlobVec::new(layout, None, TEMP_CAPACITY)) })
    }

    pub fn get_storage(&mut self, component: Identifier) -> &mut Storage {
        self.storages.get_mut(&component).unwrap()
    }
}

impl Default for TempComponentsStorage {
    fn default() -> Self {
        Self::new()
    }
}
