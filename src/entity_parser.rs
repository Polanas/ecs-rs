use std::fmt::Debug;

use regex::Regex;
use serde_json::Value;
use smol_str::{SmolStr, ToSmolStr};
use thiserror::Error;

use crate::{
    archetypes::{Archetypes, DeserializeFn, MyTypeRegistry, NameLeft, NameRight, RelDataPosition}, either::Either, expect_fn::ExpectFnOption, identifier::Identifier
};

pub struct EntityParser {
    tag: Regex,
    tag_rel_regex: Regex,
    rel_data_first_regex: Regex,
    rel_data_second_regex: Regex,
}

#[derive(Debug, Clone, Copy)]
pub enum TagType {
    Type,
    Entity,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("error parsing json")]
    SerdeError(#[from] serde_json::Error),
    #[error("unknown type: '{0}'. If you meant to add a tag, use # prefix: '#{0}'")]
    UnknownType(SmolStr),
    #[error("expected json data to be an object")]
    JsonIsNotObject,
    #[error("expected 'Tags' to be an array (of tags)")]
    TagsIsNotArray,
}

impl TagType {
    pub fn is_type(&self) -> bool {
        matches!(self, TagType::Type)
    }
}
pub type IdOrName = Either<(Identifier, TagType), SmolStr>;
#[derive(Debug)]
pub enum ComponentType {
    Regular,
    DataRelationship(RelDataPosition),
}
#[derive(Debug)]
pub enum ParsedEntityItem {
    Tag(IdOrName),
    RelationshipTag(IdOrName, IdOrName),
    Component(Identifier, DeserializeFn, serde_json::Value, ComponentType),
    Name(SmolStr),
}
impl EntityParser {
    pub fn new() -> Self {
        Self {
            tag: Regex::new(r"(#?)(\w+)").unwrap(),
            tag_rel_regex: Regex::new(r"\((#?)(\w+), (#?)(\w+)\)").unwrap(),
            rel_data_first_regex: Regex::new(r"\(\$(\w+), (\w+)\)").unwrap(),
            rel_data_second_regex: Regex::new(r"\((\w+), \$(\w+)\)").unwrap(),
        }
    }

    pub fn id_or_name(
        &self,
        archetypes: &Archetypes,
        type_registry: &std::cell::Ref<MyTypeRegistry>,
        name: SmolStr,
        marked_as_tag: bool,
    ) -> Result<IdOrName, ParseError> {
        if marked_as_tag {
            if let Some(id) = archetypes.entity_by_global_name(name.clone()) {
                Ok(Either::First((id, TagType::Entity)))
            } else {
                Ok(Either::Second(name))
            }
        } else if let Some(id) = type_registry.identifiers_by_names.get(&name) {
            Ok(Either::First((*id, TagType::Type)))
        } else {
            Err(ParseError::UnknownType(name))
        }
    }

    pub fn parse(
        &self,
        json: &str,
        archetypes: &Archetypes,
    ) -> Result<impl Iterator<Item = ParsedEntityItem>, ParseError> {
        let mut components = vec![];
        let value = serde_json::from_str::<Value>(json)?;
        let Some(object) = value.as_object() else {
            return Err(ParseError::JsonIsNotObject);
        };
        let type_registry = archetypes.type_registry_rc();
        let type_registry = type_registry.borrow();
        if let Some(name) = object.get("Name") {
            if let Some(name) = name.as_str() {
                components.push(ParsedEntityItem::Name(name.into()));
            }
        }
        for (key, value) in object.iter() {
            if key == "Tags" {
                let Some(tags) = value.as_array() else {
                    return Err(ParseError::TagsIsNotArray);
                };
                for tag in tags.iter().filter_map(|v| v.as_str()) {
                    let tag = tag.to_smolstr();
                    if let Some(captures) = self.tag_rel_regex.captures(&tag) {
                        let relation_name = captures[2].to_smolstr();
                        let target_name = captures[4].to_smolstr();
                        let relation = self.id_or_name(
                            archetypes,
                            &type_registry,
                            relation_name,
                            !captures[1].is_empty(),
                        )?;
                        let target = self.id_or_name(
                            archetypes,
                            &type_registry,
                            target_name,
                            !captures[3].is_empty(),
                        )?;
                        components.push(ParsedEntityItem::RelationshipTag(relation, target));
                    } else if let Some(captures) = self.tag.captures(&tag) {
                        let tag = &captures[2];
                        components.push(ParsedEntityItem::Tag(self.id_or_name(
                            archetypes,
                            &type_registry,
                            tag.to_smolstr(),
                            !captures[1].is_empty(),
                        )?))
                    }
                    continue;
                }
            }
            if let Some(id) = type_registry.identifiers_by_names.get(&key.to_smolstr()) {
                let deserialize_fn = type_registry.functions.get(&id.stripped()).expect("Expected deseriailzation fn for {0}. It's either a tag or you forgot to call register_component").deserialize;
                components.push(ParsedEntityItem::Component(
                    *id,
                    deserialize_fn,
                    value.clone(),
                    ComponentType::Regular,
                ));
            } else if let Some(captures) = self.rel_data_first_regex.captures(key) {
                let relation = captures[1].to_smolstr();
                let target = captures[2].to_smolstr();
                let Some(relation_id) = type_registry.identifiers_by_names.get(&relation).copied()
                else {
                    return Err(ParseError::UnknownType(relation));
                };
                let Some(target_id) = type_registry
                    .identifiers_by_names
                    .get(&target.to_smolstr())
                    .copied()
                else {
                    return Err(ParseError::UnknownType(target));
                };
                let relationship = Archetypes::relationship_id(relation_id, target_id);
                let deserialize_fn = type_registry.functions.get(&relation_id.stripped()).expect("Expected deseriailzation fn for {0}. It's either a tag or you forgot to call register_component").deserialize;
                components.push(ParsedEntityItem::Component(
                    relationship,
                    deserialize_fn,
                    value.clone(),
                    ComponentType::DataRelationship(RelDataPosition::First),
                ));
            } else if let Some(captures) = self.rel_data_second_regex.captures(key) {
                let relation = captures[1].to_smolstr();
                let target = captures[2].to_smolstr();
                let Some(relation_id) = type_registry.identifiers_by_names.get(&relation).copied()
                else {
                    return Err(ParseError::UnknownType(relation));
                };
                let Some(target_id) = type_registry
                    .identifiers_by_names
                    .get(&target.to_smolstr())
                    .copied()
                else {
                    return Err(ParseError::UnknownType(target));
                };
                let relationship = Archetypes::relationship_id(relation_id, target_id);
                let deserialize_fn = type_registry.functions.get(&target_id.stripped()).expect_fn(|| format!("expected deseriailzation fn for {0}. It's either a tag or you forgot to call register_component", relation)).deserialize;
                components.push(ParsedEntityItem::Component(
                    relationship,
                    deserialize_fn,
                    value.clone(),
                    ComponentType::DataRelationship(RelDataPosition::Second),
                ));
            }
        }

        Ok(components.into_iter())
    }
}

impl Default for EntityParser {
    fn default() -> Self {
        Self::new()
    }
}
