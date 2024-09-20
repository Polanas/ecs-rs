macro_rules! impl_tuple_helper {
    ($macro:ident, $last:ident) => {};
    ($macro:ident, $param:ident, $($rest:ident),+) => {
        $macro!($($rest),+);
    };
}
