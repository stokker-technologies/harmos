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
use std::sync::Arc;
// Third Party Imports
use serde::{Deserialize, Serialize};
use tokio::sync::{Semaphore, mpsc};
// Local Imports
use harmos_runtime::{
    Application, Error, Runtime, state::CaptureError, state::CaptureLimits, state::Entry, state::FactSource, state::Focus,
    state::JournalCapture, state::Position, state::PreparationError, state::PrincipalId, state::Recorded, state::Records,
    state::SystemFact, state::Transaction,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Refusal = &'static str;
type Group = (Vec<Arc<Entry<Recorded<Model>>>>, Position);

/* === Structs === */

#[harmos_runtime::record(id = "capture.observed", version = 1)]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Observed(String);

#[harmos_runtime::transaction(id = "capture.add", version = 1, inverse = Self)]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Add(i64);

struct Model;

struct Consumer {
    offered: mpsc::UnboundedSender<Group>,
    release: Arc<Semaphore>,
    fail: bool,
}

struct Panics;

/* === Enums === */

#[harmos_runtime::transactions(state = i64, error = Refusal)]
#[derive(Clone, Debug)]
enum Changes {
    Add(Add),
}

#[harmos_runtime::records]
#[derive(Clone, Debug)]
enum Facts {
    Observed(Observed),
}

/* === Traits === */

/* === Implementations === */

impl Transaction<i64> for Add {
    type Error = &'static str;
    fn check(&self, _: &i64) -> Result<(), Self::Error> {
        Ok(())
    }
    fn records(&self, _: &i64, records: &mut Records) {
        records.push(Observed(self.0.to_string())).unwrap();
    }
    fn apply(&self, state: &mut i64) {
        *state += self.0;
    }
}

impl harmos_runtime::state::Invert<i64> for Add {
    fn invert(&self, _: &i64) -> Self {
        Add(-self.0)
    }
}

impl Application for Model {
    type State = i64;
    type Transaction = Changes;
    type Record = Facts;
    type Streams = ();
    type Resources = ();
}

impl JournalCapture<Model> for Consumer {
    async fn capture(&mut self, entries: Vec<Arc<Entry<Recorded<Model>>>>, frontier: Position) -> Result<(), CaptureError> {
        self.offered.send((entries, frontier)).unwrap();
        self.release.acquire().await.unwrap().forget();
        if self.fail { Err(CaptureError::Failed) } else { Ok(()) }
    }
}

impl JournalCapture<Model> for Panics {
    async fn capture(&mut self, _: Vec<Arc<Entry<Recorded<Model>>>>, _: Position) -> Result<(), CaptureError> {
        panic!("capture worker failed");
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn open(pending: u32, max: u32, fail: bool, tail: usize) -> (Runtime<Model>, mpsc::UnboundedReceiver<Group>, Arc<Semaphore>) {
    let (offered, observed) = mpsc::unbounded_channel();
    let release = Arc::new(Semaphore::new(0));
    let runtime = Runtime::<Model>::builder(0, ())
        .register(Changes::catalog())
        .register(Facts::catalog())
        .tail(tail)
        .require_capture(
            CaptureLimits { pending_bytes: pending, max_group_bytes: max },
            Consumer { offered, release: release.clone(), fail },
        )
        .unwrap()
        .runtime()
        .await
        .unwrap();
    (runtime, observed, release)
}

async fn group(observed: &mut mpsc::UnboundedReceiver<Group>) -> Group {
    observed.recv().await.unwrap()
}

#[tokio::test]
async fn owned_transaction_survives_capture_after_tail_eviction() {
    let (runtime, mut observed, release) = open(4096, 4096, false, 1).await;
    let owner = Arc::new(Changes::Add(Add(1)));
    let retained = Arc::downgrade(&owner);
    let transaction = harmos_runtime::operations::Response::from_owner(owner);
    runtime.state.commit_owned(transaction).await.unwrap();
    let (first, _) = group(&mut observed).await;

    runtime.state.commit(Add(2)).await.unwrap();
    release.add_permits(1);
    let (second, _) = group(&mut observed).await;
    assert!(retained.upgrade().is_some());
    let Recorded::Transaction(transaction) = &first[0].value else {
        panic!("expected transaction");
    };
    let original = retained.upgrade().unwrap();
    assert!(std::ptr::eq(transaction.as_ref(), original.as_ref()));
    drop(original);

    drop(first);
    assert!(retained.upgrade().is_none());
    release.add_permits(1);
    drop(second);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn capture_preserves_evicted() {
    // Each 600-byte record charges >600 bytes; two cannot fit in 1,400.
    let (runtime, mut observed, release) = open(1400, 1400, false, 1).await;
    let first = runtime.records.publish(Observed("x".repeat(600))).await.unwrap();
    let (held, frontier) = group(&mut observed).await;
    assert_eq!(frontier, first.frontier);
    let journal = runtime.state.clone();
    let second =
        tokio::spawn(async move { harmos_runtime::records::RecordManager::new(journal.clone()).publish(Observed("y".repeat(600))).await });
    tokio::task::yield_now().await;
    assert!(!second.is_finished(), "admission waits for capture capacity");
    assert_eq!(runtime.state.applied(), Position(1));
    // Reads remain available while admission is awaiting capacity.
    assert_eq!(runtime.state.state().read(|state| *state).await.unwrap().value, 0);
    assert_eq!(held[0].position, Position(1));
    release.add_permits(1);
    assert_eq!(second.await.unwrap().unwrap().position, Position(2));
    let (next, _) = group(&mut observed).await;
    assert_eq!(next[0].position, Position(2));
    assert_eq!(held[0].position, Position(1), "direct ownership survives tail eviction");
    assert!(runtime.state.query::<Facts>(Position::ORIGIN).await.next().unwrap().is_err());
    release.add_permits(1);
    runtime.state.wait_until_captured(Position(2)).await.unwrap();
    assert_eq!(runtime.stop().await.unwrap(), Position(2));
}

#[tokio::test]
async fn stop_awaits_groups() {
    let (runtime, mut observed, release) = open(32_000, 8000, false, 1).await;
    let principal = PrincipalId::new("capture");
    let journal = runtime.state.as_principal(principal.clone());
    let commit = journal.commit(Add(5)).await.unwrap();
    assert_eq!((commit.position, commit.frontier), (Position(1), Position(2)));
    harmos_runtime::records::RecordManager::new(journal.clone()).publish(Observed("typed".into())).await.unwrap();
    harmos_runtime::records::RecordManager::new(journal.clone())
        .publish_system(SystemFact {
            source: FactSource::Sidecar("probe".into()),
            name: "sample".into(),
            version: 1,
            payload: vec![1, 2, 3],
        })
        .await
        .unwrap();
    journal.undo(principal.clone(), Focus::all()).await.unwrap();
    journal.redo(principal, Focus::all()).await.unwrap();
    drop(journal);
    let runtime = Arc::new(runtime);
    let stopping = tokio::spawn({
        let runtime = Arc::clone(&runtime);

        async move { runtime.stop().await.unwrap() }
    });
    let mut positions = Vec::new();
    for expected in [2, 3, 4, 6, 8] {
        let (entries, frontier) = group(&mut observed).await;
        assert_eq!(frontier, Position(expected));
        if entries.len() == 2 {
            assert_eq!(entries[0].metadata.commit, Some((entries[0].position, frontier)));
            assert_eq!(entries[1].metadata.causation, Some(harmos_runtime::state::CausationId(entries[0].position)));
        }
        if expected == 4 {
            let Recorded::System(fact) = &entries[0].value else { panic!("system fact lost") };
            assert_eq!(fact.identify().0.as_str(), "harmos:sidecar/probe/sample");
            assert_eq!(fact.payload, vec![1, 2, 3]);
        }
        positions.extend(entries.iter().map(|entry| entry.position.0));
        assert!(!stopping.is_finished(), "stop waits for every group");
        release.add_permits(1);
    }
    assert_eq!(positions, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(stopping.await.unwrap(), Position(8));
}

#[tokio::test]
async fn failure_wakes_waiters() {
    let (runtime, mut observed, release) = open(1400, 1400, true, 1).await;
    runtime.records.publish(Observed("x".repeat(600))).await.unwrap();
    group(&mut observed).await;
    let journal = runtime.state.clone();
    let pending =
        tokio::spawn(async move { harmos_runtime::records::RecordManager::new(journal.clone()).publish(Observed("y".repeat(600))).await });
    tokio::task::yield_now().await;
    assert!(!pending.is_finished(), "admission waits for capture capacity");
    release.add_permits(1);
    assert!(matches!(pending.await.unwrap(), Err(Error::CaptureFailed)));
    assert_eq!(runtime.state.applied(), Position(1));
    assert_eq!(runtime.state.wait_until_captured(Position(1)).await, Err(CaptureError::Failed));
    assert_eq!(runtime.state.capture_status(), Err(CaptureError::Failed));
    assert!(matches!(runtime.records.publish(Observed("later".into())).await, Err(Error::CaptureFailed)));
    runtime.stop().await.unwrap();

    assert_eq!(
        runtime.state.capture_status(),
        Err(CaptureError::Failed),
        "a stop reports the frontier; capture health stays answerable after it"
    );
}

#[tokio::test]
async fn oversize_never_applies() {
    let (runtime, mut observed, _) = open(1, 1, false, 1).await;
    assert!(matches!(runtime.state.commit(Add(9)).await, Err(Error::Preparation(PreparationError::Limit))));
    assert_eq!(runtime.state.state().read(|state| *state).await.unwrap().value, 0);
    assert_eq!(runtime.state.applied(), Position::ORIGIN);
    assert!(observed.try_recv().is_err());
    assert_eq!(runtime.stop().await.unwrap(), Position::ORIGIN);
}

#[tokio::test]
async fn queued_groups_outlive() {
    let (runtime, mut observed, release) = open(2_000_000, 4000, false, 1024).await;
    // More independent publications than the normal 1,024-entry tail, with
    // the first callback stalled and every later accepted group still pending.
    for index in 0..1100 {
        runtime.records.publish(Observed(index.to_string())).await.unwrap();
    }
    assert_eq!(runtime.state.applied(), Position(1100));
    release.add_permits(1100);
    for expected in 1..=1100 {
        let (entries, frontier) = group(&mut observed).await;
        assert_eq!(frontier, Position(expected));
        assert_eq!(entries[0].position, Position(expected));
        let Recorded::Record(Facts::Observed(Observed(value))) = &entries[0].value else { panic!("typed record lost") };
        assert_eq!(value, &(expected - 1).to_string());
    }
    assert_eq!(runtime.stop().await.unwrap(), Position(1100));
}

#[tokio::test]
async fn panic_fails_capture() {
    let runtime = Runtime::<Model>::builder(0, ())
        .register(Facts::catalog())
        .require_capture(CaptureLimits { pending_bytes: 4096, max_group_bytes: 4096 }, Panics)
        .unwrap()
        .runtime()
        .await
        .unwrap();
    runtime.records.publish(Observed("accepted".into())).await.unwrap();
    assert_eq!(runtime.state.wait_until_captured(Position(1)).await, Err(CaptureError::Failed));
    runtime.stop().await.unwrap();

    assert_eq!(
        runtime.state.capture_status(),
        Err(CaptureError::Failed),
        "a panicking capture worker is a capture failure the journal keeps"
    );
}

/* === Tests === */
