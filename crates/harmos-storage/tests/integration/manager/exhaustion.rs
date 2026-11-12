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
use std::sync::Arc;

// Third Party Imports
use harmos_domain::Identifier;
use harmos_storage::Error;
use harmos_storage::*;

// Local Imports
use super::backend::{history, storage_settings};
use super::waveforms;

/* === Tests === */

#[tokio::test]
async fn oversized_batch_refused() {
    let mut settings = storage_settings();
    settings.request.max_pending_write_bytes = NonZeroUsize::new(8192).unwrap();
    let data = Arc::new(super::backend::Data::new(history()));
    let storage: StorageManager = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: Arc::new(super::backend::Recovery::new(history())),
        },
        settings,
    )
    .unwrap();
    super::fixture::start(&storage).await;

    // An Impossible Batch Is Refused Before Ownership Or Cursor Advancement.
    let schema = waveforms::schema();
    let item = Identifier::new("oversized").unwrap();
    let payload = waveforms::binary(16 * 1024);
    let result = storage
        .write_records(vec![RecordInput {
            schema: schema.clone(),
            scope: None,
            item: item.clone(),
            source: None,
            source_time: None,
            payload: (payload).to_vec(),
        }])
        .await;
    assert!(matches!(result, Err(Error::ResourceExhausted { .. }) | Err(Error::InvalidInput { .. })));
    assert_eq!(storage.cursor_accepted().sequence, 0);
    assert_eq!(storage.status().pending_bytes, 0);
    assert_eq!(data.cursor_saved().await.unwrap().sequence, 0);

    // Refusal Does Not Poison The Writer; A Following Small Batch Completes.
    storage
        .write_records(vec![RecordInput {
            schema: schema.clone(),
            scope: None,
            item: item.clone(),
            source: None,
            source_time: None,
            payload: ([1, 2, 3]).to_vec(),
        }])
        .await
        .unwrap();
    let durable = storage.flush().await.unwrap();
    assert_eq!(durable.sequence, 1);
    let snapshot = data.open_snapshot(&durable).await.unwrap();
    let selection = harmos_storage::query::RecordSelection {
        schema,
        scope: None,
        item: None,
        source_start: None,
        source_end: None,
    };
    let page = ScanPage {
        after: 0,
        rows: NonZeroUsize::new(2).unwrap(),
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let records = data.read_records(&snapshot, &selection, page).await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].payload, [1, 2, 3]);
    data.release_snapshot(&snapshot).await.unwrap();
    storage.close().await.unwrap();
}
