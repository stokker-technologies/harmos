/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Reference Backend Maintenance

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::any::Any;
use std::io;
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use super::{Backend, Conditional};
use crate::{BackendFailure, BackendSnapshot, Error, MutationValue, Result, StorageBackend, StorageFuture};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Backend {
    pub(super) fn copy_image<'a>(&'a self, snapshot: &'a BackendSnapshot, target: &'a dyn StorageBackend) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.maintenance_supported()?;
            let target = (target as &dyn Any).downcast_ref::<Self>().ok_or_else(|| {
                Error::backend(
                    "copy memory backend; supply an empty memory target",
                    BackendFailure::Unsupported,
                    io::Error::other("target backend kind is unsupported"),
                )
            })?;
            let image = self.image(snapshot)?;
            if image.scope.is_some() {
                return Err(Error::InvalidInput {
                    field: "copy source",
                    reason: "select a global snapshot to copy a complete backend",
                });
            }
            self.history.check(&target.history)?;
            let mut target_receipts = target.mutation_receipts.lock().unwrap();
            let mut target_conditional = target.conditional.lock().unwrap();
            target.check_offline()?;
            let rows_empty = target.records.lock().unwrap().is_empty() && target.rows.lock().unwrap().is_empty();
            let objects_empty = target.objects.lock().unwrap().is_empty()
                && target.schemas.lock().unwrap().is_empty()
                && target.object_schemas.lock().unwrap().is_empty();
            let files_empty = target.staged.lock().unwrap().is_empty()
                && target.files.lock().unwrap().is_empty()
                && target.blobs.lock().unwrap().is_empty()
                && target.saved_productions.lock().unwrap().is_empty();
            let definitions_empty = target.projections.lock().unwrap().is_empty() && target.indexes.lock().unwrap().is_empty();
            let target_empty = target.saved.load(Ordering::SeqCst) == 0
                && rows_empty
                && objects_empty
                && files_empty
                && definitions_empty
                && target_receipts.is_empty()
                && target.retained_snapshots.lock().unwrap().is_empty();
            if !target_empty {
                return Err(Error::InvalidInput {
                    field: "copy target",
                    reason: "supply an empty offline backend",
                });
            }
            let through = image.at.sequence;
            *target.pruned_objects.lock().unwrap() =
                image.pruned_objects.range(..=through).map(|(key, value)| (*key, value.clone())).collect();
            *target.blobs.lock().unwrap() = image
                .blobs
                .iter()
                .filter(|(_, (file, _))| file.position.sequence <= through)
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            *target.object_schemas.lock().unwrap() = image.object_schemas.clone();
            *target.objects.lock().unwrap() = image.objects.range(..=through).map(|(key, value)| (*key, value.clone())).collect();
            *target.deleted_objects.lock().unwrap() =
                image.deleted_objects.range(..=through).map(|(key, value)| (*key, value.clone())).collect();
            *target.files.lock().unwrap() = image.files.range(..=through).map(|(key, value)| (*key, value.clone())).collect();
            *target.schemas.lock().unwrap() = image.schemas.range(..=through).map(|(key, value)| (*key, value.clone())).collect();
            let records = image
                .records
                .iter()
                .filter(|(sequence, _)| **sequence <= through)
                .map(|(sequence, record)| (*sequence, record.clone()))
                .collect();
            *target.records.lock().unwrap() = records;
            *target.rows.lock().unwrap() = image.rows.range(..=through).map(|(key, value)| (*key, value.clone())).collect();
            *target.projections.lock().unwrap() = image.projections.clone();
            *target.indexes.lock().unwrap() = image.indexes.clone();
            *target_receipts = image
                .receipts
                .iter()
                .filter(|(_, receipt)| match &receipt.value {
                    MutationValue::Cursor(position) | MutationValue::Schema { position, .. } => position.sequence <= through,
                    MutationValue::Compound(receipt) => receipt.position.sequence <= through,
                    MutationValue::Reference(file) => file.position.sequence <= through,
                    MutationValue::Definition { .. } | MutationValue::Rejected => true,
                    _ => false,
                })
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            let mut conditional = image.conditional.clone();
            conditional.floor = conditional.floor.min(through);
            conditional.compound_ranges.retain(|_, end| *end <= through);
            conditional.receipts.retain(|sequence, _| *sequence <= through);
            conditional.invalidations.retain(|sequence| *sequence <= through);
            conditional.deleted_scopes.retain(|_, sequence| *sequence <= through);
            *target_conditional = conditional;
            *target.retained_snapshots.lock().unwrap() = self
                .retained_snapshots
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, retained)| retained.image.at.sequence <= through)
                .map(|(token, retained)| (token.clone(), retained.clone()))
                .collect();
            target.scoped_floor.store(image.lost_objects_through as usize, Ordering::SeqCst);
            target.saved.store(through as usize, Ordering::SeqCst);
            Ok(())
        })
    }

    pub(super) fn dispose_offline(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            self.maintenance_supported()?;
            let mut receipts = self.mutation_receipts.lock().unwrap();
            let mut conditional = self.conditional.lock().unwrap();
            self.check_offline()?;
            self.objects.lock().unwrap().clear();
            self.deleted_objects.lock().unwrap().clear();
            self.object_schemas.lock().unwrap().clear();
            self.pruned_objects.lock().unwrap().clear();
            self.object_releases.lock().unwrap().clear();
            self.files.lock().unwrap().clear();
            self.blobs.lock().unwrap().clear();
            self.staged.lock().unwrap().clear();
            self.saved_productions.lock().unwrap().clear();
            self.abandoned_files.lock().unwrap().clear();
            self.resumable_files.lock().unwrap().clear();
            self.file_generations.lock().unwrap().clear();
            self.deleted_files.lock().unwrap().clear();
            *self.records.lock().unwrap() = Default::default();
            self.rows.lock().unwrap().clear();
            self.schemas.lock().unwrap().clear();
            *self.projections.lock().unwrap() = Default::default();
            *self.indexes.lock().unwrap() = Default::default();
            receipts.clear();
            self.retained_snapshots.lock().unwrap().clear();
            *conditional = Conditional::default();
            self.scoped_floor.store(0, Ordering::SeqCst);
            self.lost_files_through.store(0, Ordering::SeqCst);
            self.saved.store(0, Ordering::SeqCst);
            Ok(())
        })
    }

    fn maintenance_supported(&self) -> Result<()> {
        if self.control.unsupported_maintenance.load(Ordering::SeqCst) {
            return Err(Error::backend(
                "maintain backend; register a backend with maintenance hooks",
                BackendFailure::Unsupported,
                io::Error::other("backend maintenance is unsupported"),
            ));
        }
        Ok(())
    }

    fn check_offline(&self) -> Result<()> {
        if self.control.open.load(Ordering::SeqCst) || !self.snapshots.lock().unwrap().is_empty() {
            return Err(Error::InvalidInput {
                field: "backend maintenance",
                reason: "close all sessions and release all reader leases first",
            });
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
