use std::{cell::RefCell, rc::Rc};

use crate::{archetypes::TableReusage, entity::Entity, world::archetypes_mut};

use super::component::AbstractComponent;

#[macro_export]
macro_rules! ComponentBundle {
    (
        $( #[$meta:meta] )*
    //  ^~~~attributes~~~~^
        $vis:vis struct $name:ident {
            $(
                $( #[$field_meta:meta] )*
    //          ^~~~field attributes~~~!^
                $field_vis:vis $field_name:ident : $field_ty:ty
    //          ^~~~~~~~~~~~~~~~~a single field~~~~~~~~~~~~~~~^
            ),+
        $(,)? }
    ) => {
        $( #[$meta] )*
        $vis struct $name {
            $(
                $( #[$field_meta] )*
                $field_vis $field_name : $field_ty
            ),+
        }

        impl $crate::components::component_bundle::ComponentBundle for $name {
            fn add(self, entity: &$crate::entity::Entity) {
                $(
                    $crate::components::component_bundle::ComponentBundle::add(self.$field_name, entity);
                )+
            }
            fn remove(entity: &$crate::entity::Entity) {
                $(
                    <$field_ty>::remove(entity);
                )+
            }
        }
    }
}

pub trait ComponentBundle {
    fn add(self, entity: &Entity);
    fn remove(entity: &Entity);
}

impl<T: AbstractComponent> ComponentBundle for Option<T> {
    fn add(self, entity: &Entity) {
        let Some(component) = self else {
            return;
        };
        let (id, callbacks) = archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes
                .add_component_typed(id, entity.into(), component)
                .unwrap();
            (id, archetypes.callbacks().clone())
        });
        archetypes_mut(|a| a.lock());
        callbacks.borrow().run_add_callback(id, entity.into());
        archetypes_mut(|a| a.unlock());
    }

    fn remove(entity: &Entity) {
        let (id, callbacks) = archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes
                .remove_component(id, entity.into(), TableReusage::New)
                .unwrap();
            (id, archetypes.callbacks().clone())
        });
        archetypes_mut(|a| a.lock());
        callbacks.borrow().run_add_callback(id, entity.into());
        archetypes_mut(|a| a.unlock());
    }
}
impl<T: AbstractComponent> ComponentBundle for T {
    fn add(self, entity: &Entity) {
        let (id, callbacks) = archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes
                .add_component_typed(id, entity.into(), self)
                .unwrap();
            (id, archetypes.callbacks().clone())
        });
        archetypes_mut(|a| a.lock());
        callbacks.borrow().run_add_callback(id, entity.into());
        archetypes_mut(|a| a.unlock());
    }

    fn remove(entity: &Entity) {
        let (id, callbacks) = archetypes_mut(|archetypes| {
            let id = archetypes.component_id::<T>();
            archetypes
                .remove_component(id, entity.into(), TableReusage::New)
                .unwrap();
            (id, archetypes.callbacks().clone())
        });
        archetypes_mut(|a| a.lock());
        callbacks.borrow().run_add_callback(id, entity.into());
        archetypes_mut(|a| a.unlock());
    }
}

macro_rules! impl_comp_bundle {
    ($(($t:ident, $f:tt)),+ $(,)?) => {
        impl<$($t: ComponentBundle),+> ComponentBundle for ($($t),+,) {
            fn add(self, entity: &Entity) {
                $(
                    self.$f.add(entity);
                )+
            }
            fn remove(entity: &Entity) {
                $(
                    $t::remove(entity);
                )+
            }
        }
    };
}

impl_comp_bundle!((T0, 0));
impl_comp_bundle!((T0, 0), (T1, 1));
impl_comp_bundle!((T0, 0), (T1, 1), (T2, 2));
impl_comp_bundle!((T0, 0), (T1, 1), (T2, 2), (T3, 3));
impl_comp_bundle!((T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4));
impl_comp_bundle!((T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5));
impl_comp_bundle!(
    (T0, 0),
    (T1, 1),
    (T2, 2),
    (T3, 3),
    (T4, 4),
    (T5, 5),
    (T6, 6),
);
impl_comp_bundle!(
    (T0, 0),
    (T1, 1),
    (T2, 2),
    (T3, 3),
    (T4, 4),
    (T5, 5),
    (T6, 6),
    (T7, 7),
);
impl_comp_bundle!(
    (T0, 0),
    (T1, 1),
    (T2, 2),
    (T3, 3),
    (T4, 4),
    (T5, 5),
    (T6, 6),
    (T7, 7),
    (T8, 8),
);
impl_comp_bundle!(
    (T0, 0),
    (T1, 1),
    (T2, 2),
    (T3, 3),
    (T4, 4),
    (T5, 5),
    (T6, 6),
    (T7, 7),
    (T8, 8),
    (T9, 9),
);
impl_comp_bundle!(
    (T0, 0),
    (T1, 1),
    (T2, 2),
    (T3, 3),
    (T4, 4),
    (T5, 5),
    (T6, 6),
    (T7, 7),
    (T8, 8),
    (T9, 9),
    (T10, 10),
);
impl_comp_bundle!(
    (T0, 0),
    (T1, 1),
    (T2, 2),
    (T3, 3),
    (T4, 4),
    (T5, 5),
    (T6, 6),
    (T7, 7),
    (T8, 8),
    (T9, 9),
    (T10, 10),
    (T11, 11),
);
impl_comp_bundle!(
    (T0, 0),
    (T1, 1),
    (T2, 2),
    (T3, 3),
    (T4, 4),
    (T5, 5),
    (T6, 6),
    (T7, 7),
    (T8, 8),
    (T9, 9),
    (T10, 10),
    (T11, 11),
    (T12, 12),
);
