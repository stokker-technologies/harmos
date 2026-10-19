/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Historical Reads, Public Scalar Folds And Concurrent Snapshot Reads

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::{NonZeroU128, NonZeroUsize};
use std::sync::Arc;
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use super::backend::{Data, Recovery, history, storage_settings};
use super::waveforms::{self, Experiment, Objects, Sample};
use harmos_domain::Aggregate;
use harmos_domain::operations::storage::Query;
use harmos_storage::Error;
use harmos_storage::query::scalar::{Agg, ExactNumber, ScalarValue};
use harmos_storage::query::{Accumulator, ObjectVersions, RecordSelection, RowSlice, Scratch, Source};
use harmos_storage::{ObjectRead, ObjectReceipt, ObjectVersion, RecordRead, StorageBackends, StorageManager, StorageSettings};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

async fn manager(settings: StorageSettings) -> (StorageManager<Objects>, Arc<Data<Objects>>) {
    let data = Arc::new(Data::<Objects>::with_catalog(history()));
    let storage =
        StorageManager::<Objects>::new(StorageBackends { primary: data.clone(), state: Arc::new(Recovery::new(history())) }, settings)
            .unwrap();
    super::fixture::start(&storage).await;
    (storage, data)
}

fn paged() -> StorageSettings {
    let mut settings = storage_settings();
    settings.response.batch_rows = NonZeroUsize::new(2).unwrap();
    settings
}

async fn write(storage: &StorageManager<Objects>, id: &str, label: &str) -> ObjectReceipt {
    let receipt = storage.write_object(&Experiment { id: id.into(), label: label.into() }).await.unwrap();
    storage.flush_until(&receipt.position).await.unwrap();
    receipt
}

fn samples() -> RecordSelection {
    RecordSelection { schema: waveforms::schema(), scope: None, item: None, source_start: None, source_end: None }
}

async fn labels(storage: &StorageManager<Objects>, read: ObjectRead<'_, Experiment>) -> Vec<(String, ObjectReceipt)> {
    storage
        .read_objects(read)
        .await
        .unwrap()
        .collect()
        .await
        .unwrap()
        .into_iter()
        .map(|revision| (revision.value.label, ObjectReceipt { position: revision.position, version: revision.version }))
        .collect()
}

fn fold(agg: Agg, values: &[ScalarValue]) -> ScalarValue {
    let mut scratch = Scratch::new(values.len());
    let mut accumulator = Accumulator::new(agg);
    for (capture, value) in values.iter().enumerate() {
        accumulator.push(capture as u64, value, &mut scratch).unwrap();
    }
    accumulator.finish(&mut || Ok(())).unwrap()
}

fn ratio(negative: bool, numerator: u128, denominator: u128) -> ScalarValue {
    ScalarValue::Exact(ExactNumber::new(negative, numerator, NonZeroU128::new(denominator).unwrap()))
}

#[tokio::test]
async fn typed_objects_page() {
    let (storage, _) = manager(paged()).await;
    let first = write(&storage, "a", "one").await;
    let second = write(&storage, "a", "two").await;
    let third = write(&storage, "b", "three").await;
    let at = storage.cursor_saved();
    let read = |key, versions| ObjectRead::<Experiment> { at: at.clone(), key, versions };

    // Pages Follow The Response Row Bound In Storage Order.
    let mut reader = storage.read_objects(read(None, ObjectVersions::Revisions)).await.unwrap();
    let mut pages = Vec::new();
    let mut revisions = Vec::new();
    while let Some(page) = reader.next().await.unwrap() {
        pages.push(page.len());
        revisions.extend(page.into_iter().map(|revision| revision.version));
    }
    assert_eq!(pages, [2, 1]);
    assert_eq!(revisions, [first.version.clone(), second.version.clone(), third.version.clone()]);
    assert!(reader.next().await.unwrap().is_none());
    let latest = labels(&storage, read(None, ObjectVersions::Latest)).await;
    assert_eq!(latest, [("two".into(), second.clone()), ("three".into(), third)]);
    let key = "a".to_owned();
    let keyed = labels(&storage, read(Some(&key), ObjectVersions::Revisions)).await;
    assert_eq!(keyed, [("one".into(), first.clone()), ("two".into(), second.clone())]);

    // An Earlier Saved Cursor Reads Its Own Image.
    let earlier =
        labels(&storage, ObjectRead::<Experiment> { at: first.position.clone(), key: None, versions: ObjectVersions::Revisions }).await;
    assert_eq!(earlier, [("one".into(), first)]);

    // Consumers Narrow Decoded Values Without Changing Stored Revision Identity.
    let revisions = labels(&storage, read(None, ObjectVersions::Revisions)).await;
    let filtered: Vec<_> = revisions.into_iter().filter(|(label, _)| label == "two").collect();
    assert_eq!(filtered, [("two".into(), second)]);
    let latest = labels(&storage, read(None, ObjectVersions::Latest)).await;
    assert!(!latest.iter().any(|(label, _)| label == "one"));
}

#[tokio::test]
async fn typed_records_page() {
    let (storage, _) = manager(paged()).await;
    storage.write_schema(&waveforms::schema(), &waveforms::definition()).await.unwrap();
    let inputs = waveforms::inputs(&waveforms::samples(1));
    let records = inputs.iter().take(9).map(|input| input.record.borrowed()).collect::<Vec<_>>();
    storage.write_records(&records).await.unwrap();
    storage.flush().await.unwrap();
    let expected = waveforms::samples(1).into_iter().take(9).collect::<Vec<_>>();
    let read = |after| RecordRead { at: storage.cursor_saved(), selection: samples(), after };

    let mut reader = storage.read_records::<Sample>(read(0)).await.unwrap();
    let mut pages = Vec::new();
    let mut stored = Vec::new();
    while let Some(page) = reader.next().await.unwrap() {
        pages.push(page.len());
        stored.extend(page);
    }
    assert_eq!(pages, [2, 2, 2, 2, 1]);
    assert_eq!(stored.iter().map(|record| record.payload.clone()).collect::<Vec<_>>(), expected);
    for (record, input) in stored.iter().zip(&inputs) {
        assert_eq!(record.item, input.record.item);
        assert_eq!(record.scope, input.record.scope);
        assert_eq!(record.source_time, input.record.source_time);
    }

    // Resuming After A Position Skips Everything Already Read.
    let resumed = storage.read_records::<Sample>(read(stored[3].position.sequence)).await.unwrap().collect().await.unwrap();
    assert_eq!(resumed.into_iter().map(|record| record.payload).collect::<Vec<_>>(), expected[4..]);
    let phase: Vec<_> = storage
        .read_records::<Sample>(read(0))
        .await
        .unwrap()
        .collect()
        .await
        .unwrap()
        .into_iter()
        .filter(|record| record.payload.phase == 1)
        .collect();
    assert_eq!(phase.len(), 3);
    assert!(phase.iter().all(|record| record.payload.phase == 1));
}

#[tokio::test]
async fn unwritten_schema_empty() {
    let (storage, _) = manager(paged()).await;
    write(&storage, "a", "one").await;
    let at = storage.cursor_saved();
    let mut reader = storage.read_records::<Sample>(RecordRead { at: at.clone(), selection: samples(), after: 0 }).await.unwrap();
    assert!(reader.next().await.unwrap().is_none());
    drop(reader);

    // The Arrow Query Has No Schema To Answer, So It Still Refuses.
    let mut query = storage
        .query(Query { at: at.clone(), source: Source::Records(samples()), transforms: Vec::new(), slice: RowSlice::default() })
        .await
        .unwrap();
    assert!(query.next().await.is_err());
    drop(query);

    let (storage, _) = manager(paged()).await;
    storage.write_schema(&waveforms::schema(), &waveforms::definition()).await.unwrap();
    storage.flush().await.unwrap();
    let read = ObjectRead::<Experiment> { at: storage.cursor_saved(), key: None, versions: ObjectVersions::Revisions };
    let mut reader = storage.read_objects(read).await.unwrap();
    assert!(reader.next().await.unwrap().is_none());
    drop(reader);
}

#[tokio::test]
async fn typed_budget_refuses() {
    let mut settings = paged();
    settings.budget.read_memory_bytes = NonZeroUsize::new(128).unwrap();
    settings.response.batch_bytes = NonZeroUsize::new(settings.budget.read_memory_bytes.get() / 2).unwrap();
    let (storage, _) = manager(settings).await;
    write(&storage, "a", "one").await;
    let objects = async {
        storage
            .read_objects(ObjectRead::<Experiment> { at: storage.cursor_saved(), key: None, versions: ObjectVersions::Latest })
            .await?
            .collect()
            .await
    };
    assert!(matches!(objects.await, Err(Error::ResourceExhausted { .. })));
    let records = async {
        storage.read_records::<Sample>(RecordRead { at: storage.cursor_saved(), selection: samples(), after: 0 }).await?.collect().await
    };
    assert!(matches!(records.await, Err(Error::ResourceExhausted { .. })));
    assert_eq!(storage.status().read_bytes, 0);

    // An Admitted Reader Refuses A Page No Budget Split Can Hold.
    let mut settings = paged();
    settings.budget.read_memory_bytes = NonZeroUsize::new(8 * 1024).unwrap();
    settings.response.batch_bytes = NonZeroUsize::new(settings.budget.read_memory_bytes.get() / 2).unwrap();
    let (storage, _) = manager(settings).await;
    write(&storage, "large", &"x".repeat(16 * 1024)).await;
    let mut reader = storage
        .read_objects(ObjectRead::<Experiment> { at: storage.cursor_saved(), key: None, versions: ObjectVersions::Latest })
        .await
        .unwrap();
    assert!(matches!(reader.next().await, Err(Error::ResourceExhausted { .. })));
    drop(reader);
    assert_eq!(storage.status().read_bytes, 0);
}

#[test]
fn public_scalar_fold() {
    let unsigned = [1u64, 2, 3, 4].map(ScalarValue::Unsigned);
    assert_eq!(fold(Agg::Of(Aggregate::Sum), &unsigned), ScalarValue::Unsigned(10));
    assert_eq!(fold(Agg::Of(Aggregate::Mean), &unsigned), ratio(false, 5, 2));
    assert_eq!(fold(Agg::Of(Aggregate::Median), &unsigned), ratio(false, 5, 2));
    let signed = [ScalarValue::Signed(-3), ScalarValue::Signed(1)];
    assert_eq!(fold(Agg::Of(Aggregate::Mean), &signed), ScalarValue::Signed(-1));
    let text = ["b", "c", "a"].map(|value| ScalarValue::Text(value.into()));
    assert_eq!(fold(Agg::Of(Aggregate::Min), &text), ScalarValue::Text("a".into()));
    assert_eq!(fold(Agg::Of(Aggregate::Max), &text), ScalarValue::Text("c".into()));
    assert_eq!(fold(Agg::Last, &text), ScalarValue::Text("a".into()));
    let sparse = [ScalarValue::Unsigned(1), ScalarValue::Null, ScalarValue::Missing];
    assert_eq!(fold(Agg::Len, &sparse), ScalarValue::Unsigned(3));
    assert_eq!(fold(Agg::Of(Aggregate::Count), &sparse), ScalarValue::Unsigned(1));
    assert_eq!(fold(Agg::Of(Aggregate::Mean), &[ScalarValue::Null]), ScalarValue::Missing);

    // Floating Statistics Answer Through The Signal Kernels: Population Std.
    let values = [1.0, 2.0, 4.0, 8.0].map(ScalarValue::Float);
    let float = |agg| match fold(Agg::Of(agg), &values) {
        ScalarValue::Float(value) => value,
        other => panic!("floating statistic answered {other:?}"),
    };
    assert!((float(Aggregate::Std) - 7.1875f64.sqrt()).abs() < 1e-12);
    assert!((float(Aggregate::Rms) - 21.25f64.sqrt()).abs() < 1e-12);
    assert_eq!(fold(Agg::Of(Aggregate::Median), &values), ScalarValue::Float(3.0));
    let mut accumulator = Accumulator::new(Agg::Of(Aggregate::Sum));
    let mut scratch = Scratch::new(0);
    accumulator.push(0, &ScalarValue::Unsigned(1), &mut scratch).unwrap();
    assert!(accumulator.push(1, &ScalarValue::Text("x".into()), &mut scratch).is_err());
    let mut median = Accumulator::new(Agg::Of(Aggregate::Median));
    assert!(median.push(0, &ScalarValue::Unsigned(1), &mut scratch).is_err());
}

#[tokio::test]
async fn snapshot_reads_concurrent() {
    let (storage, data) = manager(storage_settings()).await;
    write(&storage, "a", "one").await;
    let key = "a".to_owned();
    let read = || storage.read_object::<Experiment>(&key, ObjectVersion::Latest);
    data.control.block_read.store(true, Ordering::SeqCst);
    let mut first = Box::pin(read());
    tokio::select! {
        _ = &mut first => panic!("gated read completed"),
        _ = data.control.read_entered.notified() => {}
    }

    // A Second Read Enters The Backend While The First Is Still Inside.
    let mut second = Box::pin(read());
    tokio::select! {
        _ = &mut second => panic!("gated read completed"),
        _ = data.control.read_entered.notified() => {}
    }
    assert_eq!(data.control.reads_gated_peak.load(Ordering::SeqCst), 2);

    // Closing Still Waits For Every In-Flight Read Before Releasing The Backend.
    let mut closing = Box::pin(storage.close());
    assert!(super::pending(closing.as_mut()).await);
    assert_eq!(data.control.closes.load(Ordering::SeqCst), 0);
    data.control.block_read.store(false, Ordering::SeqCst);
    data.control.read_release.notify_waiters();
    // Released Reads Settle Under Closing Admission Before The Backend Closes.
    let (first, second) = tokio::join!(first, second);
    assert!(matches!(first, Err(Error::Closing { .. })));
    assert!(matches!(second, Err(Error::Closing { .. })));
    assert_eq!(data.control.closes.load(Ordering::SeqCst), 0);
    closing.await.unwrap();
    assert_eq!(data.control.closes.load(Ordering::SeqCst), 1);
}

/* === Tests === */
