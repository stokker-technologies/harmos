/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// State Journal Admission Story

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::task::Poll;

// Third Party Imports

// Local Imports
use super::fixture::*;
use super::waveforms;
use harmos::state::Envelope;
use harmos::{Application, Message};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Refusal = std::convert::Infallible;

/* === Structs === */
struct Model;

#[harmos::transaction(id = "admission.advance", version = 1, irreversible)]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Advance {}

#[harmos::record(id = "admission.observed", version = 1)]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Observed {
    before: u64,
}

struct Projection;

/* === Enums === */
#[harmos::transactions(state = u64, error = Refusal)]
#[derive(Clone)]
enum Transactions {
    Advance(Advance),
}

#[harmos::records]
#[derive(Clone)]
enum Records {
    Observed(Observed),
}

/* === Traits === */

/* === Implementations === */
impl Application for Model {
    type State = u64;
    type Transaction = Transactions;
    type Record = Records;
    type Streams = ();
    type Resources = ();
}

impl harmos::state::Transaction<u64> for Advance {
    type Error = std::convert::Infallible;
    fn check(&self, _: &u64) -> Result<(), Self::Error> {
        Ok(())
    }
    fn records(&self, state: &u64, output: &mut harmos::state::Records) {
        output.push(Observed { before: *state }).unwrap();
    }
    fn apply(&self, state: &mut u64) {
        *state += 1;
    }
}

impl RecordProjection for Projection {
    fn project<'a>(
        &'a self,
        entry: &'a Envelope<&serde_json::Value>,
        output: &'a mut ProjectedRecords<'_>,
    ) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let schema = waveforms::schema();
            let item = Identifier::new(format!("observed-{}", entry.position.0)).unwrap();
            let payload = waveforms::Sample {
                phase: 0,
                ordinal: entry.position.0,
                value: 1.0,
            }
            .encode_to_vec();
            output.push(record(&schema, &item, &payload)).await
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */
// A Query Pins A Snapshot Through The Worker, Holding It At The Backend View Gate.
fn gate(at: StorageCursor) -> harmos::storage::operations::Query {
    harmos::storage::operations::Query {
        at,
        source: harmos::storage::query::Source::Files(harmos::storage::query::FileSelection {
            scope: None,
            item: None,
        }),
        transforms: Vec::new(),
        slice: harmos::storage::query::RowSlice::default(),
    }
}

/* === Tests === */

#[tokio::test]
async fn ready_journal_bypasses_unrelated_primary_backlog_in_order() {
    let f = Fixture::new(config()).await;
    let durable = f.storage.cursor_saved();

    // Hold The Worker At A Snapshot Gate Until Both Domains Have Been Admitted.
    f.data.control.block_view.store(true, Ordering::SeqCst);
    let mut acquiring = Box::pin(f.storage.query(gate(f.storage.cursor_saved())));
    tokio::select! {
        result = &mut acquiring => { let _ = result; panic!("view must reach its gate") },
        _ = f.data.control.view_entered.notified() => {}
    }
    write(&f.storage, "unrelated-measurement").await;
    let first = f.storage.write_recovery(&[b"state-one"]).await.unwrap();
    let second = f.storage.write_recovery(&[b"state-two"]).await.unwrap();
    let mut flushing = Box::pin(f.storage.flush_recovery());
    assert_pending(flushing.as_mut()).await;
    f.data.control.block_write.store(true, Ordering::SeqCst);
    f.data.control.block_view.store(false, Ordering::SeqCst);
    f.data.control.view_release.notify_one();
    let view = acquiring.await.unwrap();

    // A Backend Write Gate Makes Incorrect Primary Selection Observable.
    tokio::select! {
        result = &mut flushing => assert_eq!(result.unwrap(), second),
        _ = f.data.control.write_entered.notified() => panic!("ready journal was held behind unrelated primary data")
    }
    assert_eq!(f.storage.cursor_saved(), durable);
    assert_eq!(
        f.recovery.control.writes.lock().unwrap().as_slice(),
        &[first.sequence, second.sequence]
    );
    {
        let entries = f.recovery.entries.lock().unwrap();
        assert_eq!(entries[&first.sequence].payload, b"state-one");
        assert_eq!(entries[&second.sequence].payload, b"state-two");
        assert!(entries.values().all(|entry| entry.required_data == durable));
    }
    drop(view);
    f.data.control.block_write.store(false, Ordering::SeqCst);
    f.data.control.write_release.notify_one();
    f.storage.flush().await.unwrap();
    drop(flushing);
    f.finish().await;
}

async fn assert_pending<F: Future>(mut future: Pin<&mut F>) {
    poll_fn(|context| {
        assert!(
            future.as_mut().poll(context).is_pending(),
            "gated future completed unexpectedly"
        );
        Poll::Ready(())
    })
    .await;
}

#[tokio::test]
async fn projected_journal_waits_for_real_primary_dependency() {
    let data = Arc::new(Data::new(history()));
    let state = Arc::new(Recovery::new(history()));
    let policy = config();
    let storage: StorageManager = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: state.clone(),
        },
        policy.clone(),
    )
    .unwrap();
    let runtime = Runtime::<Model>::builder(0, ())
        .register(Transactions::catalog())
        .register(Records::catalog())
        .managed_state(ReadLimits {
            rows: NonZeroUsize::new(1).unwrap(),
            bytes: NonZeroUsize::new(64 * 1024).unwrap(),
        })
        .store_records(
            ReadLimits {
                rows: NonZeroUsize::new(4).unwrap(),
                bytes: NonZeroUsize::new(16 * 1024).unwrap(),
            },
            Projection,
        )
        .runtime_with_storage(&storage)
        .await
        .unwrap();
    storage
        .write_schema(&waveforms::schema(), &waveforms::definition())
        .await
        .unwrap();
    let before = storage.flush().await.unwrap();

    // Keep Both Transactions In The Queue Until The Recovery Barrier Exists.
    data.control.block_view.store(true, Ordering::SeqCst);
    let mut acquiring = Box::pin(storage.query(gate(storage.cursor_saved())));
    tokio::select! {
        result = &mut acquiring => { let _ = result; panic!("view must reach its gate") },
        _ = data.control.view_entered.notified() => {}
    }
    runtime.state.commit(Advance {}).await.unwrap();
    let first_dependency = storage.cursor_accepted();
    runtime.state.commit(Advance {}).await.unwrap();
    let final_dependency = storage.cursor_accepted();
    assert!(final_dependency.sequence > first_dependency.sequence);
    assert!(first_dependency.sequence > before.sequence);
    let mut flushing = Box::pin(storage.flush_recovery());
    assert_pending(flushing.as_mut()).await;
    data.control.block_write.store(true, Ordering::SeqCst);
    state.control.block_write.store(true, Ordering::SeqCst);
    data.control.block_view.store(false, Ordering::SeqCst);
    data.control.view_release.notify_one();
    let view = acquiring.await.unwrap();

    tokio::select! {
        result = &mut flushing => panic!("primary dependency must be blocked: {result:?}"),
        _ = state.control.write_entered.notified() => panic!("journal preceded its required data"),
        _ = data.control.write_entered.notified() => {}
    }
    assert_eq!(storage.cursor_saved(), before);
    assert!(state.control.writes.lock().unwrap().is_empty());
    data.control.block_write.store(false, Ordering::SeqCst);
    data.control.write_release.notify_one();
    tokio::select! {
        result = &mut flushing => panic!("journal must reach its gate: {result:?}"),
        _ = state.control.write_entered.notified() => {}
    }
    assert!(storage.cursor_saved().covers(&first_dependency).unwrap());
    state.control.block_write.store(false, Ordering::SeqCst);
    state.control.write_release.notify_one();
    let saved = flushing.await.unwrap();
    assert_eq!(state.control.writes.lock().unwrap().as_slice(), &[1, 2]);
    {
        let entries = state.entries.lock().unwrap();
        assert_eq!(entries[&1].required_data, first_dependency);
        assert_eq!(entries[&2].required_data, final_dependency);
        assert_eq!(saved.sequence, 2);
    }
    drop(view);
    storage.close().await.unwrap();
    runtime.stop().await.unwrap();
}
