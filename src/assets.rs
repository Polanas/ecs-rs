use std::marker::PhantomData;

use bevy_reflect::TypePath;

pub trait Asset: 'static + Send + Sync {}

pub struct AssetId<A: Asset> {
    marker: PhantomData<A>,
}
pub enum Handle<A: Asset> {
    Strong(),
    Weak(),
}
