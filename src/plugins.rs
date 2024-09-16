use crate::world::World;

macro_rules! impl_plugins {
    (
        $($t:ident),+
    ) => {
        impl< $($t: Plugin),+> Plugins for ( $($t),+, ) {
            fn add_plugins(self, world: &World) {
                #[allow(unused_parens, non_snake_case)]
                let ($($t),+,) = self;
                $(
                    $t.add_plugins(world);
                )+
            }
        }
    };
}

impl_plugins!(T0);
impl_plugins!(T0, T1);
impl_plugins!(T0, T1, T2);
impl_plugins!(T0, T1, T2, T3);
impl_plugins!(T0, T1, T2, T3, T4);
impl_plugins!(T0, T1, T2, T3, T4, T5);
impl_plugins!(T0, T1, T2, T3, T4, T5, T6);
impl_plugins!(T0, T1, T2, T3, T4, T5, T6, T7);
impl_plugins!(T0, T1, T2, T3, T4, T5, T6, T7, T8);
impl_plugins!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_plugins!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_plugins!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_plugins!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);

pub trait Plugin: 'static {
    fn build(&self, world: &World);
}

pub trait Plugins: 'static {
    fn add_plugins(self, world: &World);
}

impl<P: Plugin> Plugins for P {
    fn add_plugins(self, world: &World) {
        self.build(world);
    }
}
