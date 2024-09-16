use std::{any::TypeId, marker::PhantomData};

use crate::{
    archetypes::{self, Wildcard},
    components::component::Component,
    filter_mask::FilterMask,
    identifier::Identifier,
    query::{FilterMaskHint, QueryFilterData},
    world::archetypes_mut,
};

pub trait RelationArgument: Component {
    fn id() -> Identifier;
}

impl<T: Component> RelationArgument for T {
    fn id() -> Identifier {
        if TypeId::of::<T>() == TypeId::of::<Wildcard>() {
            archetypes::WILDCARD_RELATIONSHIP
        } else {
            archetypes_mut(|a| a.component_id::<T>())
        }
    }
}

pub struct WithoutRelation<R: RelationArgument, T: RelationArgument> {
    data: PhantomData<(R, T)>,
}

impl<R: RelationArgument, T: RelationArgument> QueryFilterData for WithoutRelation<R, T> {
    fn mask(mask: &mut FilterMask, hint: FilterMaskHint) {
        let relationship = archetypes_mut(|a| a.relationship_id_typed::<R, T>());
        match hint {
            FilterMaskHint::Regular => mask.push_not(relationship),
            FilterMaskHint::Not => mask.push_has(relationship),
        }
    }
}

pub struct WithRelation<R: RelationArgument, T: RelationArgument> {
    data: PhantomData<(R, T)>,
}

impl<R: RelationArgument, T: RelationArgument> QueryFilterData for WithRelation<R, T> {
    fn mask(mask: &mut FilterMask, hint: FilterMaskHint) {
        let relationship = archetypes_mut(|a| a.relationship_id_typed::<R, T>());
        match hint {
            FilterMaskHint::Regular => mask.push_has(relationship),
            FilterMaskHint::Not => mask.push_not(relationship),
        }
    }
}

pub struct Without<T: Component> {
    data: PhantomData<T>,
}

impl<T: Component> QueryFilterData for Without<T> {
    fn mask(mask: &mut FilterMask, hint: FilterMaskHint) {
        archetypes_mut(|a| {
            match hint {
                FilterMaskHint::Regular => mask.push_not(a.component_id::<T>()),
                FilterMaskHint::Not => mask.push_has(a.component_id::<T>()),
            };
        });
    }
}
pub struct Not<T: QueryFilterData> {
    data: PhantomData<T>,
}

impl<T: QueryFilterData> QueryFilterData for Not<T> {
    fn mask(mask: &mut FilterMask, _: FilterMaskHint) {
        T::mask(mask, FilterMaskHint::Not);
    }
}

pub struct With<T: Component> {
    data: PhantomData<T>,
}

impl<T: Component> QueryFilterData for With<T> {
    fn mask(mask: &mut FilterMask, hint: FilterMaskHint) {
        archetypes_mut(|a| {
            match hint {
                FilterMaskHint::Regular => mask.push_has(a.component_id::<T>()),
                FilterMaskHint::Not => mask.push_not(a.component_id::<T>()),
            };
        });
    }
}
