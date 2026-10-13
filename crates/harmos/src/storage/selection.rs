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
use serde::Serialize;
// Local Imports
use super::manager::Operation;
use super::*;
use crate::Identifier;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */
pub(super) trait Entry: Clone + Serialize + Send + Sync + 'static {
    const KIND: ReadKind;
    fn pending(operation: &Operation) -> &[Self];
    fn read_limits(limits: ReadLimits) -> ReadLimits {
        if Self::KIND == ReadKind::Schemas {
            ReadLimits {
                rows: std::num::NonZeroUsize::MIN,
                ..limits
            }
        } else {
            limits
        }
    }

    fn decoding_overhead(limits: ReadLimits) -> Option<usize> {
        if Self::KIND == ReadKind::Schemas {
            super::schema::decoding_overhead(limits.bytes.get())
        } else {
            Some(0)
        }
    }

    fn validate(&self, _: &super::manager::Inner, _: ReadLimits) -> crate::Result<()> {
        Ok(())
    }

    fn validate_read(
        &self,
        inner: &super::manager::Inner,
        limits: ReadLimits,
    ) -> crate::Result<()> {
        self.validate(inner, limits)
    }

    fn position(&self) -> &StorageCursor;
    fn part(entries: &[Self]) -> StoragePart<'_>;
    fn matches(&self, selection: &ReadSelection) -> bool;
    fn fetch<'a>(
        backend: &'a dyn super::backend::ErasedBackend,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<Self>>;
}

/* === Implementations === */
impl Entry for ObjectEntry {
    fn part(entries: &[Self]) -> StoragePart<'_> {
        StoragePart::Objects(entries)
    }

    fn pending(operation: &Operation) -> &[Self] {
        match operation {
            Operation::ObjectGroup { objects, .. } => objects,
            _ => &[],
        }
    }
    const KIND: ReadKind = ReadKind::Objects;
    fn position(&self) -> &StorageCursor {
        &self.position
    }

    fn matches(&self, selection: &ReadSelection) -> bool {
        selection
            .schema
            .as_ref()
            .is_none_or(|key| key == &self.value.schema)
            && selection
                .object_key
                .as_ref()
                .is_none_or(|key| key == &self.value.item)
    }

    fn validate(&self, inner: &super::manager::Inner, limits: ReadLimits) -> crate::Result<()> {
        self.version.check_position(&self.position)?;
        (inner.validate_object)(&self.value, limits.bytes)
    }

    fn validate_read(&self, _: &super::manager::Inner, _: ReadLimits) -> crate::Result<()> {
        // Typed Reads Verify Payload And Key Under Their Original Output Preclaim.
        self.version.check_position(&self.position)
    }

    fn fetch<'a>(
        backend: &'a dyn super::backend::ErasedBackend,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<Self>> {
        backend.read_objects(view, selection, cursor, limits)
    }
}

impl Entry for StoredRecord {
    fn part(entries: &[Self]) -> StoragePart<'_> {
        StoragePart::Records(entries)
    }

    fn pending(operation: &Operation) -> &[Self] {
        match operation {
            Operation::Records(values) => values,
            _ => &[],
        }
    }
    const KIND: ReadKind = ReadKind::Records;
    fn position(&self) -> &StorageCursor {
        &self.position
    }

    fn matches(&self, selection: &ReadSelection) -> bool {
        selection
            .schema
            .as_ref()
            .is_none_or(|key| key == &self.schema)
            && selection
                .source_time
                .as_ref()
                .is_none_or(|range| range.contains(self.source_time))
            && optional(&selection.scope, self.scope.as_ref())
            && optional(&selection.item, Some(&self.item))
    }

    fn fetch<'a>(
        backend: &'a dyn super::backend::ErasedBackend,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<Self>> {
        backend.read_records(view, selection, cursor, limits)
    }
}

impl Entry for StoredRow {
    fn part(entries: &[Self]) -> StoragePart<'_> {
        StoragePart::Stream(entries)
    }

    fn pending(operation: &Operation) -> &[Self] {
        match operation {
            Operation::Stream(values) => values,
            _ => &[],
        }
    }
    const KIND: ReadKind = ReadKind::Stream;
    fn position(&self) -> &StorageCursor {
        &self.record.position
    }

    fn matches(&self, selection: &ReadSelection) -> bool {
        self.record.matches(selection) && optional(&selection.stream, Some(&self.stream))
    }

    fn fetch<'a>(
        backend: &'a dyn super::backend::ErasedBackend,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<Self>> {
        backend.read_stream(view, selection, cursor, limits)
    }
}

impl Entry for StorageSchema {
    fn validate(&self, _: &super::manager::Inner, limits: ReadLimits) -> crate::Result<()> {
        self.schema
            .prepare(&harmos_contract::config::SchemaSettings::default())
            .map_err(|_| crate::Error::InvalidInput {
                field: "storage schema",
                reason: "invalid stored shared schema",
            })?;
        let bound = limits
            .bytes
            .get()
            .checked_mul(4)
            .and_then(|bytes| {
                bytes.checked_add(super::schema::decoding_overhead(limits.bytes.get())?)
            })
            .ok_or(crate::Error::InvalidInput {
                field: "storage schema",
                reason: "decoded size overflow",
            })?;
        if crate::MemorySize::memory_size(self) > bound {
            return Err(crate::Error::InvalidInput {
                field: "storage schema",
                reason: "decoded schema exceeds its original read reservation",
            });
        }
        Ok(())
    }

    fn part(entries: &[Self]) -> StoragePart<'_> {
        StoragePart::Schemas(entries)
    }

    fn pending(operation: &Operation) -> &[Self] {
        match operation {
            Operation::Schema(values) => std::slice::from_ref(values),
            Operation::ObjectGroup { schemas, .. } => schemas,
            _ => &[],
        }
    }
    const KIND: ReadKind = ReadKind::Schemas;
    fn position(&self) -> &StorageCursor {
        &self.position
    }

    fn matches(&self, selection: &ReadSelection) -> bool {
        selection.schema.as_ref().is_none_or(|key| key == &self.key)
            && optional(&selection.item, Some(&self.key.id))
    }

    fn fetch<'a>(
        backend: &'a dyn super::backend::ErasedBackend,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<Self>> {
        backend.read_schemas(view, selection, cursor, limits)
    }
}

impl Entry for FileReference {
    fn part(entries: &[Self]) -> StoragePart<'_> {
        StoragePart::Files(entries)
    }

    fn pending(operation: &Operation) -> &[Self] {
        match operation {
            Operation::File(values) => std::slice::from_ref(values),
            _ => &[],
        }
    }
    const KIND: ReadKind = ReadKind::Files;
    fn position(&self) -> &StorageCursor {
        &self.position
    }

    fn matches(&self, selection: &ReadSelection) -> bool {
        optional(&selection.scope, self.metadata.scope.as_ref())
            && optional(&selection.item, Some(&self.metadata.id))
    }

    fn fetch<'a>(
        backend: &'a dyn super::backend::ErasedBackend,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<Self>> {
        backend.read_files(view, selection, cursor, limits)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn optional(expected: &Option<Identifier>, actual: Option<&Identifier>) -> bool {
    expected
        .as_ref()
        .is_none_or(|expected| Some(expected) == actual)
}

/* === Tests === */
