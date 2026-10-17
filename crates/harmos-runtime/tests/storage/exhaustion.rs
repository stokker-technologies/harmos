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
use std::sync::Arc;

// Third Party Imports
use harmos_runtime::storage::*;
use harmos_runtime::{Error, Identifier, Runtime};

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
async fn exhaustion() {
    let directory = tempfile::tempdir().unwrap();
    let mut settings = storage_settings();
    settings.request.max_pending_write_bytes = NonZeroUsize::new(8192).unwrap();
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

    // An Impossible Batch Is Refused Before Ownership Or Cursor Advancement.
    let schema = waveforms::schema();
    let item = Identifier::new("oversized").unwrap();
    let payload = waveforms::binary(16 * 1024);
    let result = storage
        .write_records(&[RecordWrite {
            schema: &schema,
            scope: None,
            item: &item,
            source: None,
            source_time: None,
            payload: &payload,
        }])
        .await;
    assert!(matches!(
        result,
        Err(Error::ResourceExhausted { .. }) | Err(Error::InvalidInput { .. })
    ));
    assert_eq!(storage.cursor_accepted().sequence, 0);
    assert_eq!(storage.status().pending_bytes, 0);
    assert!(data.records.lock().unwrap().is_empty());

    // Refusal Does Not Poison The Writer; A Following Small Batch Completes.
    storage
        .write_records(&[RecordWrite {
            schema: &schema,
            scope: None,
            item: &item,
            source: None,
            source_time: None,
            payload: &[1, 2, 3],
        }])
        .await
        .unwrap();
    let durable = storage.flush().await.unwrap();
    assert_eq!(durable.sequence, 1);
    assert_eq!(data.records.lock().unwrap().len(), 1);
    storage.close().await.unwrap();
    runtime.stop().await.unwrap();
}
