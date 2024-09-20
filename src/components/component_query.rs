use crate::{entity::Entity, impl_tuple_helper};

use super::component::AbstractComponent;

macro_rules! impl_component_query {
    ($($params:ident),+) => {
        impl_tuple_helper!(impl_component_query, $($params),+);
        impl <$($params: ComponentQuery),+> ComponentQuery for ($($params),+,)  {
            #[allow(unused_parens)]
            type Item<'i> = ($($params::Item<'i>),+);

            fn fetch(entity: &Entity) -> Self::Item<'_> {
                ($(
                    $params::fetch(entity)
                ),+)
            }
        }
    };
}
impl_tuple_helper!(impl_component_query, T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

pub trait ComponentQuery {
    type Item<'i>;
    fn fetch(entity: &Entity) -> Self::Item<'_>;
}

impl<T: AbstractComponent> ComponentQuery for &T {
    type Item<'i> = &'i T;

    fn fetch(entity: &Entity) -> Self::Item<'_> {
        entity.get_comp::<T>().get(|c| unsafe { &*(c as *const T) })
    }
}

impl<T: AbstractComponent> ComponentQuery for &mut T {
    type Item<'i> = &'i T;

    fn fetch(entity: &Entity) -> Self::Item<'_> {
        entity
            .get_comp::<T>()
            .get_mut(|c| unsafe { &mut *(c as *mut T) })
    }
}

impl<T: AbstractComponent> ComponentQuery for Option<&T> {
    type Item<'i> = Option<&'i T>;

    fn fetch(entity: &Entity) -> Self::Item<'_> {
        Some(
            entity
                .try_get_comp::<T>()?
                .get(|c| unsafe { &*(c as *const T) }),
        )
    }
}

impl<T: AbstractComponent> ComponentQuery for Option<&mut T> {
    type Item<'i> = Option<&'i mut T>;

    fn fetch(entity: &Entity) -> Self::Item<'_> {
        Some(
            entity
                .try_get_comp::<T>()?
                .get_mut(|c| unsafe { &mut *(c as *mut T) }),
        )
    }
}
