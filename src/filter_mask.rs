use crate::{
    archetypes::Archetypes, identifier::Identifier, systems::EnumId, wrappers::ArchetypeCell
};

#[derive(Hash, Debug, Clone)]
pub struct FilterMask {
    pub has: Vec<Identifier>,
    pub not: Vec<Identifier>,
    pub any_has: Vec<Identifier>,
    pub any_not: Vec<Identifier>,
    pub states: Vec<(Identifier, EnumId)>,
}

impl FilterMask {
    pub fn new() -> Self {
        Self {
            has: vec![],
            not: vec![],
            any_has: vec![],
            any_not: vec![],
            states: vec![],
        }
    }

    pub fn sort(&mut self) {
        self.has.sort();
        self.not.sort();
        self.any_has.sort();
        self.any_not.sort();
        self.states.sort();
    }

    pub fn push_states(&mut self, state: (Identifier, EnumId)) {
        self.states.push(state);
    }

    pub fn push_not(&mut self, id: Identifier) {
        self.not.push(id);
    }

    pub fn push_any_has(&mut self, id: Identifier) {
        self.any_has.push(id);
    }

    pub fn push_any_not(&mut self, id: Identifier) {
        self.any_not.push(id);
    }

    pub fn push_has(&mut self, id: Identifier) {
        self.has.push(id);
    }

    pub fn join(&mut self, mask: &FilterMask) {
        for id in mask.any_not.iter() {
            self.push_any_not(*id)
        }
        for id in mask.any_has.iter() {
            self.push_any_has(*id)
        }
        for id in mask.not.iter() {
            self.push_not(*id)
        }
        for id in mask.has.iter() {
            self.push_has(*id)
        }
        for id in mask.states.iter() {
            self.push_states(*id)
        }
    }

    pub(crate) fn matches_archetype(
        &self,
        archetypes: &Archetypes,
        archetype: &ArchetypeCell,
    ) -> bool {
        if self.has.iter().any(|id| {
            !archetypes
                .get_archetypes_with_id(*id)
                .map(|a| a.contains(archetype))
                .unwrap_or(false)
        }) {
            return false;
        }
        if self.not.iter().any(|id| {
            archetypes
                .get_archetypes_with_id(*id)
                .map(|a| a.contains(archetype))
                .unwrap_or(false)
        }) {
            return false;
        }
        if !self.any_has.is_empty()
            && self.any_has.iter().all(|id| {
                !archetypes
                    .get_archetypes_with_id(*id)
                    .map(|a| a.contains(archetype))
                    .unwrap_or(false)
            })
        {
            return false;
        }
        if !self.any_not.is_empty()
            && self.any_not.iter().all(|id| {
                archetypes
                    .get_archetypes_with_id(*id)
                    .map(|a| a.contains(archetype))
                    .unwrap_or(false)
            })
        {
            return false;
        }

        true
    }
}

impl Default for FilterMask {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::hash::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;

    use super::*;

    #[test]
    fn hash() {
        let mut mask1 = FilterMask::new();
        mask1.has.push(1.into());
        mask1.has.push(2.into());
        mask1.has.push(3.into());

        let mut mask2 = FilterMask::new();
        mask2.has.push(3.into());
        mask2.has.push(2.into());
        mask2.has.push(1.into());
        
        mask1.sort();
        mask2.sort();

        let mut hasher1 = DefaultHasher::new();
        mask1.hash(&mut hasher1);
        let hash1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        mask2.hash(&mut hasher2);
        let hash2 = hasher2.finish();

        assert_eq!(hash1, hash2);
    }
}
