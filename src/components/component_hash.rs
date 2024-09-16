use std::collections::BTreeSet;

use crate::{archetypes::Archetypes, identifier::Identifier};

pub trait ComponentsHash {
    fn regular_hash(&self) -> u64;
    fn table_hash(&self, archetypes: &Archetypes) -> u64;
}

impl ComponentsHash for BTreeSet<Identifier> {
    fn regular_hash(&self) -> u64 {
        let mut hash = self.len() as u64;
        for id in self.iter() {
            hash = hash.wrapping_mul(314159);
            hash = hash.wrapping_add(u64::from_be_bytes(id.0));
        }
        hash
    }

    fn table_hash(&self, archetypes: &Archetypes) -> u64 {
        //TODO: finish this
        let mut hash = self.len() as u64;
        for id in self.iter() {
            //we want tables with different components set, but same actual data storages have
            //the save hash
            if !archetypes.type_registry().layouts.contains_key(id) {
                continue;
            }
            hash = hash.wrapping_mul(314159);
            hash = hash.wrapping_add(u64::from_be_bytes(id.0));
        }
        hash
    }
}
