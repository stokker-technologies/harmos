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
use std::collections::{BTreeMap, VecDeque};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

// Third Party Imports
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

// Local Imports
use super::config::MUTATION_RETENTION;
use super::{FileProducer, MutationId, MutationWindow, StorageManager, StoreHistory};
use crate::{Error, Identifier, MemorySize, Result};

/* === Constants === */

const ID_COPIES: usize = 5;
const COMPLETION_BYTES: usize = 2 * std::mem::size_of::<usize>();
const NODE_SHARE: usize = node_bytes().div_ceil(4);
// The Map And Order Headers Plus The Root Node, Charged While Any Receipt Is Held.
const LEDGER_BASE: usize = std::mem::size_of::<BTreeMap<MutationId, Entry>>() + std::mem::size_of::<VecDeque<MutationId>>() + node_bytes();
// Smallest Possible Receipt Charge, Bounding Recovered Row Counts.
const ENTRY_MINIMUM: usize =
    ID_COPIES * std::mem::size_of::<MutationId>() + std::mem::size_of::<MutationValue>() + NODE_SHARE + COMPLETION_BYTES;

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

pub(super) struct Receipts {
    execution: crate::execution::Execution,
    window: Identifier,
    deadline: Instant,
    expires_at: SystemTime,
    entries: BTreeMap<MutationId, Entry>,
    // Admission Order, Oldest First, For Expiry And Eviction.
    order: VecDeque<MutationId>,
    bytes: usize,
    bound: NonZeroUsize,
}

struct Entry {
    fingerprint: [u8; 32],
    deadline: Instant,
    expires_at: SystemTime,
    value: Option<MutationValue>,
    bytes: usize,
    completion: Arc<()>,
}

pub(super) struct Completion(Arc<()>);

/* === Enums === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum MutationValue {
    Cursor(super::StorageCursor),
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
    pub(super) fn new(retention: Duration, bound: NonZeroUsize, execution: crate::execution::Execution) -> Result<Self> {
        let now = execution.now();
        let wall = SystemTime::now();
        Ok(Self {
            execution,
            window: epoch()?,
            deadline: now + retention,
            expires_at: wall + retention,
            entries: BTreeMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            bound,
        })
    }

    pub(super) fn recovery_limits(&self) -> super::ReadLimits {
        super::ReadLimits {
            rows: NonZeroUsize::new(self.bound.get() / ENTRY_MINIMUM).unwrap_or(NonZeroUsize::MIN),
            bytes: self.bound.saturating_add(std::mem::size_of::<Vec<MutationReceipt>>()),
        }
    }

    pub(super) fn window(&mut self, history: &StoreHistory, retention: Duration) -> Result<MutationWindow> {
        let now = self.execution.now();
        self.purge(now);
        if now >= self.deadline {
            self.window = epoch()?;
            self.deadline = now + retention;
            self.expires_at = SystemTime::now().checked_add(retention).ok_or_else(expired)?;
        }
        Ok(MutationWindow { history: history.clone(), id: self.window.clone(), expires_at: self.expires_at })
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
                return Err(Error::InvalidInput { field: "storage mutation", reason: "identity reused with different request" });
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

    pub(super) fn insert(&mut self, id: &MutationId, fingerprint: [u8; 32], outcome_bytes: usize) -> Result<()> {
        self.purge(self.execution.now());
        if !matches!(self.lookup(id, &fingerprint, None)?, ReceiptState::Missing) {
            return Err(Error::AlreadyRegistered { domain: "storage mutation", name: id.key.to_string() });
        }
        let bytes = charge(id, outcome_bytes)?;
        while self.bytes.saturating_add(LEDGER_BASE).saturating_add(bytes) > self.bound.get() {
            self.evict_oldest()?;
        }
        self.order.push_back(id.clone());
        self.entries.insert(
            id.clone(),
            Entry { fingerprint, deadline: self.deadline, expires_at: self.expires_at, value: None, bytes, completion: Arc::new(()) },
        );
        self.bytes += bytes;
        Ok(())
    }

    pub(super) fn prepared(&self, id: &MutationId, value: MutationValue) -> MutationReceipt {
        let entry = self.entries.get(id).expect("admitted receipt");
        MutationReceipt { mutation: id.clone(), fingerprint: entry.fingerprint, expires_at: entry.expires_at, value }
    }

    pub(super) fn complete(&mut self, id: &MutationId, value: MutationValue) {
        let entry = self.entries.get_mut(id).expect("pending receipt retained through completion");
        if entry.value.is_some() {
            return;
        }
        // Admission Reserved The Largest Possible Outcome Before Mutating.
        assert!(charge(id, value.memory_size()).is_ok_and(|bytes| bytes <= entry.bytes));
        entry.value = Some(value);
    }

    pub(super) fn snapshot_bytes(&self) -> usize {
        if self.entries.is_empty() { 0 } else { self.bytes + LEDGER_BASE }
    }

    pub(super) fn clear(&mut self) {
        self.entries = BTreeMap::new();
        self.order = VecDeque::new();
        self.bytes = 0;
    }

    // Drop The Oldest Completed Receipt; Pending Receipts Still Await Their Outcome.
    fn evict_oldest(&mut self) -> Result<()> {
        let index = self.order.iter().position(|id| self.entries.get(id).is_some_and(Entry::reclaimable)).ok_or_else(exhausted)?;
        let id = self.order.remove(index).expect("ordered receipt");
        let entry = self.entries.remove(&id).expect("ordered receipt");
        self.bytes -= entry.bytes;
        Ok(())
    }

    pub(super) fn snapshot(&self, through: Option<&super::StorageCursor>) -> Vec<MutationReceipt> {
        let now = self.execution.now();
        let count = self.entries.values().filter(|entry| entry.value.is_some() && now < entry.deadline).count();
        let mut receipts = Vec::with_capacity(count);
        for (mutation, entry) in &self.entries {
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

    pub(super) fn restore(&mut self, mut receipts: Vec<MutationReceipt>) -> Result<()> {
        let now = self.execution.now();
        let wall = SystemTime::now();
        receipts.sort_by_key(|receipt| receipt.expires_at);
        let mut restored = BTreeMap::new();
        let mut order = VecDeque::with_capacity(receipts.len());
        let mut total = 0usize;
        for receipt in receipts {
            let Ok(remaining) = receipt.expires_at.duration_since(wall) else {
                continue;
            };
            let remaining = remaining.min(MUTATION_RETENTION);
            if remaining.is_zero() {
                continue;
            }
            let bytes = charge(&receipt.mutation, receipt.value.memory_size())?;
            total = total.checked_add(bytes).ok_or_else(exhausted)?;
            let entry = Entry {
                fingerprint: receipt.fingerprint,
                deadline: now + remaining,
                expires_at: receipt.expires_at,
                value: Some(receipt.value),
                bytes,
                completion: Arc::new(()),
            };
            order.push_back(receipt.mutation.clone());
            if restored.insert(receipt.mutation, entry).is_some() {
                return Err(Error::InvalidInput { field: "mutation recovery", reason: "duplicate receipt identity" });
            }
        }
        self.entries = restored;
        self.order = order;
        self.bytes = total;
        while self.snapshot_bytes() > self.bound.get() {
            self.evict_oldest()?;
        }
        Ok(())
    }

    // Pending Work And Attached Callers Retain Their Charged Outcome Independently
    // Of Retry Eligibility. A Pinned Front Must Not Hold Later Expired Receipts.
    pub(super) fn purge(&mut self, now: Instant) {
        self.order.retain(|id| {
            let remove = self.entries.get(id).is_none_or(|entry| now >= entry.deadline && entry.reclaimable());
            if remove && let Some(entry) = self.entries.remove(id) {
                self.bytes -= entry.bytes;
            }
            !remove
        });
        if self.entries.is_empty() {
            self.clear();
        }
    }
}

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.receipts.recover_receipts", level = "debug", skip_all)]
    pub(super) async fn recover_receipts(&self) -> Result<()> {
        if !self.inner.primary.supports_mutation_receipts() {
            return Ok(());
        }
        let limits = self.inner.state.lock().unwrap().receipts.recovery_limits();
        let bytes = limits.bytes.get();
        let _reservation = super::capacity::Reservation::lifecycle(&self.inner, super::capacity::Pool::Read, bytes)?;
        let receipts = self.inner.bounded("primary.recover_mutations", self.inner.primary.recover_mutations(limits)).await?;
        if receipts.len() > limits.rows.get() || receipts.memory_size() > bytes {
            return Err(Error::InvalidInput { field: "mutation recovery", reason: "backend exceeded recovery bounds" });
        }
        for receipt in &receipts {
            if let MutationValue::Deleted(file) = &receipt.value {
                self.inner.primary.history().check(&file.history)?;
            }
            if let MutationValue::Cursor(cursor) = &receipt.value {
                let state = self.inner.state.lock().unwrap();
                if !state.status.saved.covers(cursor)? {
                    return Err(Error::InvalidInput { field: "mutation recovery", reason: "receipt exceeds saved data frontier" });
                }
            }
            if let MutationValue::Producer(producer) = &receipt.value {
                self.inner.primary.history().check(&producer.session.history)?;
                if producer.session.generation == 0 {
                    return Err(Error::InvalidInput { field: "mutation recovery", reason: "zero producer generation" });
                }
            }
        }
        for receipt in &receipts {
            if let MutationValue::Reference(reference) = &receipt.value {
                self.inner.primary.history().check(&reference.position.history)?;
                reference
                    .check_native()
                    .map_err(|_| Error::InvalidInput { field: "mutation recovery", reason: "invalid completed file reference" })?;
            }
        }
        let mut state = self.inner.state.lock().unwrap();
        state.receipts.restore(receipts)?;
        state.collect();
        Ok(())
    }

    pub fn info(&self) -> Result<crate::storage::StorageInfo> {
        let mut state = self.inner.state.lock().unwrap();
        if !state.ready || state.status.closing {
            return Err(Error::NotReady { domain: "storage" });
        }
        let mutations = if self.inner.primary.supports_mutation_receipts() {
            Some(state.receipts.window(self.inner.primary.history(), MUTATION_RETENTION)?)
        } else {
            None
        };
        Ok(crate::storage::StorageInfo {
            accepted: state.status.accepted.clone(),
            saved: state.status.saved.clone(),
            mutations,
            file_batch_bytes: self.inner.settings.response.batch_bytes.get() as u64,
        })
    }

    pub fn mutation_window(&self) -> Result<MutationWindow> {
        if !self.inner.primary.supports_mutation_receipts() {
            return Err(Error::InvalidInput { field: "storage mutations", reason: "backend does not support mutation receipts" });
        }
        let mut state = self.inner.state.lock().unwrap();
        if !state.ready || state.status.closing {
            return Err(Error::NotReady { domain: "storage mutations" });
        }
        let window = state.receipts.window(self.inner.primary.history(), MUTATION_RETENTION)?;
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
            Self::Cursor(value) => Ok(value),
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

/* === Public Functions === */

/* === Private Functions === */

fn epoch() -> Result<Identifier> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|source| Error::OperationFailed { operation: "storage retry window".into(), source: Arc::new(source) })?;
    let name = bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    Identifier::new(name).map_err(|_| Error::InvalidInput { field: "storage retry window", reason: "invalid generated identity" })
}

// One Entry's Amortized Share: Std B-Tree Nodes Below The Root Hold At Least Five
// Entries And Internal Nodes Add At Most A Quarter More, So A Quarter Node Per
// Entry Covers Every Node But The Root, Which `LEDGER_BASE` Charges Once. The
// Identity Is Copied Into The Map Key, The Admission Order, Snapshots And Saves.
fn charge(id: &MutationId, outcome_bytes: usize) -> Result<usize> {
    id.memory_size()
        .checked_mul(ID_COPIES)
        .and_then(|bytes| bytes.checked_add(outcome_bytes))
        .and_then(|bytes| bytes.checked_add(NODE_SHARE))
        .and_then(|bytes| bytes.checked_add(COMPLETION_BYTES))
        .ok_or_else(exhausted)
}

const fn node_bytes() -> usize {
    let alignment = max(max(std::mem::align_of::<MutationId>(), std::mem::align_of::<Entry>()), std::mem::align_of::<usize>());
    (std::mem::size_of::<MutationId>() + std::mem::size_of::<Entry>()) * 11
        + 13 * std::mem::size_of::<usize>()
        + 2 * std::mem::size_of::<u16>()
        + 5 * alignment
}

const fn max(left: usize, right: usize) -> usize {
    if left > right { left } else { right }
}

fn rejected() -> Error {
    Error::InvalidInput { field: "file mutation", reason: "owned production ended without the requested result" }
}
fn exhausted() -> Error {
    Error::ResourceExhausted { resource: "storage mutation receipts" }
}
fn expired() -> Error {
    Error::InvalidInput { field: "storage mutation", reason: "retry window expired or unknown; reconcile state before issuing new work" }
}

/* === Tests === */
