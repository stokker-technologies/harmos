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
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use super::backend::{Data, Recovery, history, storage_settings};
use super::waveforms::{self, Experiment, Objects, Sample};
use harmos_storage::Error;
use harmos_storage::Query;
use harmos_storage::query::{ObjectVersions, RecordSelection, RowSlice, Source};
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
    let storage = StorageManager::<Objects>::new(
        StorageBackends {
            primary: data.clone(),
            state: Arc::new(Recovery::new(history())),
        },
        settings,
    )
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
    RecordSelection {
        schema: waveforms::schema(),
        scope: None,
        item: None,
        source_start: None,
        source_end: None,
    }
}

async fn labels(storage: &StorageManager<Objects>, read: ObjectRead<'_, Experiment>) -> Vec<(String, ObjectReceipt)> {
    let pages = storage.read_objects(read).await.unwrap().collect().await.unwrap();
    pages
        .iter()
        .flat_map(|page| page.iter())
        .map(|revision| {
            (
                revision.value.label.clone(),
                ObjectReceipt {
                    position: revision.position.clone(),
                    version: revision.version.clone(),
                },
            )
        })
        .collect()
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
        revisions.extend(page.iter().map(|revision| revision.version.clone()));
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
    let earlier = labels(
        &storage,
        ObjectRead::<Experiment> {
            at: first.position.clone(),
            key: None,
            versions: ObjectVersions::Revisions,
        },
    )
    .await;
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
    storage.write_schema(waveforms::schema(), waveforms::definition()).await.unwrap();
    let inputs = waveforms::inputs(&waveforms::samples(1));
    let records = inputs.iter().take(9).map(|input| input.record.clone()).collect::<Vec<_>>();
    storage.write_records((records).to_vec()).await.unwrap();
    storage.flush().await.unwrap();
    let expected = waveforms::samples(1).into_iter().take(9).collect::<Vec<_>>();
    let read = |after| RecordRead {
        at: storage.cursor_saved(),
        selection: samples(),
        after,
    };

    let mut reader = storage.read_records::<Sample>(read(0)).await.unwrap();
    let mut pages = Vec::new();
    let mut stored = Vec::new();
    while let Some(page) = reader.next().await.unwrap() {
        pages.push(page.len());
        stored.extend(page.iter().cloned());
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
    assert_eq!(resumed.iter().flat_map(|page| page.iter()).map(|record| record.payload.clone()).collect::<Vec<_>>(), expected[4..]);
    let phase: Vec<_> = storage
        .read_records::<Sample>(read(0))
        .await
        .unwrap()
        .collect()
        .await
        .unwrap()
        .iter()
        .flat_map(|page| page.iter())
        .filter(|record| record.payload.phase == 1)
        .cloned()
        .collect();
    assert_eq!(phase.len(), 3);
    assert!(phase.iter().all(|record| record.payload.phase == 1));
}

#[tokio::test]
async fn typed_owner_retained() {
    let (storage, data) = manager(paged()).await;
    storage.write_schema(waveforms::schema(), waveforms::definition()).await.unwrap();
    let inputs = waveforms::inputs(&waveforms::samples(1));
    storage.write_records(vec![inputs[0].record.clone()]).await.unwrap();
    storage.flush().await.unwrap();
    let mut reader = storage
        .read_records::<Sample>(RecordRead {
            at: storage.cursor_saved(),
            selection: samples(),
            after: 0,
        })
        .await
        .unwrap();
    let page = reader.next().await.unwrap().unwrap();
    let alias = page.clone();
    let pointer = page.as_ptr();
    let owner = page.owner();
    drop(reader);
    data.snapshots_released().await;
    assert!(storage.status().read_bytes >= harmos_storage::MemorySize::memory_size(&page));
    drop(page);
    drop(alias);
    assert!(storage.status().read_bytes > 0);
    let retained: &Vec<harmos_storage::StoredRecord<Sample>> = std::borrow::Borrow::borrow(owner.as_ref());
    assert_eq!(retained.as_ptr(), pointer);
    drop(owner);
    data.snapshots_released().await;
    assert_eq!(storage.status().read_bytes, 0);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn unwritten_schema_empty() {
    let (storage, _) = manager(paged()).await;
    write(&storage, "a", "one").await;
    let at = storage.cursor_saved();
    let mut reader = storage
        .read_records::<Sample>(RecordRead {
            at: at.clone(),
            selection: samples(),
            after: 0,
        })
        .await
        .unwrap();
    assert!(reader.next().await.unwrap().is_none());
    drop(reader);

    // The Arrow Query Has No Schema To Answer, So It Still Refuses.
    let mut query = storage
        .query(Query {
            at: at.clone(),
            source: Source::Records(samples()),
            transforms: Vec::new(),
            slice: RowSlice::default(),
        })
        .await
        .unwrap();
    assert!(query.next().await.is_err());
    drop(query);

    let (storage, _) = manager(paged()).await;
    storage.write_schema(waveforms::schema(), waveforms::definition()).await.unwrap();
    storage.flush().await.unwrap();
    let read = ObjectRead::<Experiment> {
        at: storage.cursor_saved(),
        key: None,
        versions: ObjectVersions::Revisions,
    };
    let mut reader = storage.read_objects(read).await.unwrap();
    assert!(reader.next().await.unwrap().is_none());
    drop(reader);
}

#[tokio::test]
async fn typed_budget_refuses() {
    let mut settings = paged();
    settings.budget.read_memory_bytes = NonZeroUsize::new(128).unwrap();
    settings.response.batch_bytes = NonZeroUsize::new(settings.budget.read_memory_bytes.get() / 2).unwrap();
    let (storage, data) = manager(settings).await;
    write(&storage, "a", "one").await;
    let objects = async {
        storage
            .read_objects(ObjectRead::<Experiment> {
                at: storage.cursor_saved(),
                key: None,
                versions: ObjectVersions::Latest,
            })
            .await?
            .collect()
            .await
    };
    assert!(matches!(objects.await, Err(Error::ResourceExhausted { .. })));
    let records = async {
        storage
            .read_records::<Sample>(RecordRead {
                at: storage.cursor_saved(),
                selection: samples(),
                after: 0,
            })
            .await?
            .collect()
            .await
    };
    assert!(matches!(records.await, Err(Error::ResourceExhausted { .. })));
    data.snapshots_released().await;
    assert_eq!(storage.status().read_bytes, 0);

    // An Admitted Reader Refuses A Page No Budget Split Can Hold.
    let mut settings = paged();
    settings.budget.read_memory_bytes = NonZeroUsize::new(8 * 1024).unwrap();
    settings.response.batch_bytes = NonZeroUsize::new(settings.budget.read_memory_bytes.get() / 2).unwrap();
    let (storage, data) = manager(settings).await;
    write(&storage, "large", &"x".repeat(16 * 1024)).await;
    let mut reader = storage
        .read_objects(ObjectRead::<Experiment> {
            at: storage.cursor_saved(),
            key: None,
            versions: ObjectVersions::Latest,
        })
        .await
        .unwrap();
    assert!(matches!(reader.next().await, Err(Error::ResourceExhausted { .. })));
    drop(reader);
    data.snapshots_released().await;
    assert_eq!(storage.status().read_bytes, 0);
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

#[tokio::test]
async fn session_slots_refuse() {
    let mut settings = paged();
    settings.request.max_active_queries = NonZeroUsize::new(1).unwrap();
    let (storage, data) = manager(settings).await;
    write(&storage, "a", "one").await;
    write(&storage, "b", "two").await;
    let session = storage.read_session();
    let read = || ObjectRead::<Experiment> {
        at: storage.cursor_saved(),
        key: None,
        versions: ObjectVersions::Latest,
    };
    let mut first = session.read_objects(read()).await.unwrap();
    let retained = first.next().await.unwrap().unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(1), session.read_objects(read())).await.unwrap();
    assert!(matches!(result, Err(Error::ResourceExhausted { resource: "active storage queries" })));
    // Point Reads Need No Active Reader Slot.
    assert!(session.read_object::<Experiment>(&"a".to_owned(), ObjectVersion::Latest).await.unwrap().is_some());
    assert!(first.next().await.unwrap().is_none());
    let mut second = session.read_objects(read()).await.unwrap();
    assert_eq!(second.next().await.unwrap().unwrap().len(), 2);
    drop(second);
    drop(first);
    drop(session);
    assert_eq!(retained[0].value.label, "one");
    drop(retained);
    data.snapshots_released().await;
    assert_eq!(storage.status().read_bytes, 0);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn session_pressure_refuses() {
    let settings = paged();
    let budget = settings.budget.read_memory_bytes.get();
    let (storage, data) = manager(settings).await;
    write(&storage, "a", "one").await;
    let session = storage.read_session();
    let mut reader = session
        .read_objects::<Experiment>(ObjectRead {
            at: storage.cursor_saved(),
            key: None,
            versions: ObjectVersions::Latest,
        })
        .await
        .unwrap();
    let page = reader.next().await.unwrap().unwrap();
    let collection = session.collection::<Vec<String>>().unwrap();
    let remaining = budget - storage.status().read_bytes;
    let collection = collection.try_map(remaining, Ok::<_, Error>).unwrap();
    assert_eq!(storage.status().read_bytes, budget);
    let point =
        tokio::time::timeout(std::time::Duration::from_secs(1), session.read_object::<Experiment>(&"a".to_owned(), ObjectVersion::Latest))
            .await
            .unwrap();
    assert!(matches!(point, Err(Error::ResourceExhausted { .. })));
    let next = tokio::time::timeout(std::time::Duration::from_secs(1), reader.next()).await.unwrap();
    assert!(matches!(next, Err(Error::ResourceExhausted { .. })));
    assert_eq!(page[0].value.label, "one");
    drop(collection);
    drop(page);
    drop(reader);
    drop(session);
    data.snapshots_released().await;
    assert_eq!(storage.status().read_bytes, 0);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn session_collection_retains() {
    let mut settings = paged();
    settings.response.batch_rows = NonZeroUsize::new(1).unwrap();
    let (storage, data) = manager(settings).await;
    write(&storage, "a", "one").await;
    write(&storage, "b", "two").await;
    let session = storage.read_session();
    let mut collection = session.collection::<Vec<String>>().unwrap();
    let mut reader = session
        .read_objects::<Experiment>(ObjectRead {
            at: storage.cursor_saved(),
            key: None,
            versions: ObjectVersions::Latest,
        })
        .await
        .unwrap();
    while let Some(page) = reader.next().await.unwrap() {
        let page = page
            .try_map(4 * std::mem::size_of::<String>(), |rows| {
                Ok::<_, Error>(rows.into_iter().map(|row| row.value.label).collect::<Vec<_>>())
            })
            .unwrap();
        collection = collection
            .try_map(4 * std::mem::size_of_val(&page), |mut pages| {
                pages.push(page);
                Ok::<_, Error>(pages)
            })
            .unwrap();
        if collection.iter().map(|page| page.len()).sum::<usize>() == 1 {
            break;
        }
    }
    assert_eq!(collection.len(), 1);
    assert_eq!(collection[0].as_slice(), &["one"]);
    // Stopping At The Selected Limit Leaves The Next Source Page Unconsumed.
    assert_eq!(reader.next().await.unwrap().unwrap()[0].value.label, "two");
    let child = collection[0].clone();
    let pointer = child[0].as_ptr();
    drop(reader);
    drop(session);
    data.snapshots_released().await;
    drop(collection);
    assert_eq!(child[0].as_ptr(), pointer);
    assert!(storage.status().read_bytes > 0);
    drop(child);
    assert_eq!(storage.status().read_bytes, 0);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn session_decode_refuses() {
    let settings = paged();
    let budget = settings.budget.read_memory_bytes.get();
    let (storage, data) = manager(settings).await;
    write(&storage, "a", "one").await;
    let session = storage.read_session();
    let peer = session.clone();
    let collection = session.collection::<Vec<String>>().unwrap();
    data.control.block_read.store(true, Ordering::SeqCst);
    let key = "a".to_owned();
    let (point, collection) = tokio::join!(
        tokio::time::timeout(std::time::Duration::from_secs(1), peer.read_object::<Experiment>(&key, ObjectVersion::Latest)),
        async {
            data.control.read_entered.notified().await;
            // The Point Read Has Its Initial Claim; Its Decoded Growth Must
            // Still See These Other Results In The Same Admission Group.
            let remaining = budget - storage.status().read_bytes;
            let collection = collection.try_map(remaining, Ok::<_, Error>).unwrap();
            data.control.block_read.store(false, Ordering::SeqCst);
            data.control.read_release.notify_one();
            collection
        }
    );
    assert!(matches!(point.unwrap(), Err(Error::ResourceExhausted { .. })));
    drop(peer);
    drop(session);
    drop(collection);
    data.snapshots_released().await;
    assert_eq!(storage.status().read_bytes, 0);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn standalone_reader_queues() {
    let mut settings = paged();
    settings.request.max_active_queries = NonZeroUsize::new(1).unwrap();
    let (storage, data) = manager(settings).await;
    write(&storage, "a", "one").await;
    let read = || ObjectRead::<Experiment> {
        at: storage.cursor_saved(),
        key: None,
        versions: ObjectVersions::Latest,
    };
    let first = storage.read_objects(read()).await.unwrap();
    let mut second = Box::pin(storage.read_objects(read()));
    assert!(super::fixture::pending(second.as_mut()).await);
    drop(first);
    let mut second = second.await.unwrap();
    assert_eq!(second.next().await.unwrap().unwrap()[0].value.label, "one");
    drop(second);
    data.snapshots_released().await;
    assert_eq!(storage.status().read_bytes, 0);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn session_metadata_growth() {
    let settings = paged();
    let budget = settings.budget.read_memory_bytes.get();
    let (storage, _) = manager(settings).await;
    let session = storage.read_session();
    let memory = session.try_memory(NonZeroUsize::new(4096).unwrap()).unwrap();
    let initial = memory.bytes();
    assert!(initial > 4096);
    let alias = memory.clone();
    let mut table = std::collections::BTreeMap::new();
    table.insert(String::from("active"), String::from("retained"));
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let memory = memory.clone();
            scope.spawn(move || {
                for _ in 0..64 {
                    memory.try_grow_by(1).unwrap();
                }
            });
        }
    });
    assert_eq!(memory.bytes(), initial + 128);
    memory.try_grow_by(budget - storage.status().read_bytes).unwrap();
    let before = memory.bytes();
    assert!(memory.try_grow_by(1).is_err());
    assert_eq!(memory.bytes(), before);
    assert_eq!(table.get("active").unwrap(), "retained");
    assert!(session.try_memory(NonZeroUsize::new(usize::MAX).unwrap()).is_err());
    drop(table);
    drop(memory);
    drop(session);
    assert_eq!(storage.status().read_bytes, budget);
    drop(alias);
    assert_eq!(storage.status().read_bytes, 0);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn session_file_pressure() {
    let settings = paged();
    let budget = settings.budget.read_memory_bytes.get();
    let (storage, data) = manager(settings).await;
    write(&storage, "a", "one").await;
    let mut writer = storage
        .create_file(harmos_storage::FileMetadata {
            id: harmos_storage::Identifier::new("mixed").unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: Some(3),
            expected_sha256: None,
        })
        .await
        .unwrap();
    writer.write_all(b"abc".to_vec()).await.unwrap();
    let file = writer.finish().await.unwrap();
    storage.flush().await.unwrap();
    let session = storage.read_session();
    let mut typed = session
        .read_objects::<Experiment>(ObjectRead {
            at: storage.cursor_saved(),
            key: None,
            versions: ObjectVersions::Latest,
        })
        .await
        .unwrap();
    let page = typed.next().await.unwrap().unwrap();
    let mut reader = session
        .read_file(harmos_storage::FileRead {
            source: harmos_storage::FileSource::Stored(file),
            range: harmos_storage::ByteRange { offset: 0, limit: None },
        })
        .await
        .unwrap();
    let memory = session.try_memory(NonZeroUsize::MIN).unwrap();
    memory.try_grow_by(budget - storage.status().read_bytes).unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(1), reader.next()).await.unwrap();
    assert!(matches!(result, Err(Error::ResourceExhausted { .. })));
    assert_eq!(page[0].value.label, "one");
    drop(reader);
    drop(typed);
    drop(page);
    drop(memory);
    drop(session);
    data.snapshots_released().await;
    assert_eq!(storage.status().read_bytes, 0);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn session_arrow_pressure() {
    let mut settings = paged();
    settings.request.max_active_queries = NonZeroUsize::MIN;
    let budget = settings.budget.read_memory_bytes.get();
    let (storage, data) = manager(settings).await;
    storage.write_schema(waveforms::schema(), waveforms::definition()).await.unwrap();
    let input = waveforms::inputs(&waveforms::samples(1)).remove(0);
    storage.write_records(vec![input.record]).await.unwrap();
    storage.flush().await.unwrap();
    let session = storage.read_session();
    let mut query = session
        .query(Query {
            at: storage.cursor_saved(),
            source: Source::Records(samples()),
            transforms: Vec::new(),
            slice: RowSlice::default(),
        })
        .await
        .unwrap();
    assert!(query.next().await.unwrap().is_some());
    let item = query.next().await.unwrap().unwrap();
    let harmos_storage::query::QueryArrow::Batch(batch) = item else {
        panic!("expected batch");
    };
    let projected = batch.try_map(0, |batch| Ok::<_, Error>(batch.num_rows())).unwrap();
    let read = RecordRead {
        at: storage.cursor_saved(),
        selection: samples(),
        after: 0,
    };
    assert!(matches!(session.read_records::<Sample>(read).await, Err(Error::ResourceExhausted { resource: "active storage queries" })));
    drop(query);
    data.snapshots_released().await;
    let mut records = session
        .read_records::<Sample>(RecordRead {
            at: storage.cursor_saved(),
            selection: samples(),
            after: 0,
        })
        .await
        .unwrap();
    let memory = session.try_memory(NonZeroUsize::MIN).unwrap();
    memory.try_grow_by(budget - storage.status().read_bytes).unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(1), records.next()).await.unwrap();
    assert!(matches!(result, Err(Error::ResourceExhausted { .. })));
    assert_eq!(*projected, 1);
    drop(records);
    drop(memory);
    drop(session);
    data.snapshots_released().await;
    assert!(storage.status().read_bytes > 0);
    drop(projected);
    assert_eq!(storage.status().read_bytes, 0);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn receipt_claim_preserves() {
    use harmos_storage::MemorySize;
    let (storage, _) = manager(paged()).await;
    let session = storage.read_session();
    let count = NonZeroUsize::new(2).unwrap();
    let bound = storage.object_receipts_memory(count).unwrap();
    let claim = session.try_memory(NonZeroUsize::new(bound).unwrap()).unwrap();
    let mut receipts = storage
        .write_object_group(storage.settings().request.max_pending_write_bytes, count, |group| {
            group.write(&Experiment { id: "a".into(), label: "one".into() })?;
            group.write(&Experiment { id: "b".into(), label: "two".into() })
        })
        .await
        .unwrap();
    assert_eq!(receipts.capacity(), count.get());
    assert_eq!(receipts.memory_size(), bound);
    let cursor = receipts.pop().unwrap().position;
    drop(receipts);
    let pending = (cursor, claim);
    assert!(storage.flush_until(&pending.0).await.unwrap().covers(&pending.0).unwrap());
    assert!(storage.status().read_bytes > 0);
    // Keeping The Accepted Receipt Reuses Its Frontier Rather Than Reissuing A Group.
    assert_eq!(storage.cursor_accepted(), pending.0);
    let mutation = harmos_storage::MutationId {
        window: storage.mutation_window().unwrap().id,
        key: harmos_storage::Identifier::new("receipts").unwrap(),
    };
    let value = Experiment {
        id: "once".into(),
        label: "replayed".into(),
    };
    let object = harmos_storage::ObjectValue {
        schema: harmos_domain::model::storage::catalog::object_schema::<Experiment>().unwrap(),
        item: harmos_storage::ObjectKey::new("once", NonZeroUsize::new(128).unwrap()).unwrap(),
        payload: harmos_storage::Message::encode(&value),
    };
    let bound = storage.object_receipts_memory(NonZeroUsize::MIN).unwrap();
    let first_claim = session.try_memory(NonZeroUsize::new(bound).unwrap()).unwrap();
    let first = storage.write_objects_once(&mutation, vec![object.clone()]).await.unwrap();
    let accepted = storage.cursor_accepted();
    let retry_claim = session.try_memory(NonZeroUsize::new(bound).unwrap()).unwrap();
    let replay = storage.write_objects_once(&mutation, vec![object]).await.unwrap();
    assert_eq!(first, replay);
    assert_eq!(first.capacity(), 1);
    assert_eq!(replay.capacity(), 1);
    assert_eq!(first.memory_size(), bound);
    assert_eq!(replay.memory_size(), bound);
    assert_eq!(storage.cursor_accepted(), accepted);
    drop(first);
    drop(replay);
    drop(first_claim);
    drop(retry_claim);
    drop(pending);
    drop(session);
    assert_eq!(storage.status().read_bytes, 0);
    storage.close().await.unwrap();
}
