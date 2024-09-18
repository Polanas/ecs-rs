use bevy_ptr::Ptr;
use bevy_reflect::Reflect;
use std::{any::Any, cell::RefMut, fmt::Debug};

use crate::{systems::EnumId, table::Storage};

#[macro_export]
macro_rules! impl_component {
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
        #[derive(Clone, bevy_reflect::Reflect, serde::Serialize, serde::Deserialize)]
        $vis struct $name (
            $(
                $( #[$field_meta] )*
                $field_vis $field_ty
            ),*
        );

        impl $crate::components::component::AbstractComponent for $name {
            fn clone_into(
                value: bevy_ptr::Ptr<'_>,
                mut storage: std::cell::RefMut<$crate::table::Storage>,
            ) {
                let value = unsafe { value.deref::<$name>() };
                storage.push(value.clone());
            }
            fn as_reflect_ref(value: bevy_ptr::Ptr<'_>, f: &dyn Fn(Option<&dyn bevy_reflect::Reflect>)) {
                let value = unsafe { value.deref::<$name>() };
                f(Some(value as &dyn bevy_reflect::Reflect));
            }
            fn as_reflect_mut(value: bevy_ptr::PtrMut<'_>, f: &dyn Fn(Option<&mut dyn bevy_reflect::Reflect>)) {
                let value = unsafe { value.deref_mut::<$name>() };
                f(Some(value as &mut dyn bevy_reflect::Reflect));
            }
            fn serialize(value: bevy_ptr::Ptr<'_>) -> Result<serde_json::Value, serde_json::error::Error> {
                let value = unsafe { value.deref::<$name>() };
                serde_json::to_value(&value)
            }

            fn deserialize(
                value: serde_json::Value,
                mut storage: std::cell::RefMut<$crate::table::Storage>,
            ) -> serde_json::Result<()> {
                let value = serde_json::from_value::<Self>(value)?;
                storage.push(value);
                Ok(())
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
        #[derive(Clone, bevy_reflect::Reflect, serde::Serialize, serde::Deserialize)]
        $vis struct $name {
            $(
                $( #[$field_meta] )*
                $field_vis $field_name : $field_ty
            ),*
        }
        impl $crate::components::component::AbstractComponent for $name {
            fn clone_into(
                value: bevy_ptr::Ptr<'_>,
                mut storage: std::cell::RefMut<$crate::table::Storage>,
            ) {
                let value = unsafe { value.deref::<$name>() };
                storage.push(value.clone());
            }

            fn as_reflect_ref(value: bevy_ptr::Ptr<'_>, f: &dyn Fn(Option<&dyn bevy_reflect::Reflect>)) {
                let value = unsafe { value.deref::<$name>() };
                f(Some(value as &dyn bevy_reflect::Reflect));
            }
            fn as_reflect_mut(value: bevy_ptr::PtrMut<'_>, f: &dyn Fn(Option<&mut dyn bevy_reflect::Reflect>)) {
                let value = unsafe { value.deref_mut::<$name>() };
                f(Some(value as &mut dyn bevy_reflect::Reflect));
            }

            fn serialize(value: bevy_ptr::Ptr<'_>) -> Result<serde_json::Value, serde_json::error::Error> {
                let value = unsafe { value.deref::<$name>() };
                serde_json::to_value(&value)
            }

            fn deserialize(
                value: serde_json::Value,
                mut storage: std::cell::RefMut<$crate::table::Storage>,
            ) -> serde_json::Result<()> {
                let value = serde_json::from_value::<Self>(value)?;
                storage.push(value);
                Ok(())
            }
        }
    }
}

#[macro_export]
macro_rules! impl_system_state {
    ($t:ty) => {
        impl $crate::systems::SystemState for $t {
            fn id(&self) -> $crate::systems::EnumId {
                let mut hasher = DefaultHasher::new();
                std::mem::discriminant(self).hash(&mut hasher);
                hasher.finish()
            }
        }
    };
}

#[macro_export]
macro_rules! impl_system_states {
    (
        $($t:ty),+
    ) => {
        $(
            impl_system_state!($t);
        )+
    };
}
#[macro_export]
macro_rules! enum_tag {
    ($(#[$meta:meta])? $vis:vis enum $name:ident {
        $($(#[$vmeta:meta])? $vname:ident $(,)?)*
    }) => {
        $(#[$meta])?
        #[derive(Clone, Copy, bevy_reflect::Reflect, serde::Serialize, serde::Deserialize)]
        $vis enum $name {
            $($(#[$vmeta])? $vname,)*
        }

        impl $crate::components::component::AbstractComponent for $name {
            fn clone_into(
                value: bevy_ptr::Ptr<'_>,
                mut storage: std::cell::RefMut<$crate::table::Storage>,
            ) {
                let value = unsafe { value.deref::<$name>() };
                storage.push(value.clone());
            }

            fn as_reflect_ref(value: bevy_ptr::Ptr<'_>, f: &dyn Fn(Option<&dyn bevy_reflect::Reflect>)) {
                let value = unsafe { value.deref::<$name>() };
                f(Some(value as &dyn bevy_reflect::Reflect));
            }
            fn as_reflect_mut(value: bevy_ptr::PtrMut<'_>, f: &dyn Fn(Option<&mut dyn bevy_reflect::Reflect>)) {
                let value = unsafe { value.deref_mut::<$name>() };
                f(Some(value as &mut dyn bevy_reflect::Reflect));
            }
            fn serialize(value: bevy_ptr::Ptr<'_>) -> Result<serde_json::Value, serde_json::error::Error> {
                let value = unsafe { value.deref::<$name>() };
                serde_json::to_value(&value)
            }

            fn deserialize(
                value: serde_json::Value,
                mut storage: std::cell::RefMut<$crate::table::Storage>,
            ) -> serde_json::Result<()> {
                let value = serde_json::from_value::<Self>(value)?;
                storage.push(value);
                Ok(())
            }
        }

        impl $crate::components::component::EnumTag for $name {
            fn id(&self) -> $crate::systems::EnumId {
                match self {
                    $(
                        $name::$vname => $name::$vname as $crate::systems::EnumId,
                    )*
                }
            }

            fn from_id(id: $crate::systems::EnumId) -> Option<Self> {
                match id {
                    $(
                        id if id == $name::$vname as $crate::systems::EnumId => Some($name::$vname),
                    )*
                    _ => None
                }
            }
        }
    };
}
impl_component! {
    pub struct ChildOf {}
}
pub trait AbstractComponent: 'static + Sized {
    fn clone_into(value: Ptr<'_>, storage: RefMut<Storage>);
    fn as_reflect_ref(value: bevy_ptr::Ptr<'_>, f: &dyn Fn(Option<&dyn Reflect>));
    fn as_reflect_mut(value: bevy_ptr::PtrMut<'_>, f: &dyn Fn(Option<&mut dyn Reflect>));
    fn serialize(value: bevy_ptr::Ptr<'_>) -> Result<serde_json::Value, serde_json::error::Error>;
    fn deserialize(
        value: serde_json::Value,
        storage: RefMut<Storage>,
    ) -> serde_json::Result<()>;
}
pub trait EnumTag: AbstractComponent + 'static {
    fn id(&self) -> EnumId;
    fn from_id(id: EnumId) -> Option<Self>;
}
