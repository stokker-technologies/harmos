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
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

// Third Party Imports
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

// Local Imports
use super::{FileProducer, MutationId, MutationWindow, StorageManager, StoreHistory};
use crate::{Error, Identifier, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationReceipt {
    pub mutation: MutationId,
    pub fingerprint: [u8; 32],
    pub expires_at: SystemTime,
    pub value: MutationValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationHeader {
    pub mutation: MutationId,
    pub fingerprint: [u8; 32],
    pub expires_at: SystemTime,
}

pub(super) struct Receipts {
    maximum: NonZeroUsize,
    retention: Duration,
    execution: crate::execution::Execution,
    window: Identifier,
    deadline: Instant,
    expires_at: SystemTime,
    entries: BTreeMap<MutationId, Entry>,
    // Admission Order, Oldest First, For Expiry And Eviction.
    order: VecDeque<MutationId>,
    unsaved: BTreeSet<MutationId>,
}

struct Entry {
    fingerprint: [u8; 32],
    deadline: Instant,
    expires_at: SystemTime,
    value: Option<MutationValue>,

    completion: Arc<()>,
}

pub(super) struct Completion(Arc<()>);

/* === Enums === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum MutationValue {
    Cursor(super::StorageCursor),
    Schema { position: super::StorageCursor, key: super::SchemaKey, outcome: super::DefinitionOutcome },
    Definition { kind: super::DefinitionKind, name: Identifier, outcome: super::DefinitionOutcome },
    Deleted(super::StagedFile),
    Producer(FileProducer),
    Reference(Box<super::FileReference>),
    Progress(super::FileProgress),
    Rejected,
}

pub(super) enum ReceiptState {
    Missing,
    Pending,
    Complete(MutationValue),
}

/* === Traits === */

/* === Implementations === */

impl Entry {
    fn reclaimable(&self) -> bool {
        self.value.is_some() && Arc::strong_count(&self.completion) == 1
    }
}

impl Receipts {
    pub(super) fn new(retention: Duration, maximum: NonZeroUsize, execution: crate::execution::Execution) -> Result<Self> {
        let reading = execution.read();
        let now = reading.instant;
        let wall = reading.date;
        Ok(Self {
            maximum,
            retention,
            execution,
            window: epoch()?,
            deadline: now + retention,
            expires_at: wall + retention,
            entries: BTreeMap::new(),
            order: VecDeque::new(),
            unsaved: BTreeSet::new(),
        })
    }

    pub(super) fn issued_position(&self, position: &super::StorageCursor) -> bool {
        let now = self.execution.now();
        self.entries.values().any(|entry| {
            entry.deadline > now
                && match entry.value.as_ref() {
                    Some(MutationValue::Cursor(cursor) | MutationValue::Schema { position: cursor, .. }) => cursor == position,
                    Some(MutationValue::Reference(file)) => &file.position == position,
                    _ => false,
                }
        })
    }

    pub(super) fn recovery_limits(&self) -> super::ReadLimits {
        super::ReadLimits {
            rows: self.maximum,
            bytes: NonZeroUsize::new(super::config::MAX_BATCH_BYTES).unwrap(),
        }
    }

    pub(super) fn window(&mut self, history: &StoreHistory, retention: Duration) -> Result<MutationWindow> {
        let reading = self.execution.read();
        let now = reading.instant;
        self.purge(now);
        if now >= self.deadline {
            self.window = epoch()?;
            self.deadline = now + retention;
            self.expires_at = reading.date.checked_add(retention).ok_or_else(expired)?;
        }
        Ok(MutationWindow {
            history: history.clone(),
            id: self.window.clone(),
            expires_at: self.expires_at,
        })
    }

    pub(super) fn lookup(&self, id: &MutationId, fingerprint: &[u8; 32], completion: Option<&Completion>) -> Result<ReceiptState> {
        let now = self.execution.now();
        if let Some(entry) = self.entries.get(id) {
            let admitted = completion.is_some_and(|completion| Arc::ptr_eq(&completion.0, &entry.completion));
            if !admitted && now >= entry.deadline {
                tracing::error!(window = %id.window, mutation = %id.key, reason = "retry window expired", "storage mutation rejected");
                return Err(expired());
            }
            if entry.fingerprint != *fingerprint {
                tracing::error!(window = %id.window, mutation = %id.key, reason = "identity reused with different request", "storage mutation rejected");
                return Err(Error::backend(
                    "replay storage mutation",
                    crate::BackendFailure::Conflict,
                    std::io::Error::other("request identifier reused with different normalized payload"),
                ));
            }
            return Ok(match &entry.value {
                Some(value) => ReceiptState::Complete(value.clone()),
                None => ReceiptState::Pending,
            });
        }
        if id.window != self.window || now >= self.deadline {
            tracing::error!(window = %id.window, mutation = %id.key, reason = "retry window expired or unknown", "storage mutation rejected");
            return Err(expired());
        }
        Ok(ReceiptState::Missing)
    }

    pub(super) fn completion(&self, id: &MutationId) -> Completion {
        Completion(self.entries.get(id).expect("admitted receipt").completion.clone())
    }

    pub(super) fn insert(&mut self, id: &MutationId, fingerprint: [u8; 32]) -> Result<()> {
        self.purge(self.execution.now());
        if !matches!(self.lookup(id, &fingerprint, None)?, ReceiptState::Missing) {
            return Err(Error::AlreadyRegistered {
                domain: "storage mutation",
                name: id.key.to_string(),
            });
        }
        while self.entries.len() >= self.maximum.get() {
            self.evict_oldest()?;
        }
        self.order.push_back(id.clone());
        self.entries.insert(
            id.clone(),
            Entry {
                fingerprint,
                deadline: self.deadline,
                expires_at: self.expires_at,
                value: None,
                completion: Arc::new(()),
            },
        );

        Ok(())
    }

    pub(super) fn header(&self, id: &MutationId) -> MutationHeader {
        let entry = self.entries.get(id).expect("admitted receipt");
        MutationHeader {
            mutation: id.clone(),
            fingerprint: entry.fingerprint,
            expires_at: entry.expires_at,
        }
    }

    pub(super) fn prepared(&self, id: &MutationId, value: MutationValue) -> MutationReceipt {
        let entry = self.entries.get(id).expect("admitted receipt");
        MutationReceipt {
            mutation: id.clone(),
            fingerprint: entry.fingerprint,
            expires_at: entry.expires_at,
            value,
        }
    }

    pub(super) fn complete(&mut self, id: &MutationId, value: MutationValue) {
        let entry = self.entries.get_mut(id).expect("pending receipt retained through completion");
        if entry.value.is_some() {
            return;
        }
        entry.value = Some(value);
        self.unsaved.insert(id.clone());
    }

    pub(super) fn pending_positions(&self, through: &super::StorageCursor) -> Vec<MutationReceipt> {
        self.snapshot(Some(through)).into_iter().filter(|receipt| matches!(receipt.value, MutationValue::Cursor(_))).collect()
    }
    fn position_saved(&self, id: &MutationId, position: &super::StorageCursor) -> Result<bool> {
        let Some(entry) = self.entries.get(id) else { return Err(expired()) };
        if self.execution.now() >= entry.deadline {
            return Err(expired());
        }
        if !matches!(&entry.value, Some(MutationValue::Cursor(saved)) if saved == position) {
            return Err(Error::InvalidInput {
                field: "Append receipt",
                reason: "mutation owns a different accepted position",
            });
        }
        Ok(!self.unsaved.contains(id))
    }
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn clear(&mut self) {
        self.entries = BTreeMap::new();
        self.order = VecDeque::new();
        self.unsaved.clear();
    }

    // Drop The Oldest Completed Receipt; Pending Receipts Still Await Their Outcome.
    fn evict_oldest(&mut self) -> Result<()> {
        let index = self
            .order
            .iter()
            .position(|id| {
                self.entries.get(id).is_some_and(|entry| {
                    entry.reclaimable() && !(self.unsaved.contains(id) && matches!(entry.value, Some(MutationValue::Cursor(_))))
                })
            })
            .ok_or_else(exhausted)?;
        // Retained IDs Still Replay; An Evicted ID Must Never Become Fresh Work.
        let next_window = epoch()?;
        let id = self.order.remove(index).expect("ordered receipt");
        self.window = next_window;
        self.entries.remove(&id).expect("ordered receipt");
        self.unsaved.remove(&id);

        Ok(())
    }

    pub(super) fn snapshot(&self, through: Option<&super::StorageCursor>) -> Vec<MutationReceipt> {
        let now = self.execution.now();
        let count = self.unsaved.len();
        let mut receipts = Vec::with_capacity(count);
        for mutation in &self.unsaved {
            let Some(entry) = self.entries.get(mutation) else { continue };
            if let Some(value) = &entry.value
                && now < entry.deadline
                && match value {
                    MutationValue::Reference(reference) => {
                        through.is_none_or(|through| through.covers(&reference.position).unwrap_or(false))
                    }
                    MutationValue::Cursor(cursor) => through.is_none_or(|through| through.covers(cursor).unwrap_or(false)),
                    _ => true,
                }
            {
                receipts.push(MutationReceipt {
                    mutation: mutation.clone(),
                    fingerprint: entry.fingerprint,
                    expires_at: entry.expires_at,
                    value: value.clone(),
                });
            }
        }
        receipts
    }

    pub(super) fn saved(&mut self, receipts: &[MutationReceipt]) {
        for receipt in receipts {
            if self
                .entries
                .get(&receipt.mutation)
                .is_some_and(|entry| entry.fingerprint == receipt.fingerprint && entry.value.as_ref() == Some(&receipt.value))
            {
                self.unsaved.remove(&receipt.mutation);
            }
        }
    }

    pub(super) fn restore(&mut self, mut receipts: Vec<MutationReceipt>) -> Result<()> {
        let reading = self.execution.read();
        let now = reading.instant;
        let wall = reading.date;
        receipts.sort_by_key(|receipt| receipt.expires_at);
        let mut restored = BTreeMap::new();
        let mut order = VecDeque::with_capacity(receipts.len());
        for receipt in receipts {
            let Ok(remaining) = receipt.expires_at.duration_since(wall) else {
                continue;
            };
            let remaining = remaining.min(self.retention);
            if remaining.is_zero() {
                continue;
            }
            let entry = Entry {
                fingerprint: receipt.fingerprint,
                deadline: now + remaining,
                expires_at: receipt.expires_at,
                value: Some(receipt.value),

                completion: Arc::new(()),
            };
            order.push_back(receipt.mutation.clone());
            if restored.insert(receipt.mutation, entry).is_some() {
                return Err(Error::InvalidInput {
                    field: "mutation recovery",
                    reason: "duplicate receipt identity",
                });
            }
        }
        self.unsaved.clear();
        self.entries = restored;
        self.order = order;
        while self.entries.len() > self.maximum.get() {
            self.evict_oldest()?;
        }
        Ok(())
    }

    // Pending Work and Attached Callers Retain Their Outcome Independently
    // Of Retry Eligibility. A Pinned Front Must Not Hold Later Expired Receipts.
    pub(super) fn purge(&mut self, now: Instant) {
        // Admission And Recovery Order Deadlines; No Later Entry Can Expire First.
        if self.order.front().and_then(|id| self.entries.get(id)).is_some_and(|entry| now < entry.deadline) {
            return;
        }
        self.order.retain(|id| {
            let remove = self.entries.get(id).is_none_or(|entry| now >= entry.deadline && entry.reclaimable());
            if remove {
                self.entries.remove(id);
                self.unsaved.remove(id);
            }
            !remove
        });
        if self.entries.is_empty() {
            self.clear();
        }
    }
}

impl StorageManager {
    #[tracing::instrument(name = "storage.receipts.recover_receipts", level = "debug", skip_all)]
    pub fn mutation_durable(&self, mutation: &MutationId, position: &super::StorageCursor) -> Result<bool> {
        let state = self.inner.state.lock().unwrap();
        Ok(state.status.saved.covers(position)? && state.receipts.position_saved(mutation, position)?)
    }
    pub async fn wait_mutation_durable(&self, mutation: &MutationId, position: &super::StorageCursor) -> Result<()> {
        let mut changed = self.inner.changed.subscribe();
        loop {
            {
                let state = self.inner.state.lock().unwrap();
                if state.status.saved.covers(position)? && state.receipts.position_saved(mutation, position)? {
                    return Ok(());
                }
                if let Some(source) = &state.status.failure {
                    return Err(Error::Persistence { source: source.clone() });
                }
                if !state.status.running {
                    return Err(Error::NotReady { domain: "Append receipt worker" });
                }
            }
            changed.changed().await.map_err(|_| Error::Closing { domain: "Append receipt" })?;
        }
    }
    pub(super) async fn settle_mutation_receipts(&self) -> Result<()> {
        let mut changed = self.inner.changed.subscribe();
        loop {
            {
                let state = self.inner.state.lock().unwrap();
                if !state.persisting_receipts && state.receipts.pending_positions(&state.status.saved).is_empty() {
                    return Ok(());
                }
                if let Some(source) = &state.status.failure {
                    return Err(Error::Persistence { source: source.clone() });
                }
                if !state.status.running {
                    return Err(Error::NotReady { domain: "Append receipt worker" });
                }
            }
            self.inner.notify();
            changed.changed().await.map_err(|_| Error::Closing { domain: "Append receipts" })?;
        }
    }
    pub(super) async fn recover_receipts(&self) -> Result<()> {
        if !self.inner.primary.capabilities().mutation_receipts {
            return Ok(());
        }
        let limits = self.inner.state.lock().unwrap().receipts.recovery_limits();
        let bytes = limits.bytes.get();
        let receipts = self.inner.bounded("primary.list_mutation_receipts", self.inner.primary.list_mutation_receipts(limits)).await?;
        if receipts.len() > limits.rows.get()
            || receipts
                .iter()
                .try_fold(0usize, |total, receipt| super::manager::encoded_size(receipt).map(|size| total.saturating_add(size)))?
                > bytes
        {
            return Err(Error::InvalidInput {
                field: "mutation recovery",
                reason: "backend exceeded recovery bounds",
            });
        }
        for receipt in &receipts {
            if let MutationValue::Definition { outcome, .. } = &receipt.value {
                outcome.validate()?;
            }
            if let MutationValue::Deleted(file) = &receipt.value {
                self.inner.primary.get_history().check(&file.history)?;
            }
            if let MutationValue::Schema { outcome, .. } = &receipt.value {
                outcome.validate()?;
            }
            if let MutationValue::Cursor(cursor) | MutationValue::Schema { position: cursor, .. } = &receipt.value {
                let state = self.inner.state.lock().unwrap();
                if !state.status.saved.covers(cursor)? {
                    return Err(Error::InvalidInput {
                        field: "mutation recovery",
                        reason: "receipt exceeds saved data frontier",
                    });
                }
            }
            if let MutationValue::Producer(producer) = &receipt.value {
                self.inner.primary.get_history().check(&producer.session.history)?;
                if producer.session.generation == 0 {
                    return Err(Error::InvalidInput {
                        field: "mutation recovery",
                        reason: "zero producer generation",
                    });
                }
            }
        }
        for receipt in &receipts {
            if let MutationValue::Reference(reference) = &receipt.value {
                self.inner.primary.get_history().check(&reference.position.history)?;
                reference.check_native().map_err(|_| Error::InvalidInput {
                    field: "mutation recovery",
                    reason: "invalid completed file reference",
                })?;
            }
        }
        let mut state = self.inner.state.lock().unwrap();
        state.receipts.restore(receipts)?;
        state.collect();
        Ok(())
    }

    pub fn info(&self) -> Result<crate::StorageInfo> {
        let mut state = self.inner.state.lock().unwrap();
        if !state.ready || state.status.closing {
            return Err(Error::NotReady { domain: "storage" });
        }
        let mutations = if self.inner.primary.capabilities().mutation_receipts {
            Some(state.receipts.window(self.inner.primary.get_history(), self.inner.settings.policy.mutation_retention)?)
        } else {
            None
        };
        Ok(crate::StorageInfo {
            accepted: state.status.accepted.clone(),
            saved: state.status.saved.clone(),
            mutations,
            file_batch_bytes: self.inner.settings.transfer_chunk_bytes().get() as u64,
        })
    }

    /// A Retained Mutation Receipt Can Reissue Its Exact Position Once Durability Is Proven.
    pub fn check_receipt_position(&self, position: &super::StorageCursor) -> Result<bool> {
        self.inner.primary.get_history().check(&position.history)?;
        let state = self.inner.state.lock().unwrap();
        super::capacity::admitting(&state, false)?;
        if !state.receipts.issued_position(position) {
            return Ok(false);
        }
        if !state.status.saved.covers(position)? {
            return Err(Error::NotReady {
                domain: "read at storage mutation receipt before durability",
            });
        }
        Ok(true)
    }

    pub fn mutation_window(&self) -> Result<MutationWindow> {
        if !self.inner.primary.capabilities().mutation_receipts {
            return Err(Error::InvalidInput {
                field: "storage mutations",
                reason: "backend does not support mutation receipts",
            });
        }
        let mut state = self.inner.state.lock().unwrap();
        if !state.ready || state.status.closing {
            return Err(Error::NotReady { domain: "storage mutations" });
        }
        let window = state.receipts.window(self.inner.primary.get_history(), self.inner.settings.policy.mutation_retention)?;
        state.collect();
        Ok(window)
    }
}

impl MutationValue {
    pub(super) fn deleted(self) -> Result<super::StagedFile> {
        match self {
            Self::Deleted(file) => Ok(file),
            _ => Err(rejected()),
        }
    }

    pub(super) fn cursor(self) -> Result<super::StorageCursor> {
        match self {
            Self::Cursor(value) | Self::Schema { position: value, .. } => Ok(value),
            _ => Err(rejected()),
        }
    }

    pub(super) fn producer(self) -> Result<FileProducer> {
        match self {
            Self::Producer(value) => Ok(value),
            _ => Err(rejected()),
        }
    }

    pub(super) fn reference(self) -> Result<super::FileReference> {
        match self {
            Self::Reference(value) => Ok(*value),
            _ => Err(rejected()),
        }
    }

    pub(super) fn progress(self) -> Result<super::FileProgress> {
        match self {
            Self::Progress(value) => Ok(value),
            _ => Err(rejected()),
        }
    }
}

impl MemorySize for MutationValue {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Deleted(value) => value.memory_size() - std::mem::size_of::<super::StagedFile>(),
                Self::Cursor(value) => value.memory_size() - std::mem::size_of::<super::StorageCursor>(),
                Self::Producer(value) => value.memory_size() - std::mem::size_of::<FileProducer>(),
                Self::Reference(value) => value.as_ref().memory_size(),
                Self::Definition { name, outcome, .. } => name.memory_size() - std::mem::size_of::<Identifier>() + outcome.heap_bytes(),
                Self::Schema { position, key, outcome } => position.memory_size() + key.memory_size() + outcome.heap_bytes(),
                Self::Progress(_) | Self::Rejected => 0,
            }
    }
}
impl MemorySize for MutationReceipt {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size() - std::mem::size_of::<MutationId>() + self.value.memory_size()
            - std::mem::size_of::<MutationValue>()
    }
}

impl MutationHeader {
    pub fn definition(&self, kind: super::DefinitionKind, name: Identifier, outcome: super::DefinitionOutcome) -> MutationReceipt {
        self.complete(MutationValue::Definition { kind, name, outcome })
    }
    pub fn replay_definition(
        &self, prior: &MutationReceipt, owner: super::DefinitionKind, name: &Identifier,
    ) -> Result<super::DefinitionOutcome> {
        if prior.mutation != self.mutation || prior.fingerprint != self.fingerprint {
            return Err(Error::backend(
                owner.action(),
                super::BackendFailure::Conflict,
                std::io::Error::other("mutation identity has a different payload"),
            ));
        }
        match &prior.value {
            MutationValue::Definition { kind, name: current, outcome } if *kind == owner && current == name => Ok(outcome.clone()),
            _ => Err(Error::InvalidInput {
                field: "storage definition receipt",
                reason: "mutation identity owns a different result",
            }),
        }
    }
    pub fn complete(&self, value: MutationValue) -> MutationReceipt {
        MutationReceipt {
            mutation: self.mutation.clone(),
            fingerprint: self.fingerprint,
            expires_at: self.expires_at,
            value,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn epoch() -> Result<Identifier> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|source| Error::OperationFailed {
        operation: "storage retry window".into(),
        source: Arc::new(source),
    })?;
    let name = bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    Identifier::new(name).map_err(|_| Error::InvalidInput {
        field: "storage retry window",
        reason: "invalid generated identity",
    })
}

fn rejected() -> Error {
    Error::InvalidInput {
        field: "file mutation",
        reason: "owned production ended without the requested result",
    }
}
fn exhausted() -> Error {
    Error::ResourceExhausted { resource: "storage mutation receipts" }
}
fn expired() -> Error {
    Error::InvalidInput {
        field: "storage mutation",
        reason: "retry window expired or unknown; reconcile state before issuing new work",
    }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn restored_receipts_expire_on_the_host_clock() {
        let epoch = SystemTime::UNIX_EPOCH + Duration::from_secs(123);
        let clock = crate::execution::Execution::at(epoch, Instant::now, |duration| Box::pin(tokio::time::sleep(duration)));
        let retention = Duration::from_secs(40);
        let maximum = NonZeroUsize::new(8).unwrap();
        let mut original = Receipts::new(retention, maximum, clock.clone()).unwrap();
        let mutation = MutationId {
            window: original.window.clone(),
            key: Identifier::new("saved").unwrap(),
        };
        original.insert(&mutation, [1; 32]).unwrap();
        original.complete(&mutation, MutationValue::Rejected);
        let saved = original.snapshot(None);
        assert_eq!(saved[0].expires_at, epoch + retention);

        tokio::time::advance(retention / 2).await;
        let mut restored = Receipts::new(retention, maximum, clock).unwrap();
        restored.restore(saved).unwrap();
        assert!(matches!(restored.lookup(&mutation, &[1; 32], None).unwrap(), ReceiptState::Complete(MutationValue::Rejected)));
        tokio::time::advance(retention / 2).await;
        assert!(restored.lookup(&mutation, &[1; 32], None).is_err());
    }

    #[test]
    fn admission_evicts_completed_receipts_without_losing_active_owners() {
        // Fill One Receipt Window At A Fixed Clock Reading.
        let now = Instant::now();
        let execution = crate::execution::Execution::at(SystemTime::UNIX_EPOCH, move || now, |_| Box::pin(std::future::pending()));
        let maximum = NonZeroUsize::new(5).unwrap();
        let mut receipts = Receipts::new(Duration::from_secs(90), maximum, execution).unwrap();
        let ids: Vec<_> = (0..=maximum.get())
            .map(|index| MutationId {
                window: receipts.window.clone(),
                key: Identifier::new(format!("receipt-{index}")).unwrap(),
            })
            .collect();
        for id in &ids[..ids.len() - 1] {
            receipts.insert(id, [1; 32]).unwrap();
        }
        let owned = receipts.completion(&ids[1]);
        let position = super::super::StorageCursor {
            history: super::super::StoreHistory {
                store: Identifier::new("records").unwrap(),
                epoch: Identifier::new("generation").unwrap(),
            },
            sequence: 7,
        };
        for id in &ids[1..ids.len() - 1] {
            let value = if id == &ids[2] { MutationValue::Cursor(position.clone()) } else { MutationValue::Rejected };
            receipts.complete(id, value);
        }

        // Admission Skips The Pending Receipt And The Completed Receipt Still Owned.
        let newest = ids.last().unwrap();
        receipts.insert(newest, [1; 32]).unwrap();
        receipts.complete(newest, MutationValue::Rejected);
        assert!(matches!(receipts.lookup(&ids[0], &[1; 32], None).unwrap(), ReceiptState::Pending));
        assert!(matches!(receipts.lookup(&ids[1], &[1; 32], None).unwrap(), ReceiptState::Complete(MutationValue::Rejected)));
        assert!(
            matches!(receipts.lookup(&ids[2], &[1; 32], None).unwrap(), ReceiptState::Complete(MutationValue::Cursor(cursor)) if cursor == position)
        );
        assert!(receipts.lookup(&ids[3], &[1; 32], None).is_err());
        assert!(matches!(receipts.lookup(newest, &[1; 32], None).unwrap(), ReceiptState::Complete(MutationValue::Rejected)));
        assert!(!receipts.snapshot(None).iter().any(|receipt| receipt.mutation == ids[3]));
        drop(owned);
    }

    #[test]
    fn receipt_checkpoints_keep_unsaved_outcomes_until_success() {
        let mut receipts =
            Receipts::new(Duration::from_secs(90), NonZeroUsize::new(4).unwrap(), crate::execution::Execution::default()).unwrap();
        let first = MutationId {
            window: receipts.window.clone(),
            key: Identifier::new("first").unwrap(),
        };
        receipts.insert(&first, [1; 32]).unwrap();
        receipts.complete(&first, MutationValue::Rejected);
        let captured = receipts.snapshot(None);
        assert_eq!(captured.len(), 1);
        assert_eq!(receipts.snapshot(None), captured);
        let second = MutationId {
            window: receipts.window.clone(),
            key: Identifier::new("second").unwrap(),
        };
        receipts.insert(&second, [2; 32]).unwrap();
        receipts.complete(&second, MutationValue::Rejected);
        receipts.saved(&captured);
        let pending = receipts.snapshot(None);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].mutation, second);
        assert!(matches!(receipts.lookup(&first, &[1; 32], None).unwrap(), ReceiptState::Complete(MutationValue::Rejected)));
        receipts.saved(&pending);
        assert!(receipts.snapshot(None).is_empty());
        receipts.restore(captured).unwrap();
        assert!(receipts.snapshot(None).is_empty());
        assert!(matches!(receipts.lookup(&first, &[1; 32], None).unwrap(), ReceiptState::Complete(MutationValue::Rejected)));
    }
}
