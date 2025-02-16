use crate::{impl_tuple_helper, world::World};

macro_rules! impl_plugins {
    ($($params:ident),+) => {
        impl_tuple_helper!(impl_plugins, $($params),+);
        impl <$($params: Plugin),+> Plugin for ($($params),+,) {
            fn build(&self, world: &World) {
                #[allow(non_snake_case)]
                let ($($params),+,) = self;
                $(
                    $params.build(world);
                )+
            }
        }
    };
}
impl_tuple_helper!(
    impl_plugins,
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
pub trait Plugin {
    fn build(&self, world: &World);
}
