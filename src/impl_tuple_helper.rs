#[macro_export]
//TODO: make the macro also call $macro with all the params
macro_rules! impl_tuple_helper {
    ($macro:ident, $last:ident) => {};
    ($macro:ident, $param:ident, $($rest:ident),+) => {
        $macro!($($rest),+);
    };
}
