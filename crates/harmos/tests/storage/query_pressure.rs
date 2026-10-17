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
use std::task::Poll;

// Third Party Imports
use arrow_array::{Float64Array, UInt64Array};
use tokio::sync::Notify;

// Local Imports
use super::backend::{Data, history, storage_settings};
use super::fixture::state_settings;
use super::support::{Bank, Ledger, catalog};
use super::waveforms;
use harmos::Runtime;
use harmos::storage::operations::Query;
use harmos::storage::query::scalar::ScalarValue;
use harmos::storage::query::transform::{
    Transform,
    aggregate::{Aggregate, Measure},
    filter::{Clause, Compare, Condition, Filter, Term},
};
use harmos::storage::query::{QueryArrow, QueryReader, RowSlice, Source, StreamSelection};
use harmos::storage::*;

/* === Constants === */
const CONTENTION_BUDGET: usize = 1024 * 1024;
const CONTENTION_PERIODS: usize = 2;

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
async fn external_pressure_reloads_uncommitted_page_once_and_cancellation_releases() {
    // Each Branch Owns Its Backend, Explicit Budget And Complete Lifecycle.
    for cancel in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let mut settings = storage_settings();
        settings.request.max_active_queries = NonZeroUsize::new(1).unwrap();
        let capacity = settings.budget.read_memory_bytes.get();
        let data = Arc::new(Data::<()>::journaled(directory.path(), history()).unwrap());
        let storage: StorageManager = StorageManager::new(
            StorageBackends {
                primary: data.clone(),
                state: Arc::new(harmos::storage::state::file::FileBackend::new(
                    directory.path().join("state"),
                    history(),
                    state_settings(),
                )),
            },
            settings,
        )
        .unwrap();
        let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
            .register(catalog())
            .runtime_with_storage(&storage)
            .await
            .unwrap();
        let schema = waveforms::schema();
        storage
            .write_schema(&schema, &waveforms::definition())
            .await
            .unwrap();
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
                            condition: Condition::Compare {
                                op: Compare::Eq,
                                value: ScalarValue::Unsigned(0),
                            },
                        }],
                    }],
                }),
                Transform::Aggregate(Aggregate {
                    groups: vec![],
                    measures: vec![
                        Measure {
                            field: "payload.value".into(),
                            statistic: harmos::Aggregate::Count,
                            name: "count".into(),
                        },
                        Measure {
                            field: "payload.value".into(),
                            statistic: harmos::Aggregate::Rms,
                            name: "rms".into(),
                        },
                    ],
                }),
            ],
            slice: RowSlice {
                offset: 0,
                limit: None,
            },
        };
        let mut reader = storage.query(query()).await.unwrap();
        assert!(matches!(
            reader.next().await.unwrap(),
            Some(QueryArrow::Schema(_))
        ));
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
        let external = storage
            .reserve_arrow_memory(capacity - occupied - overhead)
            .unwrap();
        assert_eq!(storage.status().read_bytes, capacity);
        data.control.read_release.notify_one();

        // Poll Through Conversion Refusal Into WorkingCredit Admission. No
        // Reducer Mutation Or Cursor Commit May Survive This Abandoned Page.
        assert!(matches!(
            std::future::poll_fn(|cx| Poll::Ready(std::future::Future::poll(next.as_mut(), cx)))
                .await,
            Poll::Pending
        ));
        assert!(
            storage.status().read_bytes < capacity,
            "raw page must be released before waiting"
        );
        assert!(storage.status().read_bytes >= prepared);

        if cancel {
            drop(next);
            assert!(
                reader.next().await.is_err(),
                "interrupted advancement poisons the retained reader"
            );
            assert!(reader.completion().is_err());
            drop(reader);
            drop(external);
            data.control.block_read.store(false, Ordering::SeqCst);
            // Opening A Successor Also Exercises Release Of The Query Slot.
            let mut successor = storage.query(query()).await.unwrap();
            assert!(matches!(
                successor.next().await.unwrap(),
                Some(QueryArrow::Schema(_))
            ));
            assert_result(successor.next().await.unwrap().unwrap());
            assert!(successor.next().await.unwrap().is_none());
            drop(successor);
        } else {
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
        }
        storage.close().await.unwrap();
        assert_eq!(storage.status().read_bytes, baseline);
        runtime.stop().await.unwrap();
    }
}

fn assert_result(output: QueryArrow) {
    let QueryArrow::Batch(batch) = output else {
        panic!("expected aggregate batch")
    };
    let batch = batch.as_record_batch();
    assert_eq!(batch.num_rows(), 1);
    let count = batch
        .column_by_name("count")
        .unwrap()
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    let rms = batch
        .column_by_name("rms")
        .unwrap()
        .as_any()
        .downcast_ref::<Float64Array>()
        .unwrap();
    assert_eq!(
        count.value(0),
        waveforms::SAMPLES_PER_PERIOD as u64,
        "retry must neither omit nor duplicate rows"
    );
    assert!((rms.value(0) - waveforms::AMPLITUDE / 2.0_f64.sqrt()).abs() < 1e-12);
}

#[tokio::test]
async fn cancelled_schema_admission_releases_query_slot() {
    let directory = tempfile::tempdir().unwrap();
    let mut settings = storage_settings();
    settings.request.max_active_queries = NonZeroUsize::MIN;
    let capacity = settings.budget.read_memory_bytes.get();
    let storage: StorageManager = StorageManager::new(
        StorageBackends {
            primary: Arc::new(Data::<()>::journaled(directory.path(), history()).unwrap()),
            state: Arc::new(harmos::storage::state::file::FileBackend::new(
                directory.path().join("state"),
                history(),
                state_settings(),
            )),
        },
        settings,
    )
    .unwrap();
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .runtime_with_storage(&storage)
        .await
        .unwrap();
    let at = storage.flush().await.unwrap();
    let query = || Query {
        at: at.clone(),
        source: Source::Files(harmos::storage::query::FileSelection {
            scope: None,
            item: None,
        }),
        transforms: vec![],
        slice: RowSlice {
            offset: 0,
            limit: None,
        },
    };
    let mut reader = storage.query(query()).await.unwrap();
    let occupied = storage.status().read_bytes;
    let probe = storage.reserve_arrow_memory(0).unwrap();
    let overhead = storage.status().read_bytes - occupied;
    drop(probe);
    let external = storage
        .reserve_arrow_memory(capacity - occupied - overhead)
        .unwrap();
    let mut next = Box::pin(reader.next());
    assert!(matches!(
        std::future::poll_fn(|cx| Poll::Ready(std::future::Future::poll(next.as_mut(), cx))).await,
        Poll::Pending
    ));
    drop(next);
    assert!(reader.next().await.is_err());
    assert!(reader.completion().is_err());
    drop(reader);
    drop(external);

    let mut successor = storage.query(query()).await.unwrap();
    assert!(matches!(
        successor.next().await.unwrap(),
        Some(QueryArrow::Schema(_))
    ));
    assert!(successor.next().await.unwrap().is_none());
    assert_eq!(successor.completion().unwrap().rows, 0);
    drop(successor);
    storage.close().await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn contending_captures_never_deadlock() {
    // Each Capture Retains Every Batch, As A Report Materialization Does, And
    // Announces A Refusal Before Releasing What It Holds.
    async fn capture(
        mut reader: QueryReader,
        mut kept: Vec<QueryArrow>,
        refused: &Notify,
    ) -> harmos::Result<usize> {
        loop {
            match reader.next().await {
                Ok(Some(output)) => kept.push(output),
                Ok(None) => return Ok(kept.len()),
                Err(error) => {
                    refused.notify_one();
                    return Err(error);
                }
            }
        }
    }

    let mut settings = storage_settings();
    settings.budget.read_memory_bytes = NonZeroUsize::new(CONTENTION_BUDGET).unwrap();
    let f = super::fixture::Fixture::new(settings).await;
    let schema = waveforms::schema();
    f.storage
        .write_schema(&schema, &waveforms::definition())
        .await
        .unwrap();
    let samples = waveforms::samples(CONTENTION_PERIODS);
    let inputs = waveforms::inputs(&samples);
    let borrowed: Vec<_> = inputs.iter().map(StreamInput::borrowed).collect();
    f.storage.write_stream(&borrowed).await.unwrap();
    let at = f.storage.flush().await.unwrap();
    let baseline = f.storage.status().read_bytes;
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
        transforms: vec![],
        slice: RowSlice::default(),
    };

    // Alone, One Capture Fits The Budget.
    let mut alone = f.storage.query(query()).await.unwrap();
    let mut batches = 0;
    while alone.next().await.unwrap().is_some() {
        batches += 1;
    }
    drop(alone);
    assert_eq!(f.storage.status().read_bytes, baseline);

    // Both Captures Hold Their Plan And Schema, And The Rest Of The Pool Is
    // Taken, So Each Must Wait To Grow While Retaining What The Other Needs.
    let mut first = f.storage.query(query()).await.unwrap();
    let mut second = f.storage.query(query()).await.unwrap();
    let first_kept = vec![first.next().await.unwrap().unwrap()];
    let second_kept = vec![second.next().await.unwrap().unwrap()];
    assert!(matches!(first_kept[0], QueryArrow::Schema(_)));
    assert!(matches!(second_kept[0], QueryArrow::Schema(_)));
    let occupied = f.storage.status().read_bytes;
    let probe = f.storage.reserve_arrow_memory(0).unwrap();
    let overhead = f.storage.status().read_bytes - occupied;
    drop(probe);
    let mut external = Some(
        f.storage
            .reserve_arrow_memory(CONTENTION_BUDGET - occupied - overhead)
            .unwrap(),
    );
    assert_eq!(f.storage.status().read_bytes, CONTENTION_BUDGET);

    // The Other Pool Holder Leaves Once A Capture Is Refused; Only Then Can Any Grow.
    let refused = Notify::new();
    let release = async {
        refused.notified().await;
        external.take();
    };
    let (first, second, ()) = tokio::join!(
        capture(first, first_kept, &refused),
        capture(second, second_kept, &refused),
        release
    );
    let outcomes = [first, second];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, Ok(count) if *count == batches))
            .count(),
        1,
        "{outcomes:?}"
    );
    assert!(
        outcomes
            .iter()
            .any(|outcome| matches!(outcome, Err(harmos::Error::ResourceExhausted { .. }))),
        "{outcomes:?}"
    );
    assert_eq!(f.storage.status().read_bytes, baseline);
    f.finish().await;
}
