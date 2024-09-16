use std::fmt::Debug;

use bevy_reflect::Reflect;
use packed_struct::{
    derive::PackedStruct,
    types::{bits::Bits, Integer},
    PackedStruct,
};

use crate::archetypes::{StrippedIdentifier, WILDCARD_25, WILDCARD_32};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord, Reflect)]
pub struct Identifier(pub [u8; 8]);

impl Debug for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = u64::from_be_bytes(self.0);
        f.debug_tuple("Identifier").field(&value).finish()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WildcardKind {
    Relation,
    Target,
    Both,
    None,
}

impl Identifier {
    pub(crate) fn stripped(&self) -> StrippedIdentifier {
        std::convert::Into::<StrippedIdentifier>::into(*self)
    }

    pub fn wildcard_kind(&self) -> WildcardKind {
        match (self.low32(), self.second()) {
            (WILDCARD_32, WILDCARD_25) => WildcardKind::Both,
            (WILDCARD_32, _) => WildcardKind::Relation,
            (_, WILDCARD_25) => WildcardKind::Target,
            (_, _) => WildcardKind::None,
        }
    }

    pub fn unpack(&self) -> IdentifierUnpacked {
        IdentifierUnpacked::unpack(&self.0).unwrap()
    }

    pub fn set_low32(&mut self, low32: u32) {
        let mut id = self.unpack();
        id.low32 = low32;
        *self = id.pack().unwrap().into();
    }

    pub fn set_second(&mut self, second: u32) {
        let mut id = self.unpack();
        id.high32.second = second.into();
        *self = id.pack().unwrap().into();
    }

    pub fn set_is_relation(&mut self, is_relation: bool) {
        let mut id = self.unpack();
        id.high32.is_relation = is_relation;
        *self = id.pack().unwrap().into();
    }

    pub fn set_is_target(&mut self, is_target: bool) {
        let mut id = self.unpack();
        id.high32.is_target = is_target;
        *self = id.pack().unwrap().into();
    }

    pub fn set_has_relationships(&mut self, has_relationships: bool) {
        let mut id = self.unpack();
        id.high32.is_target_exclusive = has_relationships;
        *self = id.pack().unwrap().into();
    }

    pub fn set_is_tag(&mut self, is_tag: bool) {
        let mut id = self.unpack();
        id.high32.is_watched = is_tag;
        *self = id.pack().unwrap().into();
    }

    pub fn set_is_active(&mut self, is_active: bool) {
        let mut id = self.unpack();
        id.high32.is_active = is_active;
        *self = id.pack().unwrap().into();
    }

    pub fn low32(&self) -> u32 {
        self.unpack().low32
    }

    pub fn second(&self) -> u32 {
        self.unpack().high32.second.into()
    }
    pub fn has_relatinships(&self) -> bool {
        self.unpack().high32.is_target_exclusive
    }
    pub fn is_active(&self) -> bool {
        self.unpack().high32.is_active
    }
    pub fn is_exclusive(&self) -> bool {
        self.unpack().high32.is_relation_exclusive
    }
    pub fn is_relationship(&self) -> bool {
        self.unpack().high32.is_relationship
    }
    pub fn is_relation(&self) -> bool {
        self.unpack().high32.is_relation
    }
    pub fn is_target(&self) -> bool {
        self.unpack().high32.is_target
    }
    pub fn is_tag(&self) -> bool {
        self.unpack().high32.is_watched
    }
}

#[derive(PackedStruct, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IdentifierUnpacked {
    #[packed_field(endian = "msb")]
    pub low32: u32,
    #[packed_field(endian = "msb", element_size_bytes = "4")]
    pub high32: IdentifierHigh32,
}

#[derive(PackedStruct, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[packed_struct(bit_numbering = "msb0")]
pub struct IdentifierHigh32 {
    #[packed_field(endian = "msb", bits = "0..=24")]
    pub second: Integer<u32, Bits<25>>,
    #[packed_field(bits = "25")]
    //unused
    pub is_watched: bool,
    #[packed_field(bits = "26")]
    pub is_target: bool,
    #[packed_field(bits = "27")]
    pub is_relation: bool,
    #[packed_field(bits = "28")]
    //unused
    pub is_target_exclusive: bool,
    #[packed_field(bits = "29")]
    pub is_relation_exclusive: bool,
    #[packed_field(bits = "30")]
    pub is_active: bool,
    #[packed_field(bits = "31")]
    pub is_relationship: bool,
}

impl From<u64> for Identifier {
    fn from(value: u64) -> Self {
        Self(value.to_be_bytes())
    }
}

impl From<IdentifierUnpacked> for Identifier {
    fn from(value: IdentifierUnpacked) -> Self {
        Self(value.pack().unwrap())
    }
}

impl From<[u8; 8]> for Identifier {
    fn from(value: [u8; 8]) -> Self {
        Identifier(value)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_kind() {
        let none: Identifier = IdentifierUnpacked {
            low32: 1,
            high32: IdentifierHigh32 {
                second: 2.into(),
                ..Default::default()
            }
        }.pack().unwrap().into();
        let relation: Identifier = IdentifierUnpacked {
            low32: WILDCARD_32,
            high32: IdentifierHigh32 {
                second: 2.into(),
                ..Default::default()
            }
        }.pack().unwrap().into();
        let target: Identifier = IdentifierUnpacked {
            low32: 1,
            high32: IdentifierHigh32 {
                second: WILDCARD_25.into(),
                ..Default::default()
            }
        }.pack().unwrap().into();
        let both: Identifier = IdentifierUnpacked {
            low32: WILDCARD_32,
            high32: IdentifierHigh32 {
                second: WILDCARD_25.into(),
                ..Default::default()
            }
        }.pack().unwrap().into();

        assert_eq!(none.wildcard_kind(), WildcardKind::None);
        assert_eq!(relation.wildcard_kind(), WildcardKind::Relation);
        assert_eq!(target.wildcard_kind(), WildcardKind::Target);
        assert_eq!(both.wildcard_kind(), WildcardKind::Both);
    }
}
