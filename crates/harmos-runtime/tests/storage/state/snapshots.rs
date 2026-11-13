/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "snapshots/cancelled_snapshot_releases.rs"]
mod cancelled_snapshot_releases;
#[path = "snapshots/corrupt_snapshot_blocks.rs"]
mod corrupt_snapshot_blocks;
#[path = "snapshots/journal_replays_alone.rs"]
mod journal_replays_alone;
#[path = "snapshots/oversized_entry_warns_and_preserves_recovery.rs"]
mod oversized_entry_warns_and_preserves_recovery;
#[path = "snapshots/oversized_snapshot_skipped.rs"]
mod oversized_snapshot_skipped;
#[path = "snapshots/segmented_snapshot_matches_full_replay.rs"]
mod segmented_snapshot_matches_full_replay;
#[path = "snapshots/snapshot_failure_retryable.rs"]
mod snapshot_failure_retryable;
#[path = "snapshots/snapshot_keeps_tail.rs"]
mod snapshot_keeps_tail;
#[path = "snapshots/snapshot_payload_boundary_restores_segmented_state.rs"]
mod snapshot_payload_boundary_restores_segmented_state;
#[path = "snapshots/snapshot_read_ignores_write_queue_budget.rs"]
mod snapshot_read_ignores_write_queue_budget;
#[path = "snapshots/snapshot_restores_state.rs"]
mod snapshot_restores_state;
#[path = "snapshots/snapshot_resumes_durable.rs"]
mod snapshot_resumes_durable;
#[path = "snapshots/snapshot_waits_queue.rs"]
mod snapshot_waits_queue;
#[path = "snapshots/snapshots_require_storage.rs"]
mod snapshots_require_storage;

#[path = "../../support/snapshot_store.rs"]
mod store_fixture;

/* === Imports === */

// Standard Library Imports
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::{Arc, atomic::Ordering};
// Third Party Imports
use tracing::instrument::WithSubscriber;
use tracing_subscriber::prelude::*;

// Local Imports
use super::{CleanupCapture, closed};
use crate::storage::support::{Change, Fact, HOUSE, Noted, deposit};
use crate::storage::{Bank, Data, Ledger, Recovery, StorageManager, catalog, config, history, schema};
use harmos_runtime::state::SnapshotPolicy;
use harmos_runtime::storage::ReadLimits;
use harmos_runtime::storage::state::SnapshotPayload;
use harmos_runtime::{Runtime, state::Focus, state::Position, state::PrincipalId, state::RequestKey};
use store_fixture::Store;

/* === Type Aliases === */

type Refusal = &'static str;

/* === Structs === */

struct Model;

#[derive(Clone, Default)]
struct Warnings(Arc<std::sync::Mutex<Vec<SnapshotWarning>>>);

#[derive(Default, Debug)]
struct SnapshotWarning {
    size_bytes: u64,
    segment_limit: u64,
    replay_entries: u64,
}

#[derive(harmos_runtime::Described, Clone, serde::Serialize, serde::Deserialize)]
struct Add {
    amount: i64,
}
impl Add {
    pub const ENTRY: harmos_runtime::state::TransactionEntry<i64, Self> =
        harmos_runtime::state::TransactionEntry::new("snapshot.add").barrier();
}

#[derive(harmos_runtime::Described, Clone, serde::Serialize, serde::Deserialize)]
struct Seen {
    before: i64,
}

/* === Enums === */

#[derive(Clone)]
enum Transactions {
    Add(Add),
}
impl From<Add> for Transactions {
    fn from(value: Add) -> Self {
        Self::Add(value)
    }
}
impl harmos_runtime::state::Transaction<i64> for Transactions {
    type Error = Refusal;
    fn check(&self, state: &i64) -> std::result::Result<(), Self::Error> {
        match self {
            Self::Add(value) => harmos_runtime::state::Transaction::check(value, state),
        }
    }
    fn apply(&self, state: &mut i64) {
        match self {
            Self::Add(value) => harmos_runtime::state::Transaction::apply(value, state),
        }
    }
    fn records(&self, state: &i64, records: &mut harmos_runtime::state::Records) {
        match self {
            Self::Add(value) => harmos_runtime::state::Transaction::records(value, state, records),
        }
    }
}
impl harmos_runtime::state::CatalogCodec for Transactions {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        match self {
            Self::Add(_) => Add::ENTRY.definition(),
        }
    }
    fn encode<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Add(value) => serde::Serialize::serialize(value, serializer),
        }
    }
}
fn transactions_state_catalog() -> harmos_runtime::state::StateCatalog<i64, Transactions> {
    harmos_runtime::state::StateCatalog::new().transaction(Add::ENTRY, Transactions::Add, |value| {
        let Transactions::Add(value) = value;
        Some(value)
    })
}

#[derive(Clone)]
enum Records {
    Seen(Seen),
}
impl From<Seen> for Records {
    fn from(value: Seen) -> Self {
        Self::Seen(value)
    }
}
impl harmos_runtime::state::CatalogCodec for Records {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        match self {
            Self::Seen(_) => <Seen as harmos_runtime::state::Record>::ENTRY.definition(),
        }
    }
    fn encode<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Seen(value) => serde::Serialize::serialize(value, serializer),
        }
    }
}
fn records_registry() -> harmos_runtime::state::RecordRegistry<Records> {
    harmos_runtime::state::RecordRegistry::new().entry(<Seen as harmos_runtime::state::Record>::ENTRY, Records::Seen, |value| {
        let Records::Seen(value) = value;
        Some(value)
    })
}

/* === Implementations === */

impl harmos_runtime::state::Record for Seen {
    const ENTRY: harmos_runtime::state::RecordEntry<Self> = harmos_runtime::state::RecordEntry::new("snapshot.seen");
}

impl tracing::field::Visit for SnapshotWarning {
    fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        match field.name() {
            "size_bytes" => self.size_bytes = value,
            "segment_limit" => self.segment_limit = value,
            "replay_entries" => self.replay_entries = value,
            _ => {}
        }
    }
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Warnings {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        if event.metadata().fields().field("segment_limit").is_some() {
            let mut warning = SnapshotWarning::default();
            event.record(&mut warning);
            self.0.lock().unwrap().push(warning);
        }
    }
}

impl harmos_runtime::Lifecycle for Model {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl harmos_runtime::Application for Model {
    type State = i64;
    type Transaction = Transactions;
    type Record = Records;
}

impl harmos_runtime::state::Transaction<i64> for Add {
    type Error = Refusal;
    fn check(&self, _: &i64) -> Result<(), Refusal> {
        Ok(())
    }
    fn records(&self, state: &i64, out: &mut harmos_runtime::state::Records) {
        out.push(Seen { before: *state }).unwrap();
    }
    fn apply(&self, state: &mut i64) {
        *state += self.amount;
    }
}

/* === Private Functions === */

fn limits() -> ReadLimits {
    ReadLimits {
        rows: NonZeroUsize::new(1).unwrap(),
        bytes: NonZeroUsize::new(64 * 1024).unwrap(),
    }
}

fn policy(every: u64) -> SnapshotPolicy {
    SnapshotPolicy {
        schema: schema(),
        every: NonZeroU64::new(every).unwrap(),
        max_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
    }
}

async fn balance(runtime: &Runtime<Bank>) -> i64 {
    runtime.state.state().read(|state| state.balances.get(&HOUSE).copied().unwrap_or(0)).await.unwrap().value
}

async fn launch(snapshot: SnapshotPolicy, storage: &StorageManager) -> Runtime<Bank> {
    crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
        setup.settings().storage(storage).register(catalog()).managed_storage(harmos_runtime::config::ManagedStorageSettings {
            replay: limits(),
            capture: limits(),
            snapshots: Some(snapshot),
            journal_tail: 2,
        });
    })
    .await
    .unwrap()
}
