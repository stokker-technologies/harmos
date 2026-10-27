/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

pub use crate::message::{Digest, Fingerprint};

/* === Imports === */

// Standard Library Imports
use std::fmt;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct QualifiedId(String);

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("identity `{value}` must be a lowercase portable owner segment followed by dotted local segments")]
pub struct IdentityError {
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DefinitionId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requirement {
    pub id: String,
    pub version: semver::VersionReq,
}

/* === Enums === */

/* === Traits === */

// The Stable Identity And Semantic Version Of A Declared Definition.
pub trait Identified {
    fn definition() -> (DefinitionId, u32);
}

/* === Implementations === */

impl QualifiedId {
    pub fn parse(value: &str) -> Result<Self, IdentityError> {
        let Some((owner, local)) = value.split_once('.') else {
            return Err(IdentityError { value: value.to_owned() });
        };
        Self::qualify(owner, local)
    }

    pub fn qualify(owner: &str, local: &str) -> Result<Self, IdentityError> {
        if !segment(owner) || !dotted(local) {
            return Err(IdentityError { value: format!("{owner}.{local}") });
        }
        Ok(Self(format!("{owner}.{local}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for QualifiedId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl TryFrom<String> for QualifiedId {
    type Error = IdentityError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<QualifiedId> for String {
    fn from(value: QualifiedId) -> Self {
        value.0
    }
}

impl DefinitionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// The Unit Definition Of A Catalog That Declares Nothing.
impl Identified for () {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("harmos:unit"), 1)
    }
}

impl fmt::Display for DefinitionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/* === Public Functions === */

pub const fn segment(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut index = 1;
    while index < bytes.len() {
        if !(bytes[index].is_ascii_lowercase() || bytes[index].is_ascii_digit() || bytes[index] == b'_') {
            return false;
        }
        index += 1;
    }
    true
}

// One Or More Portable Segments Joined By Single Dots.
pub const fn dotted(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut start = 0;
    let mut index = 0;
    while index <= bytes.len() {
        if index == bytes.len() || bytes[index] == b'.' {
            let length = index - start;
            if length == 0 || !bytes[start].is_ascii_lowercase() {
                return false;
            }
            let mut cursor = start + 1;
            while cursor < index {
                if !(bytes[cursor].is_ascii_lowercase() || bytes[cursor].is_ascii_digit() || bytes[cursor] == b'_') {
                    return false;
                }
                cursor += 1;
            }
            start = index + 1;
        }
        index += 1;
    }
    true
}

pub const fn artifact_subset(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_lowercase() {
        return false;
    }

    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.' | b'_')) {
            return false;
        }
        index += 1;
    }

    true
}

pub const fn qualified_len(owner: &str, name: &str) -> usize {
    owner.len() + 1 + name.len()
}

pub const fn qualify<const N: usize>(owner: &str, name: &str) -> [u8; N] {
    assert!(N == qualified_len(owner, name));
    let mut output = [0; N];
    let mut cursor = 0;
    let owner = owner.as_bytes();
    let mut index = 0;
    while index < owner.len() {
        output[cursor] = owner[index];
        cursor += 1;
        index += 1;
    }
    output[cursor] = b'.';
    cursor += 1;
    let name = name.as_bytes();
    let mut index = 0;
    while index < name.len() {
        output[cursor] = name[index];
        cursor += 1;
        index += 1;
    }

    output
}

pub const fn qualified(bytes: &'static [u8]) -> &'static str {
    match core::str::from_utf8(bytes) {
        Ok(id) => id,
        Err(_) => panic!("a qualified id is built from two portable names"),
    }
}

pub fn stream_subset(name: &str) -> bool {
    name.split('.').all(|segment| {
        let mut characters = segment.chars();
        characters.next().is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
            && characters.all(|character| character == '_' || character == '-' || character.is_ascii_alphanumeric())
    })
}

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::QualifiedId;

    #[test]
    fn qualified_segments() {
        assert_eq!(QualifiedId::qualify("host", "read_2").unwrap().as_str(), "host.read_2");
        assert_eq!(QualifiedId::parse("host.storage.read_file").unwrap().as_str(), "host.storage.read_file");
        for value in ["read", "host..read", ".read", "host.", "Host.read", "host.read-file", "host.2read", "host.read\0"] {
            assert!(QualifiedId::parse(value).is_err(), "{value:?}");
        }
        assert!(QualifiedId::qualify("owner.other", "read").is_err());
    }

    #[test]
    fn dotted_local_segments() {
        for value in ["read", "storage.read_file", "host.operations.cancel", "a0.b_2"] {
            assert!(super::dotted(value), "{value:?}");
        }
        for value in ["", ".read", "read.", "read..file", "Read", "read.2file", "read.file-name", "read.é"] {
            assert!(!super::dotted(value), "{value:?}");
        }
    }
}
