use crate::{systems::EnumId, table::Storage};
use bevy_ptr::Ptr;
use bevy_reflect::Reflect;
use macro_rules_attribute::apply;
use std::{any::Any, cell::RefMut, fmt::Debug};

#[macro_export]
macro_rules! Component {
    //Named variant
    (@return $table:ident $enum_name:ident $var_name:ident { $($field_name:ident: $field_ty:ty),* $(,)?}) => {
        return Ok(
            $enum_name::$var_name {
                $(
                    $field_name: $table.get(stringify!($field_name))?
                ),*
            }
        )
    };
    //Tuple variant
    (@return $table:ident $enum_name:ident $var_name:ident ($($field_ty:ty),*)) => {
        return Ok(
            $enum_name::$var_name {
                $(
                    ${ignore($field_ty)}
                    ${index()}: $table.get(${index()} + 1)?
                ),*
            }
        )
    };
    (@return $table:ident $enum_name:ident $var_name:ident)  => {};
    //Named variant
    (@set $self: ident $table:ident $enum_name:ident $var_name:ident { $($field_name:ident: $field_ty:ty),* $(,)?}) => {
        if let $enum_name::$var_name {
            $($field_name),*
        } = $self {
            $(
                $table.set(stringify!($field_name), $field_name)?;
            )*
        }
    };
    //Tuple variant
    (@set $self:ident $table:ident $enum_name:ident $var_name:ident ($($field_ty:ty),*)) => {
        if let $enum_name::$var_name($(
                ${ignore($field_ty)}
                paste::paste!([<$var_name _ ${index()}>])
        ),+) = $self {
            $(
                ${ignore($field_ty)}
                $table.set(${index()} + 1, paste::paste!([<$var_name _ ${index()}>]))?;
                // $table.set(0,10)?;
            )+
        }
    };
    //Unit variant
    (@set $self:ident $table:ident $enum_name:ident $var_name:ident) => {};
    // VariantName
    (
        $vis:vis,
        $( #[$meta:meta] )*
        $name:ident
        @variants [
            $($variants:tt)*
        ]
        @parsing
            $( #[$var_meta:meta] )*
            $VariantName:ident
            $(, $($input:tt)*)?
    ) => (Component! {
        $vis,
        $( #[$meta] )*
        $name
        @variants [
            $($variants)*
            {
                $( #[$var_meta] )*
                $VariantName
            }
        ]
        @parsing
            $( $($input)* )?
    });

    // VariantName(...)
    (
        $vis:vis,
        $( #[$meta:meta] )*
        $name:ident
        @variants [
            $($variants:tt)*
        ]
        @parsing
            $( #[$var_meta:meta] )*
            $VariantName:ident ( $($tt:tt)* )
            $(, $($input:tt)*)?
    ) => (Component! {
        $vis,
        $( #[$meta] )*
        $name
        @variants [
            $($variants)*
            {
                $( #[$var_meta] )*
                $VariantName ($($tt)*)
            }
        ]
        @parsing
            $( $($input)* )?
    });

    // VariantName { ... }
    (
        $vis:vis,
        $( #[$meta:meta] )*
        $name:ident
        @variants [
            $($variants:tt)*
        ]
        @parsing
            $( #[$var_meta:meta] )*
            $VariantName:ident { $($tt:tt)* }
            $(, $($input:tt)*)?
    ) => (Component! {
        $vis,
        $( #[$meta] )*
        $name
        @variants [
            $($variants)*
            {
                $( #[$var_meta] )*
                $VariantName { $($tt)* }
            }
        ]
        @parsing
            $( $($input)* )?
    });

    // Done parsing, time to generate code:
    (
        $vis:vis,
        $( #[$meta:meta] )*
        $name:ident
        @variants [
            $(
                {
                    $( #[$var_meta:meta] )*
                    $VariantName:ident $($variant_assoc:tt)?
                }
            )*
        ]
        @parsing
            // Nothing left to parse
    ) => (
        $( #[$meta] )*
        #[macro_rules_attribute::apply($crate::FromIntoLua)]
        #[derive(educe::Educe)]
        #[educe(Debug)]
        #[derive(Clone, bevy_reflect::Reflect, serde::Serialize, serde::Deserialize)]
        $vis enum $name {
            $(
                $VariantName $(
                    $variant_assoc
                )? ,
            )*
        }
        impl $crate::components::component::AbstractComponent for $name {
            fn to_debug_string(value: bevy_ptr::Ptr<'_>) -> String {
                format!("{:#?}",unsafe {value.deref::<$name>()})
            }

            fn clone_into(
                index: usize,
                src: Option<std::cell::RefMut<$crate::table::Storage>>,
                mut dst: std::cell::RefMut<$crate::table::Storage>,
            ) {
                let value = unsafe {
                        if let Some(src) = src {
                            let ptr = src.0.get_checked(index);
                            ptr.deref::<$name>().clone()
                        }
                        else {
                            let ptr = dst.0.get_checked(index);
                            ptr.deref::<$name>().clone()
                        }
                };
                dst.push(value);
            }

            fn as_reflect_ref(value: bevy_ptr::Ptr<'_>) -> Option<&dyn bevy_reflect::Reflect>  {
                let value = unsafe { value.deref::<$name>() };
                (Some(value as &dyn bevy_reflect::Reflect))
            }
            fn as_reflect_mut(value: bevy_ptr::PtrMut<'_>) -> Option<&mut dyn bevy_reflect::Reflect>  {
                let value = unsafe { value.deref_mut::<$name>() };
                (Some(value as &mut dyn bevy_reflect::Reflect))
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

            fn into_lua<'a>(value: bevy_ptr::Ptr<'_>, lua: &'a mlua::Lua) -> mlua::Result<mlua::Value<'a>> {
                let value = unsafe { value.deref::<$name>() };
                <$name as mlua::IntoLua>::into_lua(value.clone(), lua)
            }

            fn from_lua<'lua>(mlua_value: mlua::Value<'lua>, mut storage: std::cell::RefMut<$crate::table::Storage>, table_row: Option<usize>, lua: &'lua mlua::Lua) -> mlua::Result<()> {
                match <$name as mlua::FromLua>::from_lua(mlua_value,lua) {
                    Ok(new_value) => {
                        if let Some(table_row) = table_row {
                            storage.replace_unchecked(table_row, new_value);
                        } else {
                            storage.push(new_value);
                        }
                        Ok(())
                    },
                    Err(error) => Err(error)
                }
            }
        }

    );

            // == ENTRY POINT ==
            (
                $( #[$meta:meta] )*
                $vis:vis enum $name:ident {
                    $($tt:tt)*
                }
            ) => (Component! {
                $vis,
                $( #[$meta] )*
                $name
                // a sequence of brace-enclosed variants
                @variants []
                // remaining tokens to parse
                @parsing
                    $($tt)*
            });
    (
        $( #[$meta:meta] )*
        $vis:vis struct $name:ident;
    ) => {
        #[macro_rules_attribute::apply($crate::FromIntoLua)]
        #[derive(educe::Educe)]
        #[educe(Debug)]
        #[derive(Clone, bevy_reflect::Reflect, serde::Serialize, serde::Deserialize)]
        $( #[$meta] )*
        $vis struct $name;

        impl $crate::components::component::AbstractComponent for $name {
            fn to_debug_string(value: bevy_ptr::Ptr<'_>) -> String {
                format!("{:#?}",unsafe {value.deref::<$name>()})
            }

            fn clone_into(
                index: usize,
                src: Option<std::cell::RefMut<$crate::table::Storage>>,
                mut dst: std::cell::RefMut<$crate::table::Storage>,
            ) {
                let value = unsafe {
                        if let Some(src) = src {
                            let ptr = src.0.get_checked(index);
                            ptr.deref::<$name>().clone()
                        }
                        else {
                            let ptr = dst.0.get_checked(index);
                            ptr.deref::<$name>().clone()
                        }
                };
                dst.push(value);
            }

            fn as_reflect_ref(value: bevy_ptr::Ptr<'_>) -> Option<&dyn bevy_reflect::Reflect>  {
                let value = unsafe { value.deref::<$name>() };
                (Some(value as &dyn bevy_reflect::Reflect))
            }
            fn as_reflect_mut(value: bevy_ptr::PtrMut<'_>) -> Option<&mut dyn bevy_reflect::Reflect>  {
                let value = unsafe { value.deref_mut::<$name>() };
                (Some(value as &mut dyn bevy_reflect::Reflect))
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

            fn into_lua<'a>(value: bevy_ptr::Ptr<'_>, lua: &'a mlua::Lua) -> mlua::Result<mlua::Value<'a>> {
                let value = unsafe { value.deref::<$name>() };
                <$name as mlua::IntoLua>::into_lua(value.clone(), lua)
            }

            fn from_lua<'lua>(mlua_value: mlua::Value<'lua>, mut storage: std::cell::RefMut<$crate::table::Storage>, table_row: Option<usize>, lua: &'lua mlua::Lua) -> mlua::Result<()> {
                match <$name as mlua::FromLua>::from_lua(mlua_value,lua) {
                    Ok(new_value) => {
                        if let Some(table_row) = table_row {
                            storage.replace_unchecked(table_row, new_value);
                        } else {
                            storage.push(new_value);
                        }
                        Ok(())
                    },
                    Err(error) => Err(error)
                }
            }
        }
    };
    (
        $( #[$meta:meta] )*
        $vis:vis struct $name:ident (
            $(
                $( #[$field_meta:meta] )*
                $field_vis:vis $field_ty:ty
            ),*
        $(,)? );
    ) => {
        #[macro_rules_attribute::apply($crate::FromIntoLua)]
        #[derive(educe::Educe)]
        #[educe(Debug)]
        #[derive(Clone, bevy_reflect::Reflect, serde::Serialize, serde::Deserialize)]
        $( #[$meta] )*
        $vis struct $name (
            $(
                $( #[$field_meta] )*
                $field_vis $field_ty
            ),*
        );

        impl $crate::components::component::AbstractComponent for $name {
            fn to_debug_string(value: bevy_ptr::Ptr<'_>) -> String {
                format!("{:#?}",unsafe {value.deref::<$name>()})
            }

            fn clone_into(
                index: usize,
                src: Option<std::cell::RefMut<$crate::table::Storage>>,
                mut dst: std::cell::RefMut<$crate::table::Storage>,
            ) {
                let value = unsafe {
                        if let Some(src) = src {
                            let ptr = src.0.get_checked(index);
                            ptr.deref::<$name>().clone()
                        }
                        else {
                            let ptr = dst.0.get_checked(index);
                            ptr.deref::<$name>().clone()
                        }
                };
                dst.push(value);
            }

            fn as_reflect_ref(value: bevy_ptr::Ptr<'_>) -> Option<&dyn bevy_reflect::Reflect>  {
                let value = unsafe { value.deref::<$name>() };
                (Some(value as &dyn bevy_reflect::Reflect))
            }
            fn as_reflect_mut(value: bevy_ptr::PtrMut<'_>) -> Option<&mut dyn bevy_reflect::Reflect>  {
                let value = unsafe { value.deref_mut::<$name>() };
                (Some(value as &mut dyn bevy_reflect::Reflect))
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

            fn into_lua<'a>(value: bevy_ptr::Ptr<'_>, lua: &'a mlua::Lua) -> mlua::Result<mlua::Value<'a>> {
                let value = unsafe { value.deref::<$name>() };
                <$name as mlua::IntoLua>::into_lua(value.clone(), lua)
            }

            fn from_lua<'lua>(mlua_value: mlua::Value<'lua>, mut storage: std::cell::RefMut<$crate::table::Storage>, table_row: Option<usize>, lua: &'lua mlua::Lua) -> mlua::Result<()> {
                match <$name as mlua::FromLua>::from_lua(mlua_value,lua) {
                    Ok(new_value) => {
                        if let Some(table_row) = table_row {
                            storage.replace_unchecked(table_row, new_value);
                        } else {
                            storage.push(new_value);
                        }
                        Ok(())
                    },
                    Err(error) => Err(error)
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
        #[macro_rules_attribute::apply($crate::FromIntoLua)]
        #[derive(educe::Educe)]
        #[educe(Debug)]
        #[derive(Clone, bevy_reflect::Reflect, serde::Serialize, serde::Deserialize)]
        $( #[$meta] )*
        $vis struct $name {
            $(
                $( #[$field_meta] )*
                $field_vis $field_name : $field_ty
            ),*
        }

        impl $crate::components::component::AbstractComponent for $name {
            fn to_debug_string(value: bevy_ptr::Ptr<'_>) -> String {
                format!("{:#?}",unsafe {value.deref::<$name>()})
            }

            fn clone_into(
                index: usize,
                src: Option<std::cell::RefMut<$crate::table::Storage>>,
                mut dst: std::cell::RefMut<$crate::table::Storage>,
            ) {
                let value = unsafe {
                        if let Some(src) = src {
                            let ptr = src.0.get_checked(index);
                            ptr.deref::<$name>().clone()
                        }
                        else {
                            let ptr = dst.0.get_checked(index);
                            ptr.deref::<$name>().clone()
                        }
                };
                dst.push(value);
            }

            fn as_reflect_ref(value: bevy_ptr::Ptr<'_>) -> Option<&dyn bevy_reflect::Reflect>  {
                let value = unsafe { value.deref::<$name>() };
                (Some(value as &dyn bevy_reflect::Reflect))
            }
            fn as_reflect_mut(value: bevy_ptr::PtrMut<'_>) -> Option<&mut dyn bevy_reflect::Reflect>  {
                let value = unsafe { value.deref_mut::<$name>() };
                (Some(value as &mut dyn bevy_reflect::Reflect))
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

            fn into_lua<'a>(value: bevy_ptr::Ptr<'_>, lua: &'a mlua::Lua) -> mlua::Result<mlua::Value<'a>> {
                let value = unsafe { value.deref::<$name>() };
                <$name as mlua::IntoLua>::into_lua(value.clone(), lua)
            }

            fn from_lua<'lua>(mlua_value: mlua::Value<'lua>, mut storage: std::cell::RefMut<$crate::table::Storage>, table_row: Option<usize>, lua: &'lua mlua::Lua) -> mlua::Result<()> {
                match <$name as mlua::FromLua>::from_lua(mlua_value,lua) {
                    Ok(new_value) => {
                        if let Some(table_row) = table_row {
                            storage.replace_unchecked(table_row, new_value);
                        } else {
                            storage.push(new_value);
                        }
                        Ok(())
                    },
                    Err(error) => Err(error)
                }
            }
        }
    }
}

#[macro_export]
macro_rules! impl_system_state {
    ($t:ty) => {
        impl $crate::systems::SystemState for $t {
            fn id(&self) -> $crate::systems::EnumId {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::hash::DefaultHasher::new();
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
macro_rules! EnumTag {
    ($(#[$meta:meta])? $vis:vis enum $name:ident {
        $($(#[$vmeta:meta])? $vname:ident $(,)?)*
    }) => {
        $(#[$meta])?
        #[macro_rules_attribute::apply($crate::FromIntoLua)]
        #[derive(educe::Educe)]
        #[educe(Debug)]
        #[derive(Clone, Copy, bevy_reflect::Reflect, serde::Serialize, serde::Deserialize)]
        $vis enum $name {
            $($(#[$vmeta])? $vname,)*
        }

        impl $crate::components::component::AbstractComponent for $name {
            fn to_debug_string(value: bevy_ptr::Ptr<'_>) -> String {
                format!("{:#?}",unsafe {value.deref::<$name>()})
            }

            fn clone_into(
                index: usize,
                src: Option<std::cell::RefMut<$crate::table::Storage>>,
                mut dst: std::cell::RefMut<$crate::table::Storage>,
            ) {
                let value = unsafe {
                        if let Some(src) = src {
                            let ptr = src.0.get_checked(index);
                            ptr.deref::<$name>().clone()
                        }
                        else {
                            let ptr = dst.0.get_checked(index);
                            ptr.deref::<$name>().clone()
                        }
                };
                dst.push(value);
            }
            fn as_reflect_ref(value: bevy_ptr::Ptr<'_>) -> Option<&dyn bevy_reflect::Reflect>  {
                let value = unsafe { value.deref::<$name>() };
                (Some(value as &dyn bevy_reflect::Reflect))
            }
            fn as_reflect_mut(value: bevy_ptr::PtrMut<'_>) -> Option<&mut dyn bevy_reflect::Reflect>  {
                let value = unsafe { value.deref_mut::<$name>() };
                (Some(value as &mut dyn bevy_reflect::Reflect))
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

            fn into_lua<'a>(value: bevy_ptr::Ptr<'_>, lua: &'a mlua::Lua) -> mlua::Result<mlua::Value<'a>> {
                let value = unsafe { value.deref::<$name>() };
                <$name as mlua::IntoLua>::into_lua(value.clone(), lua)
            }

            fn from_lua<'lua>(mlua_value: mlua::Value<'lua>, mut storage: std::cell::RefMut<$crate::table::Storage>, table_row: Option<usize>, lua: &'lua mlua::Lua) -> mlua::Result<()> {
                match <$name as mlua::FromLua>::from_lua(mlua_value,lua) {
                    Ok(new_value) => {
                        if let Some(table_row) = table_row {
                            storage.replace_unchecked(table_row, new_value);
                        } else {
                            storage.push(new_value);
                        }
                        Ok(())
                    },
                    Err(error) => Err(error)
                }
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

#[apply(Component)]
enum TestEnum {
    TupleVariant,
    #[reflect(ignore)]
    TupleVariant2(i32, u32),
    NamedVariant {
        name: i32,
    },
}

pub trait AbstractComponent: 'static + Sized {
    fn to_debug_string(value: bevy_ptr::Ptr<'_>) -> String;
    fn clone_into(index: usize, src: Option<RefMut<Storage>>, dst: RefMut<Storage>);
    fn as_reflect_ref(value: bevy_ptr::Ptr<'_>) -> Option<&dyn Reflect>;
    fn as_reflect_mut(value: bevy_ptr::PtrMut<'_>) -> Option<&mut dyn Reflect>;
    fn serialize(value: bevy_ptr::Ptr<'_>) -> Result<serde_json::Value, serde_json::error::Error>;
    fn deserialize(value: serde_json::Value, storage: RefMut<Storage>) -> serde_json::Result<()>;
    fn into_lua<'lua>(
        value: bevy_ptr::Ptr<'_>,
        lua: &'lua mlua::Lua,
    ) -> mlua::Result<mlua::Value<'lua>>;
    fn from_lua<'lua>(
        mlua_value: mlua::Value<'lua>,
        storage: RefMut<Storage>,
        table_row: Option<usize>,
        lua: &'lua mlua::Lua,
    ) -> mlua::Result<()>;
}
pub trait EnumTag: AbstractComponent + 'static {
    fn id(&self) -> EnumId;
    fn from_id(id: EnumId) -> Option<Self>;
}
