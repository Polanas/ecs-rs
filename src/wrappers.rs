use std::{cell::RefCell, hash::Hash, ops::Deref, rc::Rc};

use crate::{archetype::Archetype, borrow_traits::BorrowFn, table::Table};

#[derive(Clone)]
pub struct ArchetypeCell(pub Rc<RefCell<Archetype>>);

impl ArchetypeCell {
    pub fn len(&self) -> usize {
        self.0.borrow_fn(|a| a.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl From<Archetype> for ArchetypeCell {
    fn from(value: Archetype) -> Self {
        Self(RefCell::new(value).into())
    }
}

impl From<Rc<RefCell<Archetype>>> for ArchetypeCell {
    fn from(value: Rc<RefCell<Archetype>>) -> Self {
        Self(value)
    }
}

impl Deref for ArchetypeCell {
    type Target = Rc<RefCell<Archetype>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Hash for ArchetypeCell {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.borrow().hash(state)
    }
}

impl PartialEq for ArchetypeCell {
    fn eq(&self, other: &Self) -> bool {
        self.0.borrow().id() == other.0.borrow().id()
    }
}
impl Eq for ArchetypeCell {}

#[derive(Clone)]
pub struct TableCell(pub Rc<RefCell<Table>>);

impl From<Table> for TableCell {
    fn from(value: Table) -> Self {
        Self(RefCell::new(value).into())
    }
}

impl From<Rc<RefCell<Table>>> for TableCell {
    fn from(value: Rc<RefCell<Table>>) -> Self {
        Self(value)
    }
}

impl Deref for TableCell {
    type Target = Rc<RefCell<Table>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Hash for TableCell {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.borrow().hash(state)
    }
}

impl PartialEq for TableCell {
    fn eq(&self, other: &Self) -> bool {
        self.0.borrow().id() == other.0.borrow().id()
    }
}
impl Eq for TableCell {}
