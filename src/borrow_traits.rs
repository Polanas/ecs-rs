use std::{cell::{Ref, RefCell, RefMut}, rc::Rc};

pub trait BorrowFn<T> {
    fn borrow_fn<F, U>(&self, f: F) -> U
    where
        F: FnOnce(Ref<T>) -> U;
}

pub trait BorrowMutFn<T> {
    fn borrow_mut_fn<F, U>(&self, f: F) -> U
    where
        F: FnOnce(RefMut<T>) -> U;
}

impl<T> BorrowFn<T> for Rc<RefCell<T>> {
    fn borrow_fn<F, U>(&self, f: F) -> U
    where
        F: FnOnce(Ref<T>) -> U {
            f(self.borrow())
    }
}

impl<T> BorrowMutFn<T> for Rc<RefCell<T>> {
    fn borrow_mut_fn<F, U>(&self, f: F) -> U
    where
        F: FnOnce(RefMut<T>) -> U {
            f(self.borrow_mut())
    }
}

impl<T> BorrowFn<T> for RefCell<T> {
    fn borrow_fn<F, U>(&self, f: F) -> U
    where
        F: FnOnce(Ref<T>) -> U {
            f(self.borrow())
    }
}

impl<T> BorrowMutFn<T> for RefCell<T> {
    fn borrow_mut_fn<F, U>(&self, f: F) -> U
    where
        F: FnOnce(RefMut<T>) -> U {
            f(self.borrow_mut())
    }
}
