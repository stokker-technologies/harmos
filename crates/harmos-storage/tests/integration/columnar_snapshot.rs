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
use std::{
    future::{Future, poll_fn},
    num::{NonZeroU32, NonZeroUsize},
    sync::{Arc, atomic::Ordering},
    task::Poll,
    time::{Duration, UNIX_EPOCH},
};

// Third Party Imports
use arrow_array::{Float64Array, UInt64Array};

// Local Imports
use crate::manager::backend;
use crate::manager::fixture::{self, Fixture};
use harmos_domain::config::SchemaSettings;
use harmos_domain::schema::graph;
use harmos_domain::{Message, schema::Described};
use harmos_query::columnar::ArrowScan;
use harmos_query::columnar::Cancellation;
use harmos_query::columnar::ColumnKind;
use harmos_query::columnar::ColumnPredicate;
use harmos_query::columnar::ColumnSource;
use harmos_query::columnar::TimeBounds;
use harmos_query::{arrow::ArrowBatch, source};
use harmos_storage::query::StreamSelection;
use harmos_storage::query::columnar::list_column_rows;
use harmos_storage::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug, Default)]
struct Sample {
    time_ns: i64,
    value: f64,
    unused: Vec<u8>,
}

#[derive(harmos_domain::Message)]
struct Reading {
    amplitude: f64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

/* === Tests === */

#[tokio::test]
async fn saved_windows_match_fallback_while_live_writes_and_cancellation_advance() {
    // Open Both Backends And Declare One Stored Shape.
    let history = backend::history();
    let policy = backend::storage_settings();
    let primary = backend::Backend::new(history.clone());
    let fallback = Arc::new(backend::Backend::new(history.clone()));
    primary.create_session().await.unwrap();
    fallback.create_session().await.unwrap();
    let key = SchemaKey {
        id: Identifier::new("sample").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = graph::Schema::from_declaration(Sample::DEFINITION, &SchemaSettings::default()).unwrap();
    let schema = StorageSchema::new(StorageCursor { history: history.clone(), sequence: 1 }, key.clone(), graph);
    primary.append_schemas(StorageBatch::new(vec![schema.clone()]).into()).await.unwrap();
    fallback.append_schemas(StorageBatch::new(vec![schema]).into()).await.unwrap();
    let records = (0..8)
        .map(|index| StoredRecord {
            position: StorageCursor {
                history: history.clone(),
                sequence: index + 2,
            },
            schema: key.clone(),
            scope: None,
            item: Identifier::new(format!("sample-{index}")).unwrap(),
            source: None,
            source_time: None,
            payload: Sample {
                time_ns: index as i64,
                value: index as f64,
                unused: vec![7; 1024],
            }
            .encode(),
        })
        .collect::<Vec<_>>();
    primary.append_records(StorageBatch::new(records.clone())).await.unwrap();
    fallback.append_records(StorageBatch::new(records.clone())).await.unwrap();
    let cut = StorageCursor { history: history.clone(), sequence: 9 };
    let saved = primary.create_snapshot(&cut).await.unwrap();
    let saved_fallback = fallback.create_snapshot(&cut).await.unwrap();

    // Save A Later Sample Without Changing Either Retained Cut.
    let mut late = records[4].clone();
    late.position.sequence = 10;
    primary.append_records(StorageBatch::new(vec![late.clone()])).await.unwrap();
    fallback.append_records(StorageBatch::new(vec![late])).await.unwrap();
    let read = ArrowScan {
        source: ColumnSource { kind: ColumnKind::Records, schema: key },
        columns: vec!["_storage.position".into(), "payload.value".into()],
        predicate: Some(ColumnPredicate {
            column: "payload.value".into(),
            lower: Some(3.0),
            upper: Some(6.0),
        }),
        time: Some(TimeBounds {
            column: "payload.time_ns".into(),
            lower_ns: 2,
            upper_ns: 7,
        }),
        start: 0,
        after: None,
        limits: ReadLimits { rows: nz(2), bytes: nz(4096) },
        work: ReadLimits {
            rows: nz(4),
            bytes: policy.response.batch_bytes,
        },
    };
    for (backend, snapshot) in [(&primary as &dyn StorageBackend, &saved), (fallback.as_ref() as &dyn StorageBackend, &saved_fallback)] {
        let mut read = read.clone();
        let mut values = Vec::new();
        let mut positions = Vec::new();
        loop {
            let page = list_column_rows(backend, snapshot, &read, &Cancellation::default()).await.unwrap();
            values.extend_from_slice(page.batch.column(1).as_any().downcast_ref::<Float64Array>().unwrap().values());
            positions.extend_from_slice(page.batch.column(0).as_any().downcast_ref::<UInt64Array>().unwrap().values());

            if page.complete {
                break;
            }
            let next = page.next.unwrap();
            assert!(next.position > read.after.as_ref().map_or(read.start, |after| after.position));
            read.after = Some(next);
        }
        assert_eq!(values, vec![3.0, 4.0, 5.0]);
        assert_eq!(positions, vec![5, 6, 7]);
    }

    // Reject A Changed Plan And Cancel An Already Entered Fallback Read.
    let page = list_column_rows(&primary, &saved, &read, &Cancellation::default()).await.unwrap();
    let mut changed = read.clone();
    changed.after = page.next;
    changed.columns.reverse();
    assert!(list_column_rows(&primary, &saved, &changed, &Cancellation::default()).await.is_err());
    fallback.control.block_read.store(true, Ordering::SeqCst);
    let cancelled = Cancellation::default();
    let pending = {
        let fallback = fallback.clone();
        let snapshot = saved_fallback.clone();
        let read = read.clone();
        let cancel = cancelled.clone();
        tokio::spawn(async move { list_column_rows(fallback.as_ref(), &snapshot, &read, &cancel).await })
    };
    fallback.control.read_entered.notified().await;
    cancelled.cancel();
    fallback.control.read_release.notify_one();
    assert!(pending.await.unwrap().is_err());
    fallback.control.block_read.store(false, Ordering::SeqCst);

    // Refuse Released And Restarted Snapshot Tokens.
    primary.delete_snapshot(&saved).await.unwrap();
    assert!(list_column_rows(&primary, &saved, &read, &Cancellation::default()).await.is_err());
    primary.delete_session().await.unwrap();

    primary.create_session().await.unwrap();
    assert!(list_column_rows(&primary, &saved, &read, &Cancellation::default()).await.is_err());
    let current = primary.create_snapshot(&cut).await.unwrap();
    assert_ne!(current.token, saved.token);
    primary.delete_snapshot(&current).await.unwrap();
    primary.delete_session().await.unwrap();
    fallback.delete_snapshot(&saved_fallback).await.unwrap();
    fallback.delete_session().await.unwrap();
}

fn sample_columns(schema: &StorageSchema, scope: &Identifier, count: usize) -> writes::DecodedBatch {
    let stream = Identifier::new("columnar.samples").unwrap();
    let rows = (0..count)
        .map(|index| StoredRow {
            record: StoredRecord {
                position: schema.position.clone(),
                schema: schema.key.clone(),
                scope: Some(scope.clone()),
                item: Identifier::new("execution").unwrap(),
                source: Some(Identifier::new("device").unwrap()),
                source_time: Some(UNIX_EPOCH + Duration::from_secs(index as u64)),
                payload: Sample {
                    time_ns: index as i64,
                    value: index as f64,
                    unused: vec![],
                }
                .encode(),
            },
            stream: stream.clone(),
            sequence: index as u64 + 1,
        })
        .collect::<Vec<_>>();
    writes::DecodedBatch {
        schema: schema.key.clone(),
        destination: stream,
        data: ArrowBatch::try_new(source::stream(schema, &rows).unwrap()).unwrap(),
    }
}

async fn columnar_fixture(settings: StorageSettings) -> (Fixture, StorageSchema) {
    let fixture = Fixture::new(settings).await;
    let key = SchemaKey {
        id: Identifier::new("sample.columns").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = graph::Schema::from_declaration(Sample::DEFINITION, &SchemaSettings::default()).unwrap();
    fixture.storage.write_schema(key.clone(), graph).await.unwrap();
    fixture.storage.flush().await.unwrap();
    let schema = fixture.storage.lookup_schema(&key).await.unwrap();
    (fixture, schema)
}

#[tokio::test]
async fn columnar_append_conformance_and_reopened_receipt() {
    let (fixture, schema) = columnar_fixture(backend::storage_settings()).await;
    let scope = Identifier::new("execution.columns").unwrap();
    let batch = sample_columns(&schema, &scope, 100);
    let (mutation, accepted) = conformance::columnar_appends(&fixture.storage, &scope, &batch).await.unwrap();
    let transient = fixture.data.create_snapshot(&accepted).await.unwrap();
    let snapshot = fixture.storage.snapshot(&accepted).await.unwrap();
    let retained = snapshot.retain(Some(&scope)).await.unwrap();
    drop(snapshot);
    let primary = fixture.data.clone();
    let reopened = fixture.storage.new_session().unwrap();
    fixture.finish().await;
    fixture::start(&reopened).await;
    conformance::columnar_reopened(&reopened, &scope, &batch, &mutation, &accepted).await.unwrap();

    // Session Tokens Expire While Scoped Columnar Ownership Survives.
    let selection = StreamSelection {
        schema: schema.key,
        scope: Some(scope),
        item: None,
        stream: Some(batch.destination),
        source_start: None,
        source_end: None,
        after: 0,
    };
    let page = ScanPage {
        after: 0,
        rows: nz(100),
        bytes: nz(64 * 1024),
    };
    assert!(primary.list_samples(&transient, &selection, page).await.is_err());
    let snapshot = primary.open_retained_snapshot(&retained).await.unwrap();
    let rows = primary.list_samples(&snapshot, &selection, page).await.unwrap();
    assert_eq!(rows.len(), 100);
    assert_eq!(rows.last().unwrap().record.position, accepted);
    primary.delete_snapshot(&snapshot).await.unwrap();
    reopened.release_retained_snapshot(&retained).await.unwrap();
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn columnar_append_acceptance_waits_for_atomic_durability_and_bounds_batches() {
    let mut settings = backend::storage_settings();
    settings.request.max_pending_write_batches = NonZeroUsize::MIN;
    let (fixture, schema) = columnar_fixture(settings).await;
    let scope = Identifier::new("execution.columns").unwrap();
    let batch = sample_columns(&schema, &scope, 100);
    fixture.data.control.block_flush.store(true, Ordering::SeqCst);
    let (mutation, accepted) = conformance::columnar_pending(&fixture.storage, &scope, &batch).await.unwrap();
    let flushing = tokio::spawn({
        let storage = fixture.storage.clone();
        async move { storage.flush().await }
    });
    fixture.data.control.flush_entered.notified().await;
    assert!(!fixture.storage.mutation_durable(&mutation, &accepted).unwrap());
    assert_eq!(fixture.storage.append_sample_batch(&mutation, Some(&scope), &batch).await.unwrap(), accepted);
    assert!(fixture.data.rows.lock().unwrap().is_empty());
    let second = MutationId {
        key: Identifier::new("second").unwrap(),
        ..mutation.clone()
    };
    let mut waiting = Box::pin(fixture.storage.append_sample_batch(&second, Some(&scope), &batch));
    assert!(poll_fn(|context| Poll::Ready(waiting.as_mut().poll(context))).await.is_pending());
    assert_eq!(fixture.storage.status().pending_batches, 1);
    assert!(fixture.storage.status().pending_bytes <= fixture.storage.settings().request.max_pending_write_bytes.get());
    fixture.data.control.block_flush.store(false, Ordering::SeqCst);
    fixture.data.control.flush_release.notify_one();
    flushing.await.unwrap().unwrap();
    let second = waiting.await.unwrap();
    assert_eq!(second.sequence, accepted.sequence + 100);
    fixture.storage.flush().await.unwrap();
    assert!(fixture.storage.mutation_durable(&mutation, &accepted).unwrap());
    fixture.finish().await;
}

#[tokio::test]
async fn columnar_append_refuses_oversized_retention_and_conflicting_metadata() {
    let mut settings = backend::storage_settings();
    settings.request.max_pending_write_bytes = nz(64 * 1024);
    let (fixture, schema) = columnar_fixture(settings).await;
    let scope = Identifier::new("execution.columns").unwrap();
    let oversized = sample_columns(&schema, &scope, 10_000);
    conformance::columnar_admission_bounds(&fixture.storage, &scope, &oversized).await.unwrap();
    let batch = sample_columns(&schema, &scope, 2);
    let mutation = MutationId {
        window: fixture.storage.mutation_window().unwrap().id,
        key: Identifier::new("refused").unwrap(),
    };
    let foreign = Identifier::new("foreign").unwrap();
    assert!(fixture.storage.append_sample_batch(&mutation, Some(&foreign), &batch).await.is_err());
    let mut changed = batch.clone();
    changed.destination = foreign;
    assert!(fixture.storage.append_sample_batch(&mutation, Some(&scope), &changed).await.is_err());
    let accepted = fixture.storage.append_sample_batch(&mutation, Some(&scope), &batch).await.unwrap();
    assert_eq!(accepted.sequence, schema.position.sequence + 2);
    fixture.finish().await;
}

#[tokio::test]
async fn columnar_append_replays_after_lost_durable_acknowledgement() {
    let (fixture, schema) = columnar_fixture(backend::storage_settings()).await;
    let scope = Identifier::new("execution.columns").unwrap();
    let batch = sample_columns(&schema, &scope, 100);
    let mutation = MutationId {
        window: fixture.storage.mutation_window().unwrap().id,
        key: Identifier::new("lost.acknowledgement").unwrap(),
    };
    fixture.data.control.flush_failures.store(1, Ordering::SeqCst);
    let accepted = fixture.storage.append_sample_batch(&mutation, Some(&scope), &batch).await.unwrap();
    assert!(fixture.storage.flush().await.is_err());
    assert_eq!(fixture.data.rows.lock().unwrap().len(), 100);
    assert!(!fixture.storage.mutation_durable(&mutation, &accepted).unwrap());
    fixture.storage.retry_transient_persistence();
    assert_eq!(fixture.storage.append_sample_batch(&mutation, Some(&scope), &batch).await.unwrap(), accepted);
    fixture.storage.flush().await.unwrap();
    fixture.storage.wait_mutation_durable(&mutation, &accepted).await.unwrap();
    assert_eq!(fixture.data.rows.lock().unwrap().len(), 100);
    assert_eq!(fixture.data.control.write_sizes.lock().unwrap().last(), Some(&1));
    fixture.finish().await;
}

#[tokio::test]
async fn columnar_appends_preserve_multiple_registered_schemas() {
    let (fixture, sample_schema) = columnar_fixture(backend::storage_settings()).await;
    let scope = Identifier::new("execution.columns").unwrap();
    let samples = sample_columns(&sample_schema, &scope, 2);
    let window = fixture.storage.mutation_window().unwrap().id;
    let first = MutationId {
        window: window.clone(),
        key: Identifier::new("sample.first").unwrap(),
    };
    fixture.storage.append_sample_batch(&first, Some(&scope), &samples).await.unwrap();
    let key = SchemaKey {
        id: Identifier::new("reading.columns").unwrap(),
        ..sample_schema.key.clone()
    };
    let graph = graph::Schema::from_declaration(Reading::DEFINITION, &SchemaSettings::default()).unwrap();
    fixture.storage.write_schema(key.clone(), graph).await.unwrap();
    fixture.storage.flush().await.unwrap();
    let reading_schema = fixture.storage.lookup_schema(&key).await.unwrap();
    let stream = Identifier::new("columnar.readings").unwrap();
    let rows = [2.0, 3.0]
        .into_iter()
        .enumerate()
        .map(|(index, amplitude)| StoredRow {
            record: StoredRecord {
                position: reading_schema.position.clone(),
                schema: key.clone(),
                scope: Some(scope.clone()),
                item: Identifier::new("execution").unwrap(),
                source: None,
                source_time: None,
                payload: Reading { amplitude }.encode(),
            },
            stream: stream.clone(),
            sequence: index as u64 + 1,
        })
        .collect::<Vec<_>>();
    let readings = writes::DecodedBatch {
        schema: key.clone(),
        destination: stream.clone(),
        data: ArrowBatch::try_new(source::stream(&reading_schema, &rows).unwrap()).unwrap(),
    };
    let second = MutationId {
        window: window.clone(),
        key: Identifier::new("reading").unwrap(),
    };
    fixture.storage.append_sample_batch(&second, Some(&scope), &readings).await.unwrap();
    let third = MutationId {
        window,
        key: Identifier::new("sample.repeat").unwrap(),
    };
    fixture.storage.append_sample_batch(&third, Some(&scope), &samples).await.unwrap();
    fixture.storage.flush().await.unwrap();
    let cut = fixture.data.create_snapshot(&fixture.storage.get_saved_position()).await.unwrap();
    let selection = StreamSelection {
        schema: key,
        scope: Some(scope),
        item: None,
        stream: Some(stream),
        source_start: None,
        source_end: None,
        after: 0,
    };
    let page = ScanPage {
        after: 0,
        rows: nz(10),
        bytes: nz(64 * 1024),
    };
    let actual = fixture.data.list_samples(&cut, &selection, page).await.unwrap();
    assert_eq!(
        actual.iter().map(|row| row.record.payload.clone()).collect::<Vec<_>>(),
        rows.iter().map(|row| row.record.payload.clone()).collect::<Vec<_>>()
    );
    let selection = StreamSelection {
        schema: sample_schema.key,
        stream: Some(samples.destination),
        ..selection
    };
    let actual = fixture.data.list_samples(&cut, &selection, page).await.unwrap();
    assert_eq!(actual.len(), 4);
    fixture.data.delete_snapshot(&cut).await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
async fn columnar_watch_pages_replay_complete_frontiers_and_resync_scope_deletion() {
    // Register Accepted, Durable and Filtered Followers Before Admission.
    let mut settings = backend::storage_settings();
    settings.response.batch_bytes = NonZeroUsize::MIN;
    let (fixture, schema) = columnar_fixture(settings).await;
    let opening = fixture.storage.get_saved_position();
    let scope = Identifier::new("watched.columns").unwrap();
    let batch = sample_columns(&schema, &scope, 3);
    let selection = StreamSelection {
        schema: schema.key,
        scope: Some(scope.clone()),
        item: None,
        stream: Some(batch.destination.clone()),
        source_start: None,
        source_end: None,
        after: 0,
    };
    let mut accepted = fixture.storage.follow::<StoredRow>(Some(&opening), selection.clone()).await.unwrap();
    let mut durable = fixture.storage.follow::<StoredRow>(Some(&opening), selection.clone()).await.unwrap();
    let prefix = StreamSelection {
        source_start: Some(UNIX_EPOCH),
        source_end: Some(UNIX_EPOCH + Duration::from_secs(2)),
        ..selection.clone()
    };
    let mut filtered = fixture.storage.follow::<StoredRow>(Some(&opening), prefix).await.unwrap();
    let mutation = MutationId {
        window: fixture.storage.mutation_window().unwrap().id,
        key: Identifier::new("watch.columns").unwrap(),
    };
    let through = fixture.storage.append_sample_batch(&mutation, Some(&scope), &batch).await.unwrap();

    // Accepted Followers Project Columns Before the Backend Commits.
    let page = accepted.next().await.unwrap();
    assert_eq!(page.entries().len(), 3);
    for (index, row) in page.entries().iter().enumerate() {
        assert_eq!(row.record.position.sequence, opening.sequence + index as u64 + 1);
        assert_eq!(row.sequence, index as u64 + 1);
        let sample = Sample::decode(&row.record.payload).unwrap();
        assert_eq!(sample.value, index as f64);
    }
    drop(accepted);
    fixture.storage.flush().await.unwrap();

    // Bounded Pages and Historical Resume Never Publish an Interior Cut.
    let mut replay = fixture.storage.follow::<StoredRow>(Some(&opening), selection).await.unwrap();
    for index in 0..3 {
        let (page, cursor) = durable.next_durable().await.unwrap();
        let (replayed, resumed) = replay.next_durable().await.unwrap();
        assert_eq!(page.entries().len(), 1);
        assert_eq!(page.entries()[0].record.position.sequence, opening.sequence + index + 1);
        assert_eq!(cursor, if index == 2 { through.clone() } else { opening.clone() });
        assert_eq!(resumed, cursor);
        assert_eq!(replayed.entries(), page.entries());
        if index < 2 {
            assert!(fixture.storage.snapshot(&page.entries()[0].record.position).await.is_err());
        }
    }

    // An Unmatched Suffix Advances Only After the Complete Batch Is Scanned.
    for _ in 0..2 {
        let (page, cursor) = filtered.next_durable().await.unwrap();
        assert_eq!(page.entries().len(), 1);
        assert_eq!(cursor, opening);
    }
    let (page, cursor) = filtered.next_durable().await.unwrap();
    assert!(page.entries().is_empty());
    assert_eq!(cursor, through);

    // Deleting the Watched Scope Requires Resync for Every Durable Observer.
    let deletion = MutationId {
        key: Identifier::new("watch.delete").unwrap(),
        ..mutation
    };
    fixture.storage.mutate_once(&deletion, vec![MutationItem::DeleteScope(scope)]).await.unwrap();
    assert!(matches!(durable.next_durable().await, Err(Error::Gap { .. })));
    assert!(matches!(replay.next_durable().await, Err(Error::Gap { .. })));
    assert!(matches!(filtered.next_durable().await, Err(Error::Gap { .. })));
    drop((durable, replay, filtered));
    fixture.finish().await;
}
