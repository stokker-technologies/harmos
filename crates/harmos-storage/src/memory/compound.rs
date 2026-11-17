/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Commit Compound Mutations Atomically

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use super::Backend;
use super::failure;
use crate::StorageSchema;
use crate::{
    BackendFailure, CompoundExchange, CompoundOutcome, CompoundWrite, Error, Identifier, MutationReceipt, MutationValue, ObjectEntry,
    ObjectKey, ObjectVersionToken, RecordView, StorageFuture,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Backend {
    pub(super) fn commit_compound<'a>(&'a self, exchange: &'a CompoundExchange) -> StorageFuture<'a, MutationReceipt> {
        Box::pin(async move {
            exchange.validate()?;
            self.history.check(&exchange.position.history)?;
            self.control.write(exchange.items.iter().map(|item| item.position().sequence)).await?;
            let mut ledger = self.mutation_receipts.lock().unwrap();
            if let Some(prior) = ledger.get(&exchange.mutation().mutation) {
                exchange.replay(prior)?;
                return Ok(prior.clone());
            }
            let mut conditional = self.conditional.lock().unwrap();
            let mut objects = self.objects.lock().unwrap();
            let mut deleted = self.deleted_objects.lock().unwrap();
            let mut schemas = self.schemas.lock().unwrap();
            let mut records = self.records.lock().unwrap();
            let first = match &exchange.items[0] {
                CompoundWrite::Object { schema, .. } => schema.position.sequence,
                item => item.position().sequence,
            };
            if first <= self.saved.load(Ordering::SeqCst) as u64 {
                return Err(failure(BackendFailure::Conflict));
            }

            // Stage Every Change Before Publishing Any of Them.
            let mut versions = current(&objects, &deleted, &exchange.items);
            let mut checked: Vec<&StorageSchema> = Vec::new();
            let mut staged_objects = BTreeMap::new();
            let mut staged_deleted = BTreeMap::new();
            let mut staged_schemas = BTreeMap::new();
            let mut staged_records = Vec::new();
            let mut deleted_scope = None;
            let mut outcome = CompoundOutcome::Applied {
                positions: exchange.items.iter().map(|item| item.position().clone()).collect(),
            };
            for (index, item) in exchange.items.iter().enumerate() {
                match item {
                    CompoundWrite::Object { expected, schema, value } => {
                        self.catalog.validate_value(&value.value, NonZeroUsize::MAX)?;
                        let current = versions
                            .get_mut(&value.value.schema.id)
                            .and_then(|items| items.get_mut(&value.value.item))
                            .expect("prepared object key");
                        if current.as_ref() != expected.as_ref() {
                            outcome = CompoundOutcome::Conflict { index, current: current.clone() };
                            break;
                        }
                        let repeated = checked.iter().find(|prior| prior.key == schema.key);
                        let differs = repeated.map_or_else(
                            || schemas.values().any(|prior| prior.key == schema.key && prior.schema != schema.schema),
                            |prior| prior.schema != schema.schema,
                        );
                        if differs {
                            return Err(failure(BackendFailure::Conflict));
                        }
                        if repeated.is_none() {
                            checked.push(schema);
                        }
                        *current = Some(value.version.clone());
                        staged_schemas.insert(schema.position.sequence, schema.clone());
                        staged_objects.insert(value.position.sequence, value.clone());
                    }
                    CompoundWrite::Delete { position, schema, item, expected } => {
                        let current = versions.get_mut(schema).and_then(|items| items.get_mut(item)).expect("prepared object key");
                        if expected.is_some() && current.as_ref() != expected.as_ref() {
                            outcome = CompoundOutcome::Conflict { index, current: current.clone() };
                            break;
                        }
                        *current = None;
                        staged_deleted.insert(position.sequence, (schema.clone(), item.clone()));
                    }
                    CompoundWrite::Scope { scope, .. } => {
                        deleted_scope = Some(scope);
                    }
                    CompoundWrite::Record(record) => {
                        staged_records.push(RecordView::new(record.clone())?);
                    }
                }
            }
            let receipt = exchange.receipt(outcome);
            exchange.replay(&receipt)?;
            if matches!(&receipt.value, MutationValue::Compound(value) if matches!(value.outcome, CompoundOutcome::Applied { .. })) {
                if let Some(scope) = deleted_scope {
                    let _files = self.file_io.lock().unwrap();
                    let staged = self.staged.lock().unwrap();
                    if staged.values().any(|(metadata, _)| {
                        metadata.scope.as_ref() == Some(scope) && !self.abandoned_files.lock().unwrap().contains(metadata.id.as_str())
                    }) {
                        return Err(Error::InvalidInput {
                            field: "scope deletion",
                            reason: "finish or abandon active file productions before deleting their scope",
                        });
                    }
                    drop(staged);
                    self.retained_snapshots.lock().unwrap().retain(|_, retained| retained.owner.as_ref() != Some(scope));
                    conditional.deleted_scopes.insert(scope.clone(), exchange.position.sequence);
                    // Remove Every Revision of Identities Currently Owned by the Scope.
                    let mut seen = BTreeSet::new();
                    let identities = objects
                        .values()
                        .rev()
                        .filter_map(|object| {
                            let key = (object.value.schema.id.clone(), object.value.item.clone());
                            (seen.insert(key.clone()) && object.value.scope.as_ref() == Some(scope)).then_some(key)
                        })
                        .collect::<BTreeSet<_>>();
                    objects.retain(|_, object| {
                        object.value.scope.as_ref() != Some(scope)
                            && !identities.contains(&(object.value.schema.id.clone(), object.value.item.clone()))
                    });
                    records.delete_scope(scope);
                    self.rows.lock().unwrap().retain(|_, row| row.record.scope.as_ref() != Some(scope))?;
                    self.files.lock().unwrap().retain(|_, file| file.metadata.scope.as_ref() != Some(scope));
                    self.blobs.lock().unwrap().retain(|_, (file, _)| file.metadata.scope.as_ref() != Some(scope));
                    self.saved_productions.lock().unwrap().retain(|_, (checkpoint, _)| checkpoint.metadata.scope.as_ref() != Some(scope));
                    self.staged.lock().unwrap().retain(|_, (metadata, _)| metadata.scope.as_ref() != Some(scope));
                    self.scoped_floor.fetch_max(exchange.position.sequence as usize, Ordering::SeqCst);
                    self.reclaim_scoped_projections();
                }
                objects.extend(staged_objects);
                deleted.extend(staged_deleted);
                if exchange.items.iter().any(|item| matches!(item, CompoundWrite::Delete { .. } | CompoundWrite::Scope { .. })) {
                    conditional.invalidations.insert(exchange.position.sequence);
                }
                schemas.extend(staged_schemas);
                for view in staged_records {
                    records.insert_view(view);
                }
            }
            conditional.mutation_ranges.insert(first, exchange.position.sequence);
            ledger.insert(receipt.mutation.clone(), receipt.clone());
            self.saved.fetch_max(exchange.position.sequence as usize, Ordering::SeqCst);
            if self
                .control
                .compound_lost_replies
                .try_update(Ordering::AcqRel, Ordering::Acquire, |remaining| remaining.checked_sub(1))
                .is_ok()
            {
                return Err(failure(BackendFailure::Transient));
            }
            Ok(receipt)
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn current(
    objects: &BTreeMap<u64, ObjectEntry>, deleted: &BTreeMap<u64, (Identifier, ObjectKey)>, items: &[CompoundWrite],
) -> BTreeMap<Identifier, BTreeMap<ObjectKey, Option<ObjectVersionToken>>> {
    let mut versions: BTreeMap<Identifier, BTreeMap<ObjectKey, Option<ObjectVersionToken>>> = BTreeMap::new();
    for item in items {
        let (schema, item) = match item {
            CompoundWrite::Object { value, .. } => (&value.value.schema.id, &value.value.item),
            CompoundWrite::Delete { schema, item, .. } => (schema, item),
            CompoundWrite::Record(_) | CompoundWrite::Scope { .. } => continue,
        };
        versions.entry(schema.clone()).or_default().entry(item.clone()).or_insert(None);
    }
    if versions.is_empty() {
        return versions;
    }

    // Resolve Requested Keys in One Reverse Object Scan.
    let mut remaining = versions.values().map(BTreeMap::len).sum::<usize>();
    for object in objects.values().rev() {
        if remaining == 0 {
            break;
        }
        if let Some(current) = versions.get_mut(&object.value.schema.id).and_then(|items| items.get_mut(&object.value.item))
            && current.is_none()
        {
            *current = Some(object.version.clone());
            remaining -= 1;
        }
    }

    // Older Tombstones Cannot Hide Any Selected Version.
    let Some(oldest) =
        versions.values().flat_map(BTreeMap::values).filter_map(Option::as_ref).map(|version| version.revision().get()).min()
    else {
        return versions;
    };
    for (position, (schema, item)) in deleted.iter().rev() {
        if *position <= oldest {
            break;
        }
        if let Some(current) = versions.get_mut(schema).and_then(|items| items.get_mut(item))
            && current.as_ref().is_some_and(|version| *position > version.revision().get())
        {
            *current = None;
        }
    }
    versions
}

/* === Tests === */
