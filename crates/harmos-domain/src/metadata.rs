/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod codec;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::{collections::BTreeMap, fmt};
// Third Party Imports
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
// Local Imports
use crate::{MemorySize, Value};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Identifier(String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    pub identifier: Identifier,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "unique_tags")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
    #[serde(default, with = "crate::value::optional_datetime")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(default, with = "crate::value::optional_datetime")]
    pub updated_at: Option<OffsetDateTime>,
}

/* === Enums === */
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum IdentifierError {
    #[error("identifier must not be empty")]
    Empty,
    #[error("identifier must start with an ASCII letter, number, or underscore")]
    InvalidStart,
    #[error("identifier may contain only ASCII letters, numbers, '.', '_', or '-'")]
    InvalidCharacter,
}

/* === Traits === */

/* === Implementations === */
impl MemorySize for Metadata {
    fn memory_size(&self) -> usize {
        let fields = [
            (self.identifier.memory_size(), std::mem::size_of_val(&self.identifier)),
            (self.label.memory_size(), std::mem::size_of_val(&self.label)),
            (self.description.memory_size(), std::mem::size_of_val(&self.description)),
            (self.tags.memory_size(), std::mem::size_of_val(&self.tags)),
            (self.attributes.memory_size(), std::mem::size_of_val(&self.attributes)),
        ];
        fields.into_iter().fold(std::mem::size_of::<Self>(), |bytes, (owned, inline)| {
            if owned == usize::MAX { usize::MAX } else { bytes.saturating_add(owned.saturating_sub(inline)) }
        })
    }
}

impl MemorySize for Identifier {
    fn memory_size(&self) -> usize {
        self.0.memory_size()
    }
}

impl Identifier {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        let value = value.into();
        validate_identifier(&value)?;

        Ok(Self(value))
    }

    pub fn declared(value: impl Into<String>, kind: &str) -> Self {
        let value = value.into();
        Self::new(value.clone()).unwrap_or_else(|error| panic!("invalid {kind} identifier {value:?}: {error}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl crate::__private::PostcardValue for Identifier {
    fn write(&self, bytes: &mut Vec<u8>) {
        crate::__private::PostcardValue::write(&self.0, bytes);
    }
    fn size(&self) -> usize {
        crate::__private::PostcardValue::size(&self.0)
    }
    fn memory(input: &mut crate::__private::PostcardReader<'_>) -> Result<usize, crate::MessageError> {
        let text: &str = input.value()?;
        validate_identifier(text).map_err(|_| crate::MessageError::borrowed("invalid identifier"))?;
        crate::__private::decoding_add(std::mem::size_of::<Self>(), text.len())
    }
    fn read(input: &mut crate::__private::PostcardReader<'_>) -> Result<Self, crate::MessageError> {
        let text: &str = input.value()?;
        validate_identifier(text).map_err(|_| crate::MessageError::borrowed("invalid identifier"))?;
        Ok(Self(Box::<str>::from(text).into_string()))
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Identifier {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl Metadata {
    pub fn new(identifier: Identifier, label: impl Into<String>, description: Option<String>) -> Self {
        Self::definition(identifier, label, description, std::iter::empty::<String>())
    }

    pub fn definition(
        identifier: Identifier, label: impl Into<String>, description: Option<String>, tags: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut unique_tags = Vec::new();
        for tag in tags {
            let tag = tag.into();
            if !unique_tags.contains(&tag) {
                unique_tags.push(tag);
            }
        }

        Self {
            identifier,
            label: label.into(),
            description,
            tags: unique_tags,
            attributes: BTreeMap::new(),
            created_at: None,
            updated_at: None,
        }
    }
}

/* === Public Functions === */
pub(crate) const fn validate_identifier(value: &str) -> Result<(), IdentifierError> {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return Err(IdentifierError::Empty);
    }
    if !(bytes[0].is_ascii_alphanumeric() || bytes[0] == b'_') {
        return Err(IdentifierError::InvalidStart);
    }
    let mut index = 1;
    while index < bytes.len() {
        if !(bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'.' | b'_' | b'-')) {
            return Err(IdentifierError::InvalidCharacter);
        }
        index += 1;
    }
    Ok(())
}

/* === Private Functions === */
fn unique_tags<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    let tags = Vec::<String>::deserialize(deserializer)?;
    let mut unique = Vec::new();

    for tag in tags {
        if !unique.contains(&tag) {
            unique.push(tag);
        }
    }

    Ok(unique)
}

/* === Tests === */
