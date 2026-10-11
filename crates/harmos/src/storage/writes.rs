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
use std::borrow::Cow;
// Third Party Imports
// Local Imports
use super::manager::ENTRY_OVERHEAD;
use super::{SchemaEncoding, SchemaKey, StorageCursor, StoreHistory};
use crate::schema::{Field, Kind};
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageSchema {
    pub position: StorageCursor,
    pub key: SchemaKey,
    pub fields: Vec<crate::schema::Field>,
    encoding: SchemaEncoding,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemorySize for StorageSchema {
    fn memory_size(&self) -> usize {
        fn fields_bytes(fields: &[Field]) -> usize {
            fields.iter().fold(
                fields.len().saturating_mul(std::mem::size_of::<Field>()),
                |bytes, field| {
                    let name = match &field.name {
                        Cow::Borrowed(_) => 0,
                        Cow::Owned(name) => name.capacity(),
                    };
                    let children = match &field.kind {
                        Kind::Message {
                            fields: Cow::Owned(fields),
                        } => fields_bytes(fields).saturating_add(
                            (fields.capacity() - fields.len())
                                .saturating_mul(std::mem::size_of::<Field>()),
                        ),
                        _ => 0,
                    };
                    bytes.saturating_add(name).saturating_add(children)
                },
            )
        }
        std::mem::size_of::<Self>()
            .saturating_add(
                self.position
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<StorageCursor>()),
            )
            .saturating_add(
                self.key
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<SchemaKey>()),
            )
            .saturating_add(fields_bytes(&self.fields))
            .saturating_add(
                (self.fields.capacity() - self.fields.len())
                    .saturating_mul(std::mem::size_of::<Field>()),
            )
    }
}

impl StorageSchema {
    pub fn new(
        position: StorageCursor,
        key: SchemaKey,
        fields: Vec<Field>,
        encoding: SchemaEncoding,
    ) -> Self {
        Self {
            position,
            key,
            fields,
            encoding,
        }
    }

    pub fn encoding(&self) -> SchemaEncoding {
        self.encoding
    }

    pub fn encoding_memory_bytes(&self) -> Result<usize> {
        self.encoding.encoding_memory_bytes(&self.fields)
    }

    pub(super) fn resident_bytes(
        key: &SchemaKey,
        fields: &[Field],
        history: &StoreHistory,
        encoding: SchemaEncoding,
    ) -> Result<usize> {
        [
            std::mem::size_of::<Self>(),
            ENTRY_OVERHEAD,
            key.id.as_str().len(),
            history.store.as_str().len(),
            history.epoch.as_str().len(),
            cloned_fields_bytes(fields)?,
            encoding.planning_bytes(fields)?,
        ]
        .into_iter()
        .try_fold(0usize, |bytes, next| bytes.checked_add(next))
        .ok_or(size_error())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn cloned_fields_bytes(fields: &[Field]) -> Result<usize> {
    let slots = fields
        .len()
        .checked_mul(std::mem::size_of::<Field>())
        .ok_or(size_error())?;

    fields.iter().try_fold(slots, |bytes, field| {
        let name = match &field.name {
            Cow::Borrowed(_) => 0,
            Cow::Owned(name) => name.len(),
        };
        let children = match &field.kind {
            Kind::Message {
                fields: Cow::Owned(fields),
            } => cloned_fields_bytes(fields)?,
            _ => 0,
        };

        bytes
            .checked_add(name)
            .and_then(|bytes| bytes.checked_add(children))
            .ok_or(size_error())
    })
}

fn size_error() -> Error {
    Error::InvalidInput {
        field: "storage schema",
        reason: "size overflow",
    }
}

/* === Tests === */
