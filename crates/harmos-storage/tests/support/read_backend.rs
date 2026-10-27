/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Serve Retained Snapshot Reads.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C: ObjectCatalog> Data<C> {
    #[allow(dead_code, reason = "Shared fixture also compiles in runtime stories without native snapshot reads.")]
    pub async fn snapshots_released(&self) {
        loop {
            let completed = self.control.release_completed.notified();
            if self.snapshots.lock().unwrap().is_empty() {
                return;
            }
            completed.await;
        }
    }

    pub(super) fn fixture_open_snapshot<'a>(&'a self, at: &'a StorageCursor) -> StorageFuture<'a, BackendSnapshot> {
        Box::pin(async move {
            if self.control.block_view.load(Ordering::SeqCst) {
                self.control.view_entered.notify_one();
                self.control.view_release.notified().await;
            }
            self.history.check(&at.history)?;
            let lost_objects_through = self.control.lost_objects_through.load(Ordering::SeqCst) as u64;
            // Pruned Revisions No Longer Reconstruct An Older Image.
            if at.sequence < lost_objects_through {
                return Err(Error::Gap {
                    requested: at.sequence,
                    available: lost_objects_through,
                });
            }
            let snapshot = Snapshot {
                lost_objects_through,
                objects: self.objects.lock().unwrap().clone(),
                files: self.files.lock().unwrap().clone(),
                schemas: self.schemas.lock().unwrap().clone(),
                records: self.records.lock().unwrap().clone(),
                rows: self.rows.lock().unwrap().clone(),
            };
            let token = self.next_snapshot.fetch_add(1, Ordering::SeqCst).to_le_bytes().to_vec();
            self.snapshots.lock().unwrap().insert(token.clone(), snapshot);
            Ok(BackendSnapshot { at: at.clone(), token })
        })
    }

    pub(super) fn fixture_release_snapshot<'a>(&'a self, snapshot: &'a BackendSnapshot) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.control.releases.fetch_add(1, Ordering::SeqCst);
            if self.control.block_release.load(Ordering::SeqCst) {
                self.control.release_entered.notify_one();
                self.control.release_release.notified().await;
            }
            if self.control.fail_release.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Permanent));
            }
            self.snapshots.lock().unwrap().remove(&snapshot.token);
            self.control.releases_completed.fetch_add(1, Ordering::SeqCst);
            self.control.release_completed.notify_waiters();
            Ok(())
        })
    }

    pub(super) fn fixture_read_schemas<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a SchemaSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StorageSchema>> {
        Box::pin(async move {
            let image = self.image(snapshot)?;
            scan(
                image.schemas.values(),
                snapshot,
                page,
                |row| &row.position,
                |row| selection.schema.as_ref().is_none_or(|key| key == &row.key),
            )
        })
    }

    pub(super) fn fixture_read_objects<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a ObjectSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<ObjectEntry>> {
        Box::pin(async move {
            self.control.gate_read().await;
            let image = self.image(snapshot)?;
            // Complete Revision History Cannot Begin Before Pruned Revisions.
            if selection.versions == ObjectVersions::Revisions && page.after < image.lost_objects_through {
                return Err(Error::Gap {
                    requested: page.after + 1,
                    available: image.lost_objects_through + 1,
                });
            }
            let selected = |object: &ObjectEntry| {
                selection.schema.as_ref().is_none_or(|schema| schema == &object.value.schema)
                    && selection.key.as_ref().is_none_or(|item| item == &object.value.item)
            };
            // Latest Keeps Each Selected Key's Newest Revision At The Snapshot.
            let latest = (selection.versions == ObjectVersions::Latest).then(|| {
                let mut latest = BTreeMap::new();
                for object in image.objects.values() {
                    if object.position.sequence <= snapshot.at.sequence && selected(object) {
                        latest.insert((object.value.schema.id.clone(), object.value.item.clone()), object.position.sequence);
                    }
                }
                latest
            });
            let mut entries = scan(
                image.objects.values(),
                snapshot,
                page,
                |row| &row.position,
                |object| {
                    selected(object)
                        && latest.as_ref().is_none_or(|latest| {
                            latest.get(&(object.value.schema.id.clone(), object.value.item.clone())) == Some(&object.position.sequence)
                        })
                },
            )?;
            if self.control.invalid_page.load(Ordering::SeqCst) && !entries.is_empty() {
                entries[0].value.payload = vec![0xff];
            }
            Ok(entries)
        })
    }

    pub(super) fn fixture_read_records<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a RecordSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRecord>> {
        Box::pin(async move {
            self.control.gate_read().await;
            let image = self.image(snapshot)?;
            let mut entries = scan(
                image.records.values(),
                snapshot,
                page,
                |row| &row.position,
                |row| {
                    record_matches(row, &selection.schema, &selection.scope, &selection.item)
                        && window(selection.source_start, selection.source_end, row.source_time)
                },
            )?;
            if self.control.invalid_page.load(Ordering::SeqCst) && !entries.is_empty() {
                entries[0].item = Identifier::new("wrong-item").unwrap();
            }
            Ok(entries)
        })
    }

    pub(super) fn fixture_read_stream<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a StreamSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRow>> {
        Box::pin(async move {
            self.control.gate_read().await;
            let image = self.image(snapshot)?;
            scan(
                image.rows.values(),
                snapshot,
                page,
                |row| &row.record.position,
                |row| {
                    record_matches(&row.record, &selection.schema, &selection.scope, &selection.item)
                        && selection.stream.as_ref().is_none_or(|id| id == &row.stream)
                        && window(selection.source_start, selection.source_end, row.record.source_time)
                },
            )
        })
    }

    pub(super) fn fixture_read_files<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a FileSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<FileReference>> {
        Box::pin(async move {
            let image = self.image(snapshot)?;
            scan(
                image.files.values(),
                snapshot,
                page,
                |file| &file.position,
                |file| {
                    selection.item.as_ref().is_none_or(|id| id == &file.metadata.id)
                        && selection.scope.as_ref().is_none_or(|scope| Some(scope) == file.metadata.scope.as_ref())
                },
            )
        })
    }

    pub(super) fn fixture_read_object<'a>(
        &'a self, snapshot: &'a BackendSnapshot, query: &'a ObjectPoint,
    ) -> StorageFuture<'a, Option<ObjectEntry>> {
        Box::pin(async move {
            self.control.gate_read().await;
            let image = self.image(snapshot)?;
            let object = image.objects.values().rev().find(|object| {
                object.position.sequence <= snapshot.at.sequence
                    && object.value.schema.id == query.schema
                    && object.value.item == query.item
                    && match &query.version {
                        ObjectVersion::Latest => true,
                        ObjectVersion::Exact(version) => version == &object.version,
                    }
            });
            let mut object = object.cloned();
            if self.control.invalid_page.load(Ordering::SeqCst)
                && let Some(object) = &mut object
            {
                object.value.payload = vec![0xff];
            }
            Ok(object)
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
