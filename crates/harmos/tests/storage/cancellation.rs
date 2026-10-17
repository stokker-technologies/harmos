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
use std::num::NonZeroUsize;
use std::sync::{Arc, atomic::Ordering};
use std::task::Poll;

// Third Party Imports
use harmos::storage::*;
use harmos::{Identifier, Runtime};

// Local Imports
use super::backend::{Data, Recovery, history, storage_settings};
use super::support::{Bank, Ledger, catalog};
use super::waveforms;

/* === Constants === */
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
async fn cancellation() {
    let directory = tempfile::tempdir().unwrap();
    let mut settings = storage_settings();
    settings.request.max_active_queries = NonZeroUsize::new(1).unwrap();
    settings.request.max_pending_write_batches = NonZeroUsize::new(1).unwrap();
    let data = Arc::new(Data::<()>::journaled(directory.path(), history()).unwrap());
    let storage: StorageManager = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: Arc::new(Recovery::new(history())),
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
    let first = Identifier::new("first").unwrap();
    let second = Identifier::new("cancelled").unwrap();
    let record = |item| RecordWrite {
        schema: &schema,
        scope: None,
        item,
        source: None,
        source_time: None,
        payload: &[1, 2, 3],
    };

    // A Controlled Backend Gate Separates Acceptance From Durability.
    data.control.block_write.store(true, Ordering::SeqCst);
    let accepted = storage.write_records(&[record(&first)]).await.unwrap();
    let flushing = {
        let storage = storage.clone();
        tokio::spawn(async move { storage.flush().await })
    };
    data.control.write_entered.notified().await;
    assert_eq!(storage.cursor_saved().sequence, 0);
    let batch = [record(&second)];
    let mut waiting = Box::pin(storage.write_records(&batch));
    assert!(matches!(
        std::future::poll_fn(|cx| Poll::Ready(std::future::Future::poll(waiting.as_mut(), cx)))
            .await,
        Poll::Pending
    ));
    drop(waiting);
    // Dropping The Future Must Not Retract The Already Accepted Batch.
    assert_eq!(storage.cursor_accepted(), accepted);
    data.control.block_write.store(false, Ordering::SeqCst);
    data.control.write_release.notify_one();
    assert_eq!(flushing.await.unwrap().unwrap(), accepted);
    assert_eq!(data.records.lock().unwrap().len(), 1);
    assert_eq!(
        data.records.lock().unwrap().values().next().unwrap().item,
        first
    );
    assert_eq!(storage.status().pending_bytes, 0);
    // Cancelling A Queued Query Releases Its Admission Slot.
    let query = || harmos::storage::query::Query {
        at: accepted.clone(),
        source: harmos::storage::query::Source::Files(harmos::storage::query::FileSelection {
            scope: None,
            item: None,
        }),
        transforms: vec![],
        slice: harmos::storage::query::RowSlice {
            offset: 0,
            limit: None,
        },
    };
    let first_reader = storage.query(query()).await.unwrap();
    let mut waiting = Box::pin(storage.query(query()));
    assert!(matches!(
        std::future::poll_fn(|cx| Poll::Ready(std::future::Future::poll(waiting.as_mut(), cx)))
            .await,
        Poll::Pending
    ));
    drop(waiting);
    drop(first_reader);
    let mut following = storage.query(query()).await.unwrap();
    assert!(matches!(
        following.next().await.unwrap(),
        Some(harmos::storage::query::QueryArrow::Schema(_))
    ));
    assert!(following.next().await.unwrap().is_none());
    assert_eq!(following.completion().unwrap().rows, 0);
    drop(following);
    storage.close().await.unwrap();
    runtime.stop().await.unwrap();
}
