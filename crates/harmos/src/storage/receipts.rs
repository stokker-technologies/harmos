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
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

// Third Party Imports
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

// Local Imports
use super::config::{MAX_MUTATIONS, MUTATION_MEMORY_BYTES, MUTATION_RETENTION};
use super::{FileProducer, MutationId, MutationWindow, StorageManager, StoreHistory};
use crate::supervision::executor;
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

pub(super) struct Receipts {
    window: Identifier,
    deadline: Instant,
    expires_at: SystemTime,
    entries: BTreeMap<MutationId, Entry>,
    bytes: usize,
}

struct Entry {
    fingerprint: [u8; 32],
    deadline: Instant,
    expires_at: SystemTime,
    value: Option<MutationValue>,
    bytes: usize,
}

/* === Enums === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
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
impl Receipts {
    pub(super) fn new(retention: Duration) -> Result<Self> {
        let now = executor::now();
        let wall = SystemTime::now();
        Ok(Self {
            window: epoch()?,
            deadline: now + retention,
            expires_at: wall + retention,
            entries: BTreeMap::new(),
            bytes: 0,
        })
    }

    pub(super) fn window(
        &mut self,
        history: &StoreHistory,
        retention: Duration,
    ) -> Result<MutationWindow> {
        let now = executor::now();
        self.purge(now);
        if now >= self.deadline {
            self.window = epoch()?;
            self.deadline = now + retention;
            self.expires_at = SystemTime::now()
                .checked_add(retention)
                .ok_or_else(expired)?;
        }
        Ok(MutationWindow {
            history: history.clone(),
            id: self.window.clone(),
            expires_at: self.expires_at,
        })
    }

    pub(super) fn lookup(&self, id: &MutationId, fingerprint: &[u8; 32]) -> Result<ReceiptState> {
        let now = executor::now();
        if let Some(entry) = self.entries.get(id) {
            if now >= entry.deadline {
                tracing::error!(window = %id.window, mutation = %id.key, reason = "retry window expired", "storage mutation rejected");
                return Err(expired());
            }
            if entry.fingerprint != *fingerprint {
                tracing::error!(window = %id.window, mutation = %id.key, reason = "identity reused with different request", "storage mutation rejected");
                return Err(Error::InvalidInput {
                    field: "storage mutation",
                    reason: "identity reused with different request",
                });
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

    pub(super) fn insert(
        &mut self,
        id: &MutationId,
        fingerprint: [u8; 32],
        outcome_bytes: usize,
    ) -> Result<()> {
        self.purge(executor::now());
        if !matches!(self.lookup(id, &fingerprint)?, ReceiptState::Missing) {
            return Err(Error::AlreadyRegistered {
                domain: "storage mutation",
                name: id.key.to_string(),
            });
        }
        let bytes = charge(id, outcome_bytes)?;
        if self.entries.len() >= MAX_MUTATIONS.get()
            || bytes > MUTATION_MEMORY_BYTES.get().saturating_sub(self.bytes)
        {
            return Err(Error::ResourceExhausted {
                resource: "storage mutation receipts",
            });
        }
        self.entries.insert(
            id.clone(),
            Entry {
                fingerprint,
                deadline: self.deadline,
                expires_at: self.expires_at,
                value: None,
                bytes,
            },
        );
        self.bytes += bytes;
        Ok(())
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
        let Some(entry) = self.entries.get_mut(id) else {
            return;
        };
        if entry.value.is_some() {
            return;
        }
        // Admission Reserved The Largest Possible Outcome Before Mutating.
        assert!(charge(id, value.memory_size()).is_ok_and(|bytes| bytes <= entry.bytes));
        entry.value = Some(value);
    }

    pub(super) fn snapshot_bytes(&self) -> usize {
        self.bytes
    }

    pub(super) fn clear(&mut self) {
        self.entries = BTreeMap::new();
        self.bytes = 0;
    }

    pub(super) fn snapshot(&self, through: Option<&super::StorageCursor>) -> Vec<MutationReceipt> {
        let now = executor::now();
        let count = self
            .entries
            .values()
            .filter(|entry| entry.value.is_some() && now < entry.deadline)
            .count();
        let mut receipts = Vec::with_capacity(count);
        for (mutation, entry) in &self.entries {
            if let Some(value) = &entry.value
                && now < entry.deadline
                && match value {
                    MutationValue::Reference(reference) => through
                        .is_none_or(|through| through.covers(&reference.position).unwrap_or(false)),
                    MutationValue::Cursor(cursor) => {
                        through.is_none_or(|through| through.covers(cursor).unwrap_or(false))
                    }
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

    pub(super) fn restore(&mut self, receipts: Vec<MutationReceipt>) -> Result<()> {
        let now = executor::now();
        let wall = SystemTime::now();
        let mut restored = BTreeMap::new();
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
            if total > MUTATION_MEMORY_BYTES.get() || restored.len() >= MAX_MUTATIONS.get() {
                return Err(exhausted());
            }
            let entry = Entry {
                fingerprint: receipt.fingerprint,
                deadline: now + remaining,
                expires_at: receipt.expires_at,
                value: Some(receipt.value),
                bytes,
            };
            if restored.insert(receipt.mutation, entry).is_some() {
                return Err(Error::InvalidInput {
                    field: "mutation recovery",
                    reason: "duplicate receipt identity",
                });
            }
        }
        self.entries = restored;
        self.bytes = total;
        Ok(())
    }

    pub(super) fn purge(&mut self, now: Instant) {
        self.entries.retain(|_, entry| {
            if now < entry.deadline {
                true
            } else {
                self.bytes -= entry.bytes;
                false
            }
        });
        if self.entries.is_empty() {
            self.entries = BTreeMap::new();
        }
    }
}

impl<C> StorageManager<C> {
    pub(super) async fn recover_receipts(&self) -> Result<()> {
        if !self.inner.primary.supports_mutation_receipts() {
            return Ok(());
        }
        let bytes = (self.inner.settings.budget.read_memory_bytes.get() / 2)
            .min(MUTATION_MEMORY_BYTES.get());
        let limits = super::ReadLimits {
            rows: MAX_MUTATIONS,
            bytes: std::num::NonZeroUsize::new(bytes).ok_or_else(exhausted)?,
        };
        let _reservation = super::capacity::Reservation::lifecycle(
            &self.inner,
            super::capacity::Pool::Read,
            bytes,
        )?;
        let receipts = self
            .inner
            .bounded(self.inner.primary.recover_mutations(limits))
            .await?;
        if receipts.len() > limits.rows.get() || receipts.memory_size() > bytes {
            return Err(Error::InvalidInput {
                field: "mutation recovery",
                reason: "backend exceeded recovery bounds",
            });
        }
        for receipt in &receipts {
            if let MutationValue::Deleted(file) = &receipt.value {
                self.inner.primary.history().check(&file.history)?;
            }
            if let MutationValue::Cursor(cursor) = &receipt.value {
                let state = self.inner.state.lock().unwrap();
                if !state.status.saved.covers(cursor)? {
                    return Err(Error::InvalidInput {
                        field: "mutation recovery",
                        reason: "receipt exceeds saved data frontier",
                    });
                }
            }
            if let MutationValue::Producer(producer) = &receipt.value {
                self.inner
                    .primary
                    .history()
                    .check(&producer.session.history)?;
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
                self.inner
                    .primary
                    .history()
                    .check(&reference.position.history)?;
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

    pub fn info(&self) -> Result<crate::storage::StorageInfo> {
        let mut state = self.inner.state.lock().unwrap();
        if !state.ready || state.status.closing {
            return Err(Error::NotReady { domain: "storage" });
        }
        let mutations = if self.inner.primary.supports_mutation_receipts() {
            Some(
                state
                    .receipts
                    .window(self.inner.primary.history(), MUTATION_RETENTION)?,
            )
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
            return Err(Error::InvalidInput {
                field: "storage mutations",
                reason: "backend does not support mutation receipts",
            });
        }
        let mut state = self.inner.state.lock().unwrap();
        if !state.ready || state.status.closing {
            return Err(Error::NotReady {
                domain: "storage mutations",
            });
        }
        let window = state
            .receipts
            .window(self.inner.primary.history(), MUTATION_RETENTION)?;
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
                Self::Deleted(value) => {
                    value.memory_size() - std::mem::size_of::<super::StagedFile>()
                }
                Self::Cursor(value) => {
                    value.memory_size() - std::mem::size_of::<super::StorageCursor>()
                }
                Self::Producer(value) => value.memory_size() - std::mem::size_of::<FileProducer>(),
                Self::Reference(value) => value.as_ref().memory_size(),
                Self::Progress(_) | Self::Rejected => 0,
            }
    }
}
impl MemorySize for MutationReceipt {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<MutationId>()
            + self.value.memory_size()
            - std::mem::size_of::<MutationValue>()
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
    let name = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Identifier::new(name).map_err(|_| Error::InvalidInput {
        field: "storage retry window",
        reason: "invalid generated identity",
    })
}

fn charge(id: &MutationId, outcome_bytes: usize) -> Result<usize> {
    // Reuse The Native Map Policy, Including An Empty Root And The File's ID Copy.
    let tree = crate::btree_map_memory::<MutationId, Entry>(1);
    id.memory_size()
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(outcome_bytes))
        .and_then(|bytes| bytes.checked_add(tree))
        .ok_or_else(exhausted)
}

fn rejected() -> Error {
    Error::InvalidInput {
        field: "file mutation",
        reason: "owned production ended without the requested result",
    }
}
fn exhausted() -> Error {
    Error::ResourceExhausted {
        resource: "storage mutation receipts",
    }
}
fn expired() -> Error {
    Error::InvalidInput {
        field: "storage mutation",
        reason: "retry window expired or unknown; reconcile state before issuing new work",
    }
}

/* === Tests === */
