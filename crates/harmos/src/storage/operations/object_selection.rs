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

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::Selection;
#[cfg(feature = "grpc")]
use crate::__private::DecodeInput;
#[cfg(feature = "grpc")]
use crate::storage::codec::Unsigned;
use crate::storage::codec::{Nested, Optional, Value, product};
use crate::storage::object_values::Key;
use crate::storage::{ObjectKey, ObjectVersions, SchemaKey};
use crate::{Error, MemorySize, MessageError, Result};

/* === Constants === */

/* === Macros === */
product!(ObjectSelection {
    selection: Nested<Selection> => 1,
    schema: Optional<Nested<SchemaKey>> => 2,
    key: Optional<Key> => 3,
    versions: Versions => 4,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectSelection {
    pub selection: Selection,
    pub schema: Option<SchemaKey>,
    pub key: Option<ObjectKey>,
    pub versions: ObjectVersions,
}
struct Versions;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Default for ObjectSelection {
    fn default() -> Self {
        Self {
            selection: Selection::default(),
            schema: None,
            key: None,
            versions: ObjectVersions::Latest,
        }
    }
}
impl ObjectSelection {
    pub fn validate(&self) -> Result<()> {
        self.selection.validate()?;
        if self.selection.scope.is_some() || self.selection.item.is_some() {
            return Err(Error::InvalidInput {
                field: "object selection",
                reason: "objects use opaque keys, not scope or item identifiers",
            });
        }
        Ok(())
    }

    pub(super) fn validate_follow(&self) -> Result<()> {
        self.validate()?;
        if self.versions != ObjectVersions::Revisions
            || self.selection.descending
            || self.selection.through.is_some()
        {
            return Err(Error::InvalidInput {
                field: "object follow",
                reason: "following requires ascending open-ended revisions",
            });
        }
        Ok(())
    }

    pub(super) fn query(&self, following: bool) -> Result<crate::storage::EncodedObjectQuery> {
        if following {
            self.validate_follow()?;
        } else {
            self.validate()?;
        }
        Ok(crate::storage::EncodedObjectQuery {
            schema: self.schema.clone(),
            item: self.key.clone(),
            versions: self.versions,
            history: if following {
                crate::storage::ReadHistory::Complete
            } else {
                crate::storage::ReadHistory::Retained
            },
            options: self.selection.options()?,
        })
    }
}
impl MemorySize for ObjectSelection {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.selection.memory_size()
            - std::mem::size_of_val(&self.selection)
            + self.schema.memory_size()
            - std::mem::size_of_val(&self.schema)
            + self.key.memory_size()
            - std::mem::size_of_val(&self.key)
    }
}
impl Value for Versions {
    fn postcard_write(value: &Self::Native, bytes: &mut Vec<u8>) {
        crate::__private::PostcardValue::write(
            &match value {
                ObjectVersions::Revisions => 0u64,
                ObjectVersions::Latest => 1u64,
            },
            bytes,
        );
    }

    fn postcard_size(_: &Self::Native) -> usize {
        1
    }

    fn postcard_memory(
        input: &mut crate::__private::PostcardReader<'_>,
    ) -> std::result::Result<usize, MessageError> {
        Self::postcard_read(input)?;
        Ok(0)
    }

    fn postcard_read(
        input: &mut crate::__private::PostcardReader<'_>,
    ) -> std::result::Result<Self::Native, MessageError> {
        match input.value::<u64>()? {
            0 => Ok(ObjectVersions::Revisions),
            1 => Ok(ObjectVersions::Latest),
            _ => Err(MessageError::borrowed("invalid storage selector")),
        }
    }

    type Native = ObjectVersions;
    const KIND: crate::schema::Kind = crate::schema::Kind::U64;
    #[cfg(feature = "grpc")]
    fn write(value: &ObjectVersions, tag: u32, bytes: &mut Vec<u8>) {
        Unsigned::write(&u64::from(*value == ObjectVersions::Latest), tag, bytes);
    }

    #[cfg(feature = "grpc")]
    fn size(value: &ObjectVersions, tag: u32) -> usize {
        Unsigned::size(&u64::from(*value == ObjectVersions::Latest), tag)
    }

    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> std::result::Result<usize, MessageError> {
        Self::read(input, tag).map(|_| 0)
    }

    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> std::result::Result<ObjectVersions, MessageError> {
        match Unsigned::read(input, tag)? {
            0 => Ok(ObjectVersions::Revisions),
            1 => Ok(ObjectVersions::Latest),
            _ => Err(MessageError::borrowed("invalid object versions")),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
