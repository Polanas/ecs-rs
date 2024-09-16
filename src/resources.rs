use std::{any::TypeId, cell::RefCell, rc::Rc};

use crate::archetypes::Resources;

macro_rules! impl_resource_query {
    ($($params:ident),+) => {
        impl <$($params: ResourceQuery),+> ResourceQuery for ($($params),+,)  {
            #[allow(unused_parens)]
            type Item<'i> = ($($params::Item<'i>),+);

            fn fetch(
                resources: &Rc<RefCell<Resources>>
            ) -> Self::Item<'_> {
                ($(
                    $params::fetch(resources)
                ),+)
            }
        }
    };
}
impl_resource_query!(T0);
impl_resource_query!(T0, T1);
impl_resource_query!(T0, T1, T3);
impl_resource_query!(T0, T1, T3, T4);
impl_resource_query!(T0, T1, T3, T4, T5);
impl_resource_query!(T0, T1, T3, T4, T5, T6);
impl_resource_query!(T0, T1, T3, T4, T5, T6, T7);
impl_resource_query!(T0, T1, T3, T4, T5, T6, T7, T8);
impl_resource_query!(T0, T1, T3, T4, T5, T6, T7, T8, T9);
impl_resource_query!(T0, T1, T3, T4, T5, T6, T7, T8, T9, T10);
impl_resource_query!(T0, T1, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_resource_query!(T0, T1, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);

pub trait Resource: 'static {}
impl<T: 'static> Resource for T {}

pub trait ResourceQuery {
    type Item<'i>;
    fn fetch(resources: &Rc<RefCell<Resources>>) -> Self::Item<'_>;
}

impl<T: Resource> ResourceQuery for Option<&T> {
    type Item<'i> = Option<&'i T>;

    fn fetch(resources: &Rc<RefCell<Resources>>) -> Self::Item<'_> {
        let resources = resources.borrow();
        let resource = resources.get(&TypeId::of::<T>())?.borrow();
        let resource = resource.downcast_ref::<T>().unwrap();
        //TODO: delay resource deletions while a query is active
        unsafe { Some(&*(resource as *const T)) }
    }
}

impl<T: Resource> ResourceQuery for Option<&mut T> {
    type Item<'i> = Option<&'i mut T>;

    fn fetch(resources: &Rc<RefCell<Resources>>) -> Self::Item<'_> {
        let mut resources = resources.borrow_mut();
        let mut resource = resources.get_mut(&TypeId::of::<T>())?.borrow_mut();
        let resource = resource.downcast_mut::<T>().unwrap();
        //TODO: delay resource deletions while a query is active
        unsafe { Some(&mut *(resource as *mut T)) }
    }
}
impl<T: Resource> ResourceQuery for &T {
    type Item<'i> = &'i T;

    fn fetch(resources: &Rc<RefCell<Resources>>) -> Self::Item<'_> {
        let resources = resources.borrow();
        let resource = resources
            .get(&TypeId::of::<T>())
            .unwrap_or_else(|| panic!("failed to get resource {0}", tynm::type_name::<T>()))
            .borrow();
        let resource = resource.downcast_ref::<T>().unwrap();
        //TODO: delay resource deletions while a query is active
        unsafe { &*(resource as *const T) }
    }
}

impl<T: Resource> ResourceQuery for &mut T {
    type Item<'i> = &'i mut T;

    fn fetch(resources: &Rc<RefCell<Resources>>) -> Self::Item<'_> {
        let mut resources = resources.borrow_mut();
        let mut resource = resources
            .get_mut(&TypeId::of::<T>())
            .unwrap_or_else(|| panic!("failed to get resource {0}", tynm::type_name::<T>()))
            .borrow_mut();
        let resource = resource.downcast_mut::<T>().unwrap();
        //TODO: delay resource deletions while a query is active
        unsafe { &mut *(resource as *mut T) }
    }
}
