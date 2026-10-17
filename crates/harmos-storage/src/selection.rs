/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Stored Entry Kinds Shared By Scans And Live Subscriptions

/* === Modules === */
mod sealed {
    pub trait Sealed {}
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::time::SystemTime;

// Third Party Imports
use serde::Serialize;

// Local Imports
use super::backend::{BackendSnapshot, ScanPage, StorageBackend};
use super::manager::Operation;
use super::query::{
    FileSelection, ObjectSelection, RecordSelection, SchemaSelection, StreamSelection,
};
use super::{
    FileReference, ObjectEntry, StorageCursor, StorageFuture, StorageSchema, StoredRecord,
    StoredRow,
};
use crate::Identifier;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
/// One accepted queue operation, opaque outside this crate.
pub struct Queued<'a>(&'a Operation);

/* === Enums === */

/* === Traits === */
/// One stored entry kind: how it is scanned, matched, positioned and found in
/// the accepted queue. Sealed: the engine owns every implementation.
pub trait Entry: Clone + Serialize + Send + Sync + 'static + sealed::Sealed {
    type Selection: Send + Sync;

    fn position(&self) -> &StorageCursor;
    fn matches(&self, selection: &Self::Selection) -> bool;
    /// The storage position a selection starts after; scans begin there.
    fn start(_selection: &Self::Selection) -> u64 {
        0
    }
    fn pending(queued: Queued<'_>) -> &[Self];
    fn scan<'a>(
        backend: &'a dyn StorageBackend,
        snapshot: &'a BackendSnapshot,
        selection: &'a Self::Selection,
        page: ScanPage,
    ) -> StorageFuture<'a, Vec<Self>>;
}

/* === Implementations === */
impl<'a> Queued<'a> {
    pub(super) fn new(operation: &'a Operation) -> Self {
        Self(operation)
    }
}

impl sealed::Sealed for ObjectEntry {}
impl sealed::Sealed for StoredRecord {}
impl sealed::Sealed for StoredRow {}
impl sealed::Sealed for StorageSchema {}
impl sealed::Sealed for FileReference {}

impl Entry for ObjectEntry {
    type Selection = ObjectSelection;

    fn position(&self) -> &StorageCursor {
        &self.position
    }

    // Latest selection is resolved by the backend; matching checks identity only.
    fn matches(&self, selection: &ObjectSelection) -> bool {
        self.version.check_position(&self.position).is_ok()
            && selection
                .schema
                .as_ref()
                .is_none_or(|key| key == &self.value.schema)
            && selection
                .key
                .as_ref()
                .is_none_or(|key| key == &self.value.item)
    }

    fn pending(queued: Queued<'_>) -> &[Self] {
        match queued.0 {
            Operation::ObjectGroup { objects, .. } => objects,
            _ => &[],
        }
    }

    fn scan<'a>(
        backend: &'a dyn StorageBackend,
        snapshot: &'a BackendSnapshot,
        selection: &'a ObjectSelection,
        page: ScanPage,
    ) -> StorageFuture<'a, Vec<Self>> {
        backend.read_objects(snapshot, selection, page)
    }
}

impl Entry for StoredRecord {
    type Selection = RecordSelection;

    fn position(&self) -> &StorageCursor {
        &self.position
    }

    fn matches(&self, selection: &RecordSelection) -> bool {
        self.schema == selection.schema
            && optional(&selection.scope, self.scope.as_ref())
            && optional(&selection.item, Some(&self.item))
            && within(
                selection.source_start,
                selection.source_end,
                self.source_time,
            )
    }

    fn pending(queued: Queued<'_>) -> &[Self] {
        match queued.0 {
            Operation::Records(values) => values,
            _ => &[],
        }
    }

    fn scan<'a>(
        backend: &'a dyn StorageBackend,
        snapshot: &'a BackendSnapshot,
        selection: &'a RecordSelection,
        page: ScanPage,
    ) -> StorageFuture<'a, Vec<Self>> {
        backend.read_records(snapshot, selection, page)
    }
}

impl Entry for StoredRow {
    type Selection = StreamSelection;

    fn position(&self) -> &StorageCursor {
        &self.record.position
    }

    fn matches(&self, selection: &StreamSelection) -> bool {
        let record = &self.record;
        record.schema == selection.schema
            && record.position.sequence > selection.after
            && optional(&selection.scope, record.scope.as_ref())
            && optional(&selection.item, Some(&record.item))
            && optional(&selection.stream, Some(&self.stream))
            && within(
                selection.source_start,
                selection.source_end,
                record.source_time,
            )
    }

    fn start(selection: &StreamSelection) -> u64 {
        selection.after
    }

    fn pending(queued: Queued<'_>) -> &[Self] {
        match queued.0 {
            Operation::Stream(values) => values,
            _ => &[],
        }
    }

    fn scan<'a>(
        backend: &'a dyn StorageBackend,
        snapshot: &'a BackendSnapshot,
        selection: &'a StreamSelection,
        page: ScanPage,
    ) -> StorageFuture<'a, Vec<Self>> {
        backend.read_stream(snapshot, selection, page)
    }
}

impl Entry for StorageSchema {
    type Selection = SchemaSelection;

    fn position(&self) -> &StorageCursor {
        &self.position
    }

    fn matches(&self, selection: &SchemaSelection) -> bool {
        selection.schema.as_ref().is_none_or(|key| key == &self.key)
    }

    fn pending(queued: Queued<'_>) -> &[Self] {
        match queued.0 {
            Operation::Schema(value) => std::slice::from_ref(value),
            Operation::ObjectGroup { schemas, .. } => schemas,
            _ => &[],
        }
    }

    fn scan<'a>(
        backend: &'a dyn StorageBackend,
        snapshot: &'a BackendSnapshot,
        selection: &'a SchemaSelection,
        page: ScanPage,
    ) -> StorageFuture<'a, Vec<Self>> {
        backend.read_schemas(snapshot, selection, page)
    }
}

impl Entry for FileReference {
    type Selection = FileSelection;

    fn position(&self) -> &StorageCursor {
        &self.position
    }

    fn matches(&self, selection: &FileSelection) -> bool {
        optional(&selection.scope, self.metadata.scope.as_ref())
            && optional(&selection.item, Some(&self.metadata.id))
    }

    fn pending(queued: Queued<'_>) -> &[Self] {
        match queued.0 {
            Operation::File(value) => std::slice::from_ref(value),
            _ => &[],
        }
    }

    fn scan<'a>(
        backend: &'a dyn StorageBackend,
        snapshot: &'a BackendSnapshot,
        selection: &'a FileSelection,
        page: ScanPage,
    ) -> StorageFuture<'a, Vec<Self>> {
        backend.read_files(snapshot, selection, page)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn optional(expected: &Option<Identifier>, actual: Option<&Identifier>) -> bool {
    expected
        .as_ref()
        .is_none_or(|expected| Some(expected) == actual)
}

// Half-Open Source Windows Select Only Records Carrying A Source Time.
fn within(start: Option<SystemTime>, end: Option<SystemTime>, time: Option<SystemTime>) -> bool {
    match (start, end) {
        (Some(start), Some(end)) => time.is_some_and(|time| time >= start && time < end),
        _ => true,
    }
}

/* === Tests === */
