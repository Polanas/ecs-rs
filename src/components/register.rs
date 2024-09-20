use crate::world::archetypes_mut;

use super::component::AbstractComponent;

macro_rules! impl_register_query {
    (
        $($t:ident),+
    ) => {
            __impl_register_query_helper!($($t),+);
            impl<$($t: RegisterComponentQuery),+> RegisterComponentQuery for ($($t),+,) {
                fn register() {
                    $(
                        $t::register();
                    )+
                }
            }
    };
}

macro_rules! __impl_register_query_helper {
    ($t:ident) => {};
    ($t:ident, $($rest:ident),+) => {
        impl_register_query!($($rest),+);
    }
}
impl_register_query!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T14, T15, T16);

pub trait RegisterComponentQuery {
    fn register();
}

impl<T: AbstractComponent> RegisterComponentQuery for T {
    fn register() {
        archetypes_mut(|a| {
            a.register_component::<T>();
        })
    }
}
