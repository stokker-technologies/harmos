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
