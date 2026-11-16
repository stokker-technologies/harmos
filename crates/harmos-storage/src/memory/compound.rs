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
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use super::Backend;
use super::failure;
use crate::StorageSchema;
use crate::{
    BackendFailure, CompoundExchange, CompoundOutcome, CompoundWrite, Identifier, MutationReceipt, MutationValue, ObjectEntry, ObjectKey,
    ObjectVersionToken, RecordView, StorageFuture,
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
                    CompoundWrite::Record(record) => {
                        staged_records.push(RecordView::new(record.clone())?);
                    }
                }
            }
            let receipt = exchange.receipt(outcome);
            exchange.replay(&receipt)?;
            if matches!(&receipt.value, MutationValue::Compound(value) if matches!(value.outcome, CompoundOutcome::Applied { .. })) {
                objects.extend(staged_objects);
                deleted.extend(staged_deleted);
                schemas.extend(staged_schemas);
                for view in staged_records {
                    records.insert_view(view);
                }
            }
            conditional.compound_ranges.insert(first, exchange.position.sequence);
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
            CompoundWrite::Record(_) => continue,
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
