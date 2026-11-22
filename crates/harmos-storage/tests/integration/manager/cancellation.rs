/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::{Arc, atomic::Ordering};
use std::time::Duration;

// Third Party Imports
use harmos_domain::Identifier;
use harmos_storage::*;

// Local Imports
use super::backend::Backend;
use super::backend::RecoveryBackend;
use super::backend::history;
use super::backend::storage_settings;
use super::waveforms;

/* === Private Functions === */

// Start Controlled Storage With One Active Query And One Pending Write Batch.
async fn start() -> (Arc<Backend>, StorageManager) {
    let mut settings = storage_settings();
    settings.request.max_active_queries = NonZeroUsize::new(1).unwrap();
    settings.request.max_pending_write_batches = NonZeroUsize::new(1).unwrap();
    let data = Arc::new(Backend::new(history()));
    let storage: StorageManager = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: Arc::new(RecoveryBackend::new(history())),
        },
        settings,
    )
    .unwrap();
    super::fixture::start(&storage).await;
    (data, storage)
}

/* === Tests === */

#[tokio::test]
async fn dropped_write_retained() {
    let (data, storage) = start().await;
    let schema = waveforms::schema();
    let first = Identifier::new("first").unwrap();
    let second = Identifier::new("cancelled").unwrap();
    let record = |item: &Identifier| RecordInput {
        schema: schema.clone(),
        scope: None,
        item: item.clone(),
        source: None,
        source_time: None,
        payload: ([1, 2, 3]).to_vec(),
    };

    // A Controlled Backend Gate Separates Acceptance From Durability.
    data.control.block_write.store(true, Ordering::SeqCst);
    let accepted = storage.append_records(vec![record(&first)]).await.unwrap();
    let flushing = {
        let storage = storage.clone();
        tokio::spawn(async move { storage.flush().await })
    };
    data.control.write_entered.notified().await;
    assert_eq!(storage.get_saved_position().sequence, 0);
    let batch = [record(&second)];
    let mut waiting = Box::pin(storage.append_records((batch).to_vec()));
    assert!(
        std::future::poll_fn(|context| std::task::Poll::Ready(std::future::Future::poll(waiting.as_mut(), context).is_pending())).await
    );
    drop(waiting);
    // Dropping The Future Must Not Retract The Already Accepted Batch.
    assert_eq!(storage.cursor_accepted(), accepted);
    data.control.block_write.store(false, Ordering::SeqCst);
    data.control.write_release.notify_one();
    assert_eq!(flushing.await.unwrap().unwrap(), accepted);
    assert_eq!(data.records.lock().unwrap().len(), 1);
    assert_eq!(data.records.lock().unwrap().values().next().unwrap().item, first);
    assert_eq!(storage.status().pending_bytes, 0);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn cancelled_query_releases() {
    let mut settings = storage_settings();
    settings.request.max_active_queries = NonZeroUsize::MIN;
    settings.request.max_pending_write_batches = NonZeroUsize::MIN;
    let data = std::sync::Arc::new(super::backend::Backend::new(history()));
    let backends = StorageBackends {
        primary: data,
        state: std::sync::Arc::new(RecoveryBackend::new(history())),
    };
    let storage: StorageManager = StorageManager::new(backends, settings).unwrap();
    super::fixture::start(&storage).await;
    let accepted = storage.cursor_accepted();
    // Cancelling A Queued Query Releases Its Admission Slot.
    let query = || harmos_domain::model::storage::query::Query {
        merge: None,
        at: accepted.clone(),
        source: harmos_storage::query::Source::Files(harmos_storage::query::FileSelection { scope: None, item: None }),
        transforms: vec![],
        slice: harmos_storage::query::RowSlice { offset: 0, limit: None },
    };
    let first_reader = storage.query(query()).await.unwrap();
    let mut waiting = Box::pin(storage.query(query()));
    assert!(
        std::future::poll_fn(|context| std::task::Poll::Ready(std::future::Future::poll(waiting.as_mut(), context).is_pending())).await
    );
    drop(waiting);
    drop(first_reader);
    let mut following = storage.query(query()).await.unwrap();
    assert!(matches!(following.next().await.unwrap(), Some(harmos_query::QueryArrow::Schema(_))));
    assert!(following.next().await.unwrap().is_none());
    assert_eq!(following.completion().unwrap().rows, 0);
    drop(following);
    storage.close().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn close_cancels_an_in_flight_reader() {
    let (data, storage) = start().await;
    let file = storage
        .write_file(
            FileMetadata {
                id: Identifier::new("closing-reader").unwrap(),
                scope: None,
                media_type: "application/octet-stream".into(),
                expected_length: None,
                expected_sha256: None,
            },
            b"retained".to_vec(),
        )
        .await
        .unwrap();
    storage.flush().await.unwrap();
    let mut reader = storage
        .read_file(FileRead {
            source: harmos_domain::model::storage::files::FileSource::Stored(file.clone()),
            range: ByteRange { offset: 0, limit: None },
        })
        .await
        .unwrap();
    data.control.block_read.store(true, Ordering::SeqCst);
    let reading = tokio::spawn(async move { reader.next().await });
    data.control.read_entered.notified().await;

    tokio::time::timeout(Duration::from_secs(1), storage.close())
        .await
        .expect("close cancels the read without spending its I/O deadline")
        .unwrap();
    assert!(matches!(reading.await.unwrap(), Err(Error::Closing { .. })));
    assert!(storage.status().closed);
    assert!(storage.get_saved_position().covers(&file.position).unwrap());
}

#[tokio::test(start_paused = true)]
async fn close_ends_live_followers_and_open_producers() {
    let (data, storage) = start().await;
    let mut private = storage
        .create_file(FileMetadata {
            id: Identifier::new("private").unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: None,
            expected_sha256: None,
        })
        .await
        .unwrap();
    private.write_all(b"unfinished".to_vec()).await.unwrap();
    let mut public = storage
        .produce_file(FileMetadata {
            id: Identifier::new("public").unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: None,
            expected_sha256: None,
        })
        .await
        .unwrap();
    public.write_all(b"retained".to_vec()).await.unwrap();
    let session = public.session().unwrap().clone();
    let file = StagedFile {
        history: session.history.clone(),
        id: session.id.clone(),
    };
    let mut production = storage.follow_production(&file, public.length()).await.unwrap();
    let mut live = storage
        .follow::<StoredRecord>(
            None,
            query::RecordSelection {
                schema: waveforms::schema(),
                scope: None,
                item: None,
                source_start: None,
                source_end: None,
            },
        )
        .await
        .unwrap();
    let mut production_next = std::pin::pin!(production.next_batch(NonZeroUsize::MIN));
    let mut live_next = std::pin::pin!(live.next());
    assert!(futures_util::FutureExt::now_or_never(production_next.as_mut()).is_none());
    assert!(futures_util::FutureExt::now_or_never(live_next.as_mut()).is_none());

    tokio::time::timeout(Duration::from_secs(1), storage.close()).await.unwrap().unwrap();
    assert!(matches!(production_next.await, Err(Error::Closing { .. })));
    assert!(matches!(live_next.await, Err(Error::Closing { .. })));
    assert!(private.write_all(b"late".to_vec()).await.is_err());
    assert!(public.write_all(b"late".to_vec()).await.is_err());
    assert!(!data.staged.lock().unwrap().contains_key("private"));
    assert_eq!(data.staged.lock().unwrap()["public"].1, b"retained");

    let reopened = storage.new_session().unwrap();
    super::fixture::start(&reopened).await;
    let mut resumed = reopened.resume_file(&session).await.unwrap();
    assert_eq!(resumed.length(), 8);
    resumed.write_all(b" bytes".to_vec()).await.unwrap();
    let reference = resumed.finish().await.unwrap();
    reopened.close().await.unwrap();
    assert_eq!(data.blobs.lock().unwrap()[reference.metadata.id.as_str()].1, b"retained bytes");
}

#[tokio::test(start_paused = true)]
async fn close_drains_an_admitted_deletion() {
    let (data, storage) = start().await;
    let reference = storage
        .write_file(
            FileMetadata {
                id: Identifier::new("deleted").unwrap(),
                scope: None,
                media_type: "application/octet-stream".into(),
                expected_length: None,
                expected_sha256: None,
            },
            b"bytes".to_vec(),
        )
        .await
        .unwrap();
    storage.flush().await.unwrap();
    let file = StagedFile {
        history: reference.position.history.clone(),
        id: reference.metadata.id.clone(),
    };
    let mutation = MutationId {
        window: storage.mutation_window().unwrap().id,
        key: Identifier::new("delete").unwrap(),
    };
    data.control.block_delete.store(true, Ordering::SeqCst);
    let deletion = storage.delete_file_once(&mutation, &file);
    tokio::pin!(deletion);
    tokio::select! {
        biased;
        _ = &mut deletion => panic!("the admitted deletion waits at its backend gate"),
        () = data.control.delete_entered.notified() => {}
    }
    let closing = storage.close();
    tokio::pin!(closing);
    assert!(futures_util::FutureExt::now_or_never(closing.as_mut()).is_none());
    data.control.block_delete.store(false, Ordering::SeqCst);
    data.control.delete_release.notify_one();
    tokio::time::timeout(Duration::from_secs(1), closing).await.unwrap().unwrap();
    assert!(storage.status().closed);
    assert!(!data.blobs.lock().unwrap().contains_key("deleted"));
    assert_eq!(data.mutation_receipts.lock().unwrap()[&mutation].value, MutationValue::Deleted(file.clone()));
}

#[tokio::test(start_paused = true)]
async fn close_wakes_a_query_waiting_for_an_idle_reader() {
    let (_, storage) = start().await;
    let query = || harmos_domain::model::storage::query::Query {
        merge: None,
        at: storage.get_saved_position(),
        source: query::Source::Files(query::FileSelection { scope: None, item: None }),
        transforms: Vec::new(),
        slice: query::RowSlice { offset: 0, limit: None },
    };
    let _reader = storage.query(query()).await.unwrap();
    let queued = storage.query(query());
    tokio::pin!(queued);
    assert!(futures_util::FutureExt::now_or_never(queued.as_mut()).is_none());
    tokio::time::timeout(Duration::from_secs(1), storage.close()).await.unwrap().unwrap();
    assert!(matches!(queued.await, Err(Error::Closing { .. })));
}
