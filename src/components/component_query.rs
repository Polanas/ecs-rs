use crate::{archetypes::GetComponentError, entity::Entity, impl_tuple_helper};

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
impl_tuple_helper!(
    impl_component_query,
    T0,
    T1,
    T2,
    T3,
    T4,
    T5,
    T6,
    T7,
    T8,
    T9,
    T10,
    T11,
    T12,
    T13,
    T14,
    T15,
    T16
);

pub trait ComponentQuery {
    type Item<'i>;
    fn fetch(entity: &Entity) -> Self::Item<'_>;
}

impl<T: AbstractComponent> ComponentQuery for &T {
    type Item<'i> = &'i T;

    fn fetch(entity: &Entity) -> Self::Item<'_> {
        entity.comp_ret(|c| unsafe { &*(c as *const T) })
    }
}

impl<T: AbstractComponent> ComponentQuery for &mut T {
    type Item<'i> = &'i T;

    fn fetch(entity: &Entity) -> Self::Item<'_> {
        entity.comp_mut_ret(|c| unsafe { &mut *(c as *mut T) })
    }
}
impl<T: AbstractComponent> ComponentQuery for Result<&T, GetComponentError> {
    type Item<'i> = Result<&'i T, GetComponentError>;

    fn fetch(entity: &Entity) -> Self::Item<'_> {
        entity.get_comp_ret(|c| c.map(|c| unsafe { &*(c as *const T) }))
    }
}

impl<T: AbstractComponent> ComponentQuery for Result<&mut T, GetComponentError> {
    type Item<'i> = Result<&'i mut T, GetComponentError>;

    fn fetch(entity: &Entity) -> Self::Item<'_> {
        entity.get_comp_mut_ret(|c| c.map(|c| unsafe { &mut *(c as *mut T) }))
    }
}

impl<T: AbstractComponent> ComponentQuery for Option<&T> {
    type Item<'i> = Option<&'i T>;

    fn fetch(entity: &Entity) -> Self::Item<'_> {
        entity.get_comp_ret(|c| c.map(|c| unsafe { &*(c as *const T) }).ok())
    }
}

impl<T: AbstractComponent> ComponentQuery for Option<&mut T> {
    type Item<'i> = Option<&'i mut T>;

    fn fetch(entity: &Entity) -> Self::Item<'_> {
        entity.get_comp_mut_ret(|c| c.map(|c| unsafe { &mut *(c as *mut T) }).ok())
    }
}
