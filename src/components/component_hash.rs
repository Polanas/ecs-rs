use std::collections::BTreeSet;

use crate::{archetypes::Archetypes, identifier::Identifier};

pub trait ComponentsHash {
    fn archetype_hash(&self) -> u64;
    fn table_hash(&self, archetypes: &Archetypes) -> u64;
}

impl ComponentsHash for BTreeSet<Identifier> {
    fn archetype_hash(&self) -> u64 {
        let mut hash = self.len() as u64;
        for id in self.iter() {
            hash = hash.wrapping_mul(314159);
            hash = hash.wrapping_add(u64::from_ne_bytes(id.0));
        }
        hash
    }

    fn table_hash(&self, archetypes: &Archetypes) -> u64 {
        let type_registry = archetypes.type_registry();
        let mut comps_with_data = self
            .iter()
            .filter(|c| type_registry.layouts.contains_key(&c.stripped()));
        let len = comps_with_data.by_ref().count();
        let mut hash = len as u64;
        for id in comps_with_data {
            //discard the components if it doesn't contain any data, meaning it's a tag
            if !archetypes
                .type_registry()
                .layouts
                .contains_key(&id.stripped())
            {
                continue;
            }
            hash = hash.wrapping_mul(314159);
            hash = hash.wrapping_add(u64::from_ne_bytes(id.0));
        }
        hash
    }
}
