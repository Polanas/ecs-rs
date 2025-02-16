pub trait ExpectFnOption<T> {
    fn expect_fn(self, f: impl FnOnce() -> String) -> T;
}

impl<T> ExpectFnOption<T> for Option<T> {
    fn expect_fn(self, f: impl FnOnce() -> String) -> T {
        self.unwrap_or_else(|| panic!("{}", f()))
    }
}

pub trait ExpectFnResult<T, E>
where
    E: std::fmt::Display,
{
    fn expect_fn(self, f: impl FnOnce(E) -> String) -> T;
    fn expect_pretty(self, msg: &'static str) -> T;
}

impl<T, E> ExpectFnResult<T, E> for Result<T, E>
where
    E: std::fmt::Display,
{
    fn expect_fn(self, f: impl FnOnce(E) -> String) -> T {
        self.unwrap_or_else(|err| panic!("{}: ", f(err)))
    }

    fn expect_pretty(self, msg: &'static str) -> T {
        self.unwrap_or_else(|err| panic!("{0}: {1}", msg, err))
    }
}
