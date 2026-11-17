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
use std::any::type_name;

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

impl Backend {
    pub(super) fn retain_image<'a>(
        &'a self, snapshot: &'a BackendSnapshot, scope: Option<&'a Identifier>,
    ) -> StorageFuture<'a, BackendSnapshot> {
        Box::pin(async move {
            let transaction = self.conditional.lock().unwrap();
            if scope.is_some_and(|scope| transaction.deleted_scopes.get(scope).is_some_and(|deleted| *deleted > snapshot.at.sequence)) {
                return Err(Error::InvalidInput {
                    field: "retained scope",
                    reason: "scope was deleted; select a fresh scope and cut",
                });
            }
            let mut image = self.image(snapshot)?.as_ref().clone();
            if scope.is_some_and(|scope| image.scope.as_ref().is_some_and(|selected| selected != scope)) {
                return Err(Error::InvalidInput {
                    field: "retained scope",
                    reason: "retain the original scope or select a global image",
                });
            }
            // Copy Only the Selected Cut and Scope into Durable Ownership.
            let at = snapshot.at.sequence;
            if let Some(scope) = scope {
                image.scope = Some(scope.clone());
            }
            let mut owners = BTreeMap::new();
            for object in image.objects.range(..=at).map(|(_, object)| object) {
                owners.insert((object.value.schema.id.clone(), object.value.item.clone()), object.value.scope.clone());
            }
            image.objects.retain(|position, object| {
                let identity = (object.value.schema.id.clone(), object.value.item.clone());
                let selected = scope.is_none_or(|scope| {
                    object.value.scope.as_ref() == Some(scope) && owners.get(&identity).is_some_and(|owner| owner.as_ref() == Some(scope))
                });
                *position <= at && selected
            });
            let identities =
                image.objects.values().map(|object| (object.value.schema.id.clone(), object.value.item.clone())).collect::<BTreeSet<_>>();
            image.deleted_objects.retain(|position, identity| *position <= at && (scope.is_none() || identities.contains(identity)));
            image.pruned_objects.retain(|position, (schema, item)| {
                *position <= at && (scope.is_none() || identities.contains(&(schema.id.clone(), item.clone())))
            });
            image.records = image
                .records
                .iter()
                .filter(|(position, record)| **position <= at && scope.is_none_or(|scope| record.scope.as_ref() == Some(scope)))
                .map(|(position, record)| (*position, record.clone()))
                .collect();
            image.rows.retain(|position, row| *position <= at && scope.is_none_or(|scope| row.record.scope.as_ref() == Some(scope)));
            image.files.retain(|position, file| *position <= at && scope.is_none_or(|scope| file.metadata.scope.as_ref() == Some(scope)));
            image.blobs.retain(|_, (file, _)| {
                file.position.sequence <= at && scope.is_none_or(|scope| file.metadata.scope.as_ref() == Some(scope))
            });
            image.schemas.retain(|position, _| *position <= at);
            // Global Projection Outputs Can Contain Rows from Unrelated Scopes.
            if scope.is_some() {
                image.projections = Default::default();
            }
            let token = format!("retained:{}:{}", self.snapshot_scope, self.next_snapshot.fetch_add(1, Ordering::SeqCst)).into_bytes();
            let retained = BackendSnapshot { at: snapshot.at.clone(), token };
            self.retained_snapshots.lock().unwrap().insert(
                retained.token.clone(),
                RetainedImage {
                    owner: scope.cloned(),
                    image: Arc::new(image),
                },
            );
            Ok(retained)
        })
    }

    pub(super) fn open_retained_image<'a>(&'a self, retained: &'a BackendSnapshot) -> StorageFuture<'a, BackendSnapshot> {
        Box::pin(async move {
            let _transaction = self.conditional.lock().unwrap();
            let image = self
                .retained_snapshots
                .lock()
                .unwrap()
                .get(&retained.token)
                .filter(|retained_image| retained_image.image.at == retained.at)
                .map(|retained_image| retained_image.image.clone())
                .ok_or(Error::InvalidInput {
                    field: "retained snapshot",
                    reason: "ownership was released; list a fresh image",
                })?;
            Ok(self.lease_image(image))
        })
    }

    pub(super) fn release_retained_image<'a>(&'a self, retained: &'a BackendSnapshot) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.history.check(&retained.at.history)?;
            let _transaction = self.conditional.lock().unwrap();
            let mut retained_snapshots = self.retained_snapshots.lock().unwrap();
            if retained_snapshots.get(&retained.token).is_some_and(|retained_image| retained_image.image.at != retained.at) {
                return Err(Error::InvalidInput {
                    field: "retained snapshot",
                    reason: "use the original retained handle",
                });
            }
            retained_snapshots.remove(&retained.token);
            drop(retained_snapshots);
            self.reclaim_scoped_projections();
            Ok(())
        })
    }

    fn lease_image(&self, image: Arc<Snapshot>) -> BackendSnapshot {
        let token =
            format!("{}:{}:{}", type_name::<Self>(), self.snapshot_scope, self.next_snapshot.fetch_add(1, Ordering::SeqCst)).into_bytes();
        let snapshot = BackendSnapshot { at: image.at.clone(), token };
        self.snapshots.lock().unwrap().insert(snapshot.token.clone(), image);
        snapshot
    }

    pub fn retained_file_bytes(&self) -> usize {
        let _transaction = self.conditional.lock().unwrap();
        let current = self.blobs.lock().unwrap().values().map(|(_, bytes)| bytes.len()).sum::<usize>();
        let mut images = BTreeSet::new();
        let snapshots = self.snapshots.lock().unwrap();
        let retained_snapshots = self.retained_snapshots.lock().unwrap();
        let retained = snapshots
            .values()
            .chain(retained_snapshots.values().map(|retained| &retained.image))
            .filter(|snapshot| images.insert(Arc::as_ptr(snapshot)))
            .flat_map(|snapshot| snapshot.blobs.values())
            .map(|(_, bytes)| bytes.len())
            .sum::<usize>();
        current.saturating_add(retained)
    }

    pub(super) fn reclaim_scoped_projections(&self) {
        let floor = self.scoped_floor.load(Ordering::SeqCst) as u64;
        if floor > 0
            && self.snapshots.lock().unwrap().values().all(|snapshot| snapshot.scope.is_some() || snapshot.at.sequence >= floor)
            && self
                .retained_snapshots
                .lock()
                .unwrap()
                .values()
                .all(|retained| retained.image.scope.is_some() || retained.image.at.sequence >= floor)
        {
            self.projections.lock().unwrap().reclaim_through(floor - 1);
        }
    }

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

    pub(super) fn open_snapshot<'a>(&'a self, at: &'a StorageCursor) -> StorageFuture<'a, BackendSnapshot> {
        Box::pin(async move {
            if self.control.block_view.load(Ordering::SeqCst) {
                self.control.view_entered.notify_one();
                self.control.view_release.notified().await;
            }
            self.history.check(&at.history)?;
            let receipts = self.mutation_receipts.lock().unwrap();
            let conditional = self.conditional.lock().unwrap();

            if at.sequence > self.saved.load(Ordering::SeqCst) as u64 {
                return Err(Error::NotReady { domain: "requested storage cut" });
            }
            let lost_objects_through =
                (self.control.lost_objects_through.load(Ordering::SeqCst) as u64).max(self.scoped_floor.load(Ordering::SeqCst) as u64);
            if at.sequence < self.scoped_floor.load(Ordering::SeqCst) as u64 {
                let retained = self.snapshots.lock().unwrap().values().find(|image| image.at == *at && image.scope.is_none()).cloned();
                if let Some(image) = retained {
                    return Ok(self.lease_image(image));
                }
            }
            // Pruned Revisions No Longer Reconstruct An Older Image.
            if at.sequence < lost_objects_through {
                return Err(Error::Gap {
                    requested: at.sequence,
                    available: lost_objects_through,
                });
            }
            if conditional.compound_ranges.range(..=at.sequence).next_back().is_some_and(|(_, end)| at.sequence < *end) {
                return Err(Error::InvalidInput {
                    field: "storage cut",
                    reason: "select a complete compound receipt frontier",
                });
            }
            let records = self.records.lock().unwrap();
            let retained = records.clone();
            let copied = if records.shares(&retained) { 0 } else { records.len() };
            self.control.snapshot_rows_copied.fetch_add(copied, Ordering::Relaxed);
            drop(records);
            let snapshot = Snapshot {
                at: at.clone(),
                scope: None,
                pruned_objects: self.pruned_objects.lock().unwrap().clone(),
                blobs: self.blobs.lock().unwrap().clone(),
                lost_objects_through,
                objects: self.objects.lock().unwrap().clone(),
                deleted_objects: self.deleted_objects.lock().unwrap().clone(),
                files: self.files.lock().unwrap().clone(),
                schemas: self.schemas.lock().unwrap().clone(),
                records: retained,
                rows: self.rows.lock().unwrap().clone(),
                conditional: conditional.clone(),
                projections: self.projections.lock().unwrap().at(at.sequence),
                indexes: self.indexes.lock().unwrap().clone(),
                receipts: receipts.clone(),
                object_schemas: self.object_schemas.lock().unwrap().clone(),
            };
            let snapshot = self.lease_image(Arc::new(snapshot));
            drop(conditional);
            Ok(snapshot)
        })
    }

    pub(super) fn release_snapshot<'a>(&'a self, snapshot: &'a BackendSnapshot) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.control.releases.fetch_add(1, Ordering::SeqCst);
            if self.control.block_release.load(Ordering::SeqCst) {
                self.control.release_entered.notify_one();
                self.control.release_release.notified().await;
            }
            if self.control.fail_release.load(Ordering::SeqCst) {
                return Err(failure(BackendFailure::Permanent));
            }
            let _transaction = self.conditional.lock().unwrap();
            self.snapshots.lock().unwrap().remove(&snapshot.token);
            self.reclaim_scoped_projections();
            self.control.releases_completed.fetch_add(1, Ordering::SeqCst);
            self.control.release_completed.notify_waiters();
            Ok(())
        })
    }

    pub(super) fn read_schemas<'a>(
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

    pub(super) fn read_objects<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a ObjectSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<ObjectEntry>> {
        Box::pin(async move {
            self.control.gate_read().await;
            let image = self.image(snapshot)?;
            // Only Pruning Within The Selected History Creates A Gap.
            let pruned = image.pruned_objects.iter().any(|(position, (schema, item))| {
                *position > page.after
                    && *position <= snapshot.at.sequence
                    && selection.schema.as_ref().is_none_or(|selected| selected == schema)
                    && selection.key.as_ref().is_none_or(|selected| selected == item)
            });
            if selection.versions == ObjectVersions::Revisions && pruned {
                return Err(Error::Gap {
                    requested: page.after + 1,
                    available: image.lost_objects_through + 1,
                });
            }
            let selected = |object: &ObjectEntry| {
                selection.schema.as_ref().is_none_or(|schema| schema == &object.value.schema)
                    && selection.key.as_ref().is_none_or(|item| item == &object.value.item)
                    && !image.deleted_objects.iter().any(|(position, (schema, item))| {
                        *position <= snapshot.at.sequence
                            && *position > object.position.sequence
                            && schema == &object.value.schema.id
                            && item == &object.value.item
                    })
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

    pub(super) fn read_records<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a RecordSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRecord>> {
        Box::pin(async move {
            self.control.gate_read().await;
            let image = self.image(snapshot)?;
            let mut entries = Vec::new();
            let mut bytes = 0usize;
            for (row, size) in image.records.selected(&selection.schema, page.after, snapshot.at.sequence) {
                self.control.record_rows_examined.fetch_add(1, Ordering::Relaxed);
                if !record_matches(row, &selection.schema, &selection.scope, &selection.item)
                    || !window(selection.source_start, selection.source_end, row.source_time)
                {
                    continue;
                }
                if entries.len() == page.rows.get() || (!entries.is_empty() && bytes + size > page.bytes.get()) {
                    break;
                }
                bytes += size;
                self.control.record_rows_copied.fetch_add(1, Ordering::Relaxed);
                entries.push(row.clone());
            }
            if self.control.invalid_page.load(Ordering::SeqCst) && !entries.is_empty() {
                entries[0].item = Identifier::new("wrong-item").unwrap();
            }
            Ok(entries)
        })
    }

    pub(super) fn read_stream<'a>(
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

    pub(super) fn read_files<'a>(
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

    pub(super) fn read_object<'a>(
        &'a self, snapshot: &'a BackendSnapshot, query: &'a ObjectPoint,
    ) -> StorageFuture<'a, Option<ObjectEntry>> {
        Box::pin(async move {
            self.control.gate_read().await;
            let image = self.image(snapshot)?;
            if let ObjectVersion::Exact(version) = &query.version {
                self.history.check(version.history())?;
                let sequence = version.revision().get();
                if image.pruned_objects.get(&sequence).is_some_and(|(schema, item)| schema.id == query.schema && item == &query.item) {
                    return Err(Error::Gap {
                        requested: sequence,
                        available: image.lost_objects_through.saturating_add(1),
                    });
                }
            }
            let object = image.objects.values().rev().find(|object| {
                object.position.sequence <= snapshot.at.sequence
                    && object.value.schema.id == query.schema
                    && object.value.item == query.item
                    && !image.deleted_objects.iter().any(|(position, (schema, item))| {
                        *position <= snapshot.at.sequence
                            && *position > object.position.sequence
                            && schema == &query.schema
                            && item == &query.item
                    })
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
