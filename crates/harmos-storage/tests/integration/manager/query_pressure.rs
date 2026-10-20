/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Query Pressure And Page Rollback Stories

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::{Arc, atomic::Ordering};

// Third Party Imports
use arrow_array::{Float64Array, UInt64Array};

// Local Imports
use super::backend::{Data, history, storage_settings};
use super::fixture::state_settings;
use super::waveforms;
use harmos_domain::operations::storage::Query;
use harmos_storage::query::scalar::ScalarValue;
use harmos_storage::query::transform::{
    Transform,
    aggregate::{Aggregate, Measure},
    filter::{Clause, Compare, Condition, Filter, Term},
};
use harmos_storage::query::{QueryArrow, QueryReader, RecordSelection, RowSlice, Source, StreamSelection};
use harmos_storage::*;

/* === Constants === */

const CONTENTION_BUDGET: usize = 4 * 1024 * 1024;
const CONTENTION_PERIODS: usize = 13;
const FITTING_PERIODS: usize = 6;
const CONTENTION_QUERIES: usize = 64;
const SMALL_READERS: usize = 15;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn pressure_reloads_page() {
    // Each Story Owns Its Backend, Explicit Budget And Complete Lifecycle.
    let directory = tempfile::tempdir().unwrap();
    let mut settings = storage_settings();
    settings.request.max_active_queries = NonZeroUsize::new(1).unwrap();
    let capacity = settings.budget.read_memory_bytes.get();
    let data = Arc::new(Data::<()>::journaled(directory.path(), history()).unwrap());
    let storage: StorageManager = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: Arc::new(harmos_storage::state::file::FileBackend::new(directory.path().join("state"), history(), state_settings())),
        },
        settings,
    )
    .unwrap();
    super::fixture::start(&storage).await;
    let schema = waveforms::schema();
    storage.write_schema(&schema, &waveforms::definition()).await.unwrap();
    let samples = waveforms::samples(1);
    let inputs = waveforms::inputs(&samples);
    let borrowed: Vec<_> = inputs.iter().map(StreamInput::borrowed).collect();
    storage.write_stream(&borrowed).await.unwrap();
    let at = storage.flush().await.unwrap();
    let baseline = storage.status().read_bytes;
    let query = || Query {
        at: at.clone(),
        source: Source::Stream(StreamSelection {
            schema: schema.clone(),
            scope: None,
            item: None,
            stream: None,
            source_start: None,
            source_end: None,
            after: 0,
        }),
        transforms: vec![
            Transform::Filter(Filter {
                clauses: vec![Clause {
                    terms: vec![Term {
                        field: "payload.phase".into(),
                        negate: false,
                        condition: Condition::Compare { op: Compare::Eq, value: ScalarValue::Unsigned(0) },
                    }],
                }],
            }),
            Transform::Aggregate(Aggregate {
                groups: vec![],
                measures: vec![
                    Measure { field: "payload.value".into(), statistic: harmos_domain::Aggregate::Count, name: "count".into() },
                    Measure { field: "payload.value".into(), statistic: harmos_domain::Aggregate::Rms, name: "rms".into() },
                ],
            }),
        ],
        slice: RowSlice { offset: 0, limit: None },
    };
    let mut reader = storage.query(query()).await.unwrap();
    assert!(matches!(reader.next().await.unwrap(), Some(QueryArrow::Schema(_))));
    let prepared = storage.status().read_bytes;

    // Stall A Native Page After Its Original Reservation Was Acquired.
    data.control.block_read.store(true, Ordering::SeqCst);
    let mut next = Box::pin(reader.next());
    tokio::select! {
        result = &mut next => { let _ = result; panic!("native page must be gated") },
        _ = data.control.read_entered.notified() => {}
    }
    // Measure The Public Reservation Envelope Instead Of Copying Its Private Overhead.
    let occupied = storage.status().read_bytes;
    let probe = storage.reserve_arrow_memory(0).unwrap();
    let overhead = storage.status().read_bytes - occupied;
    drop(probe);
    let external = storage.reserve_arrow_memory(capacity - occupied - overhead).unwrap();
    assert_eq!(storage.status().read_bytes, capacity);
    data.control.read_release.notify_one();

    // Poll Through Conversion Refusal Into WorkingCredit Admission. No
    // Reducer Mutation Or Cursor Commit May Survive This Abandoned Page.
    assert!(pending(next.as_mut()).await);
    assert!(storage.status().read_bytes < capacity, "raw page must be released before waiting");
    assert!(storage.status().read_bytes >= prepared);

    // The Gate Must Be Entered A Second Time: A Native Page Is Reloaded
    // At Its Uncommitted Cursor, Rather Than Reusing Freed Input.
    drop(external);
    tokio::select! {
        result = &mut next => { let _ = result; panic!("reload must reach the backend gate") },
        _ = data.control.read_entered.notified() => {}
    }
    data.control.block_read.store(false, Ordering::SeqCst);
    data.control.read_release.notify_one();
    assert_result(next.await.unwrap().unwrap());
    assert!(reader.next().await.unwrap().is_none());
    assert_eq!(reader.completion().unwrap().at, at);
    drop(reader);
    storage.close().await.unwrap();
    assert_eq!(storage.status().read_bytes, baseline);
}

fn assert_result(output: QueryArrow) {
    let QueryArrow::Batch(batch) = output else { panic!("expected aggregate batch") };
    let batch = batch.as_record_batch();
    assert_eq!(batch.num_rows(), 1);
    let count = batch.column_by_name("count").unwrap().as_any().downcast_ref::<UInt64Array>().unwrap();
    let rms = batch.column_by_name("rms").unwrap().as_any().downcast_ref::<Float64Array>().unwrap();
    assert_eq!(count.value(0), waveforms::SAMPLES_PER_PERIOD as u64, "retry must neither omit nor duplicate rows");
    assert!((rms.value(0) - waveforms::AMPLITUDE / 2.0_f64.sqrt()).abs() < 1e-12);
}

#[tokio::test]
async fn cancelled_advance_poisons() {
    let directory = tempfile::tempdir().unwrap();
    let mut settings = storage_settings();
    settings.request.max_active_queries = NonZeroUsize::MIN;
    let capacity = settings.budget.read_memory_bytes.get();
    let storage: StorageManager = StorageManager::new(
        StorageBackends {
            primary: Arc::new(Data::<()>::journaled(directory.path(), history()).unwrap()),
            state: Arc::new(harmos_storage::state::file::FileBackend::new(directory.path().join("state"), history(), state_settings())),
        },
        settings,
    )
    .unwrap();
    super::fixture::start(&storage).await;
    let at = storage.flush().await.unwrap();
    let query = || Query {
        at: at.clone(),
        source: Source::Files(harmos_storage::query::FileSelection { scope: None, item: None }),
        transforms: vec![],
        slice: RowSlice { offset: 0, limit: None },
    };
    let mut reader = storage.query(query()).await.unwrap();
    let occupied = storage.status().read_bytes;
    let probe = storage.reserve_arrow_memory(0).unwrap();
    let overhead = storage.status().read_bytes - occupied;
    drop(probe);
    let external = storage.reserve_arrow_memory(capacity - occupied - overhead).unwrap();
    let mut next = Box::pin(reader.next());
    assert!(pending(next.as_mut()).await);
    drop(next);
    assert!(reader.next().await.is_err());
    assert!(reader.completion().is_err());
    drop(reader);
    drop(external);

    let mut successor = storage.query(query()).await.unwrap();
    assert!(matches!(successor.next().await.unwrap(), Some(QueryArrow::Schema(_))));
    assert!(successor.next().await.unwrap().is_none());
    assert_eq!(successor.completion().unwrap().rows, 0);
    drop(successor);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn captures_refuse_cycles() {
    // Each Capture Alone Fits, But Together They Exceed The Budget.
    let (f, query) = loaded(CONTENTION_PERIODS).await;
    let (batches, retained) = alone(&f, &query).await;
    assert!(retained * 2 > CONTENTION_BUDGET, "capture retains {retained} of {CONTENTION_BUDGET} bytes");

    // Both Hold A Third Of Their Capture Before Advancing Together: One Must Be
    // Refused Rather Than Both Waiting On Each Other's Retained Batches.
    let mut first = f.storage.query(query.clone()).await.unwrap();
    let mut second = f.storage.query(query.clone()).await.unwrap();
    let mut first_kept = Vec::new();
    let mut second_kept = Vec::new();
    for _ in 0..batches / 3 {
        first_kept.push(first.next().await.unwrap().unwrap());
    }
    for _ in 0..batches / 3 {
        second_kept.push(second.next().await.unwrap().unwrap());
    }
    let outcomes = tokio::join!(capture(first, first_kept), capture(second, second_kept));
    let outcomes = [outcomes.0, outcomes.1];
    assert_eq!(outcomes.iter().filter(|outcome| matches!(outcome, Ok(count) if *count == batches)).count(), 1, "{outcomes:?}");
    assert!(outcomes.iter().any(|outcome| matches!(outcome, Err(harmos_storage::Error::ResourceExhausted { .. }))), "{outcomes:?}");
    assert_eq!(f.storage.status().read_bytes, 0);
    f.finish().await;
}

#[tokio::test]
async fn fitting_captures_complete() {
    // Two Captures That Fit Together Advance In Alternation Without Refusal.
    let (f, query) = loaded(FITTING_PERIODS).await;
    let (batches, retained) = alone(&f, &query).await;
    assert!(retained * 3 < CONTENTION_BUDGET, "capture retains {retained} of {CONTENTION_BUDGET} bytes");
    let first = f.storage.query(query.clone()).await.unwrap();
    let second = f.storage.query(query.clone()).await.unwrap();
    let outcomes = tokio::join!(capture(first, Vec::new()), capture(second, Vec::new()));
    assert_eq!(outcomes.0.unwrap(), batches);
    assert_eq!(outcomes.1.unwrap(), batches);
    assert_eq!(f.storage.status().read_bytes, 0);
    f.finish().await;
}

#[tokio::test]
async fn settlement_preserves_readers() {
    let (f, query) = loaded(1).await;
    let inputs = waveforms::inputs(&waveforms::samples(1));
    let records = |range: std::ops::Range<usize>| inputs[range].iter().map(|input| input.record.borrowed()).collect::<Vec<_>>();
    let saved = f.storage.write_records(&records(0..9)).await.unwrap();
    f.storage.await_durable(&saved).await.unwrap();

    // A Query Holding Its Schema And A Record Reader Holding Its Description
    // Both Wait For Growth While The Pool Is Taken.
    let mut reader = f.storage.query(query).await.unwrap();
    assert!(matches!(reader.next().await.unwrap(), Some(QueryArrow::Schema(_))));
    let typed = f
        .storage
        .read_records::<waveforms::Sample>(RecordRead {
            at: saved,
            selection: RecordSelection { schema: waveforms::schema(), scope: None, item: None, source_start: None, source_end: None },
            after: 0,
        })
        .await
        .unwrap();
    let external = fill(&f);
    let mut waiting = Box::pin(reader.next());
    assert!(pending(waiting.as_mut()).await);
    let mut settlement = Box::pin(typed.collect());
    assert!(pending(settlement.as_mut()).await);

    // Further Records Settle Durably Meanwhile; Releasing The Pool Lets Both Readers Finish.
    let later = f.storage.write_records(&records(9..18)).await.unwrap();
    f.storage.await_durable(&later).await.unwrap();
    drop(external);
    let (read, settled) = tokio::join!(waiting, settlement);
    assert!(matches!(read.unwrap(), Some(QueryArrow::Batch(_))));
    assert_eq!(settled.unwrap().len(), 9);
    drop(reader);
    assert_eq!(f.storage.status().read_bytes, 0);
    f.finish().await;
}

#[tokio::test]
async fn small_readers_progress() {
    // A Report Capture Holds Most Of Its Rows While Small Readers Hold Only Their Schemas.
    let (f, query) = loaded(FITTING_PERIODS).await;
    let (batches, _) = alone(&f, &query).await;
    let mut report = f.storage.query(query.clone()).await.unwrap();
    let mut kept = Vec::new();
    for _ in 0..batches / 2 {
        kept.push(report.next().await.unwrap().unwrap());
    }
    let mut readers = Vec::new();
    for _ in 0..SMALL_READERS {
        let mut reader = f.storage.query(query.clone()).await.unwrap();
        assert!(matches!(reader.next().await.unwrap(), Some(QueryArrow::Schema(_))));
        readers.push(reader);
    }

    // With The Pool Taken, Every One Of Them Waits To Grow; None Is Refused.
    let external = fill(&f);
    let mut report = Box::pin(capture(report, kept));
    assert!(pending(report.as_mut()).await);
    let mut small = readers.into_iter().map(|reader| Box::pin(capture(reader, Vec::new()))).collect::<Vec<_>>();
    for reader in &mut small {
        assert!(pending(reader.as_mut()).await);
    }
    drop(external);
    assert_eq!(report.await.unwrap(), batches);
    // Their Schemas Were Already Delivered.
    for reader in small {
        assert_eq!(reader.await.unwrap(), batches - 1);
    }
    assert_eq!(f.storage.status().read_bytes, 0);
    f.finish().await;
}

// A Manager With A Bounded Read Budget And Saved Waveform Stream Rows.
async fn loaded(periods: usize) -> (super::fixture::Fixture, Query) {
    let mut settings = storage_settings();
    settings.budget.read_memory_bytes = NonZeroUsize::new(CONTENTION_BUDGET).unwrap();
    settings.request.max_active_queries = NonZeroUsize::new(CONTENTION_QUERIES).unwrap();
    let f = super::fixture::Fixture::new(settings).await;
    let schema = waveforms::schema();
    f.storage.write_schema(&schema, &waveforms::definition()).await.unwrap();
    let inputs = waveforms::inputs(&waveforms::samples(periods));
    let borrowed: Vec<_> = inputs.iter().map(StreamInput::borrowed).collect();
    f.storage.write_stream(&borrowed).await.unwrap();
    let at = f.storage.flush().await.unwrap();
    assert_eq!(f.storage.status().read_bytes, 0);
    let query = Query {
        at,
        source: Source::Stream(StreamSelection {
            schema,
            scope: None,
            item: None,
            stream: None,
            source_start: None,
            source_end: None,
            after: 0,
        }),
        transforms: vec![],
        slice: RowSlice::default(),
    };
    (f, query)
}

// One Capture Alone: Its Batch Count And The Bytes It Retains At The End.
async fn alone(f: &super::fixture::Fixture, query: &Query) -> (usize, usize) {
    let mut reader = f.storage.query(query.clone()).await.unwrap();
    let mut kept = Vec::new();
    while let Some(output) = reader.next().await.unwrap() {
        kept.push(output);
    }
    drop(reader);
    let retained = f.storage.status().read_bytes;
    let batches = kept.len();
    drop(kept);
    assert_eq!(f.storage.status().read_bytes, 0);
    (batches, retained)
}

// Retain Every Batch, As A Report Materialization Does, Yielding After Each.
async fn capture(mut reader: QueryReader, mut kept: Vec<QueryArrow>) -> harmos_storage::Result<usize> {
    while let Some(output) = reader.next().await? {
        kept.push(output);
        tokio::task::yield_now().await;
    }
    Ok(kept.len())
}

// Take Every Remaining Byte Of The Read Pool With A Non-Waiting Reservation.
fn fill(f: &super::fixture::Fixture) -> harmos_storage::arrow::RetainedMemory {
    let occupied = f.storage.status().read_bytes;
    let probe = f.storage.reserve_arrow_memory(0).unwrap();
    let overhead = f.storage.status().read_bytes - occupied;
    drop(probe);
    let external = f.storage.reserve_arrow_memory(CONTENTION_BUDGET - occupied - overhead).unwrap();
    assert_eq!(f.storage.status().read_bytes, CONTENTION_BUDGET);
    external
}

// Whether One Poll Leaves The Future Waiting.
async fn pending<F: std::future::Future>(future: std::pin::Pin<&mut F>) -> bool {
    let mut future = future;
    std::future::poll_fn(|cx| std::task::Poll::Ready(future.as_mut().poll(cx).is_pending())).await
}
