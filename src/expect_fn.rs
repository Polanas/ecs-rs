pub trait ExpectFnOption<T> {
    fn expect_fn(self, f: impl FnOnce() -> String) -> T;
}

impl<T> ExpectFnOption<T> for Option<T> {
    fn expect_fn(self, f: impl FnOnce() -> String) -> T {
        self.unwrap_or_else(|| panic!("{}", f()))
    }
}

pub trait ExpectFnResult<T, E> {
    fn expect_fn(self, f: impl FnOnce(E) -> String) -> T;
}

impl<T, E> ExpectFnResult<T, E> for Result<T, E> {
    fn expect_fn(self, f: impl FnOnce(E) -> String) -> T {
        self.unwrap_or_else(|err| panic!("{}", f(err)))
    }
}
