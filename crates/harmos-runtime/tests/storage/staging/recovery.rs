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
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
// Third Party Imports
// Local Imports
use super::metadata;
use crate::storage::{Bank, Data, Fixture, Ledger, Recovery, Runtime, catalog, config, operations};
use harmos_runtime::storage::{StagedFile, StorageBackends, StorageManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
// Copy A Crash Image Without Letting The Original Manager's Close Save Its Tail.
fn crash_image(source: &Fixture) -> Arc<Data> {
    let data = Data::new(source.data.history.clone());
    let data = Arc::new(data);
    *data.saved_productions.lock().unwrap() = source.data.saved_productions.lock().unwrap().clone();
    *data.file_generations.lock().unwrap() = source.data.file_generations.lock().unwrap().clone();
    *data.resumable_files.lock().unwrap() = source.data.resumable_files.lock().unwrap().clone();
    *data.staged.lock().unwrap() = source.data.staged.lock().unwrap().clone();
    *data.abandoned_files.lock().unwrap() = source.data.abandoned_files.lock().unwrap().clone();
    *data.deleted_files.lock().unwrap() = source.data.deleted_files.lock().unwrap().clone();
    data.lost_files_through.store(
        source.data.lost_files_through.load(Ordering::SeqCst),
        Ordering::SeqCst,
    );
    *data.blobs.lock().unwrap() = source.data.blobs.lock().unwrap().clone();
    let saved = source.data.saved.load(Ordering::SeqCst) as u64;
    *data.files.lock().unwrap() = source
        .data
        .files
        .lock()
        .unwrap()
        .iter()
        .filter(|(position, _)| **position <= saved)
        .map(|(position, file)| (*position, file.clone()))
        .collect();
    macro_rules! restore_saved {
        ($($field:ident),*) => {$(
            *data.$field.lock().unwrap() = source.data.$field.lock().unwrap().iter()
                .filter(|(position, _)| **position <= saved)
                .map(|(position, value)| (*position, value.clone()))
                .collect();
        )*};
    }
    restore_saved!(records, rows, schemas);
    data.saved
        .store(source.data.saved.load(Ordering::SeqCst), Ordering::SeqCst);
    *data.mutation_receipts.lock().unwrap() = source.data.mutation_receipts.lock().unwrap().clone();
    data
}

pub(super) async fn restart(source: &Fixture) -> Fixture {
    let data = crash_image(source);
    let recovery = Arc::new(Recovery::new(source.data.history.clone()));
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(operations::Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    let settings = config();
    let storage = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: recovery.clone(),
        },
        settings,
    )
    .unwrap();
    storage.start(&runtime).await.unwrap();
    Fixture {
        runtime,
        storage,
        data,
        recovery,
    }
}

/* === Tests === */

#[tokio::test]
async fn restart_resumes_prefix() {
    let f = Fixture::new(config()).await;
    let mut producer = f
        .storage
        .produce_file(metadata("durable-prefix"))
        .await
        .unwrap();
    let prefix = vec![42; 137];
    producer.write_all(&prefix).await.unwrap();
    let token = producer.session().unwrap().clone();
    f.storage.flush().await.unwrap();
    producer.write_all(b"unsaved tail").await.unwrap();
    let recovered = restart(&f).await;
    let identity = StagedFile {
        history: token.history.clone(),
        id: token.id.clone(),
    };
    assert!(recovered.storage.follow_file(&identity, 138).await.is_err());
    let mut resumed = recovered.storage.resume_file(&token).await.unwrap();
    assert_eq!(resumed.length(), 137);
    resumed.write_all(b"new suffix").await.unwrap();
    let reference = resumed.finish().await.unwrap();
    let mut expected = prefix;
    expected.extend_from_slice(b"new suffix");
    assert_eq!(
        recovered.data.blobs.lock().unwrap()[reference.metadata.id.as_str()].1,
        expected
    );
    recovered.finish().await;
    f.finish().await;
}

#[tokio::test]
async fn fences_survive_restart() {
    let f = Fixture::new(config()).await;
    let mut original = f
        .storage
        .produce_file(metadata("durable-fence"))
        .await
        .unwrap();
    original.write_all(b"saved").await.unwrap();
    let first = original.session().unwrap().clone();
    f.storage.flush().await.unwrap();
    let mut current = f.storage.resume_file(&first).await.unwrap();
    current.write_all(b"volatile").await.unwrap();
    let token = current.session().unwrap().clone();
    assert_eq!(token.generation, 2);
    let recovered = restart(&f).await;
    assert!(recovered.storage.resume_file(&first).await.is_err());
    let mut resumed = recovered.storage.resume_file(&token).await.unwrap();
    assert_eq!(resumed.session().unwrap().generation, 3);
    assert_eq!(resumed.length(), 5);
    resumed.abort().await.unwrap();
    recovered.finish().await;
    f.finish().await;
}

#[tokio::test(start_paused = true)]
async fn recovery_renews_deadline() {
    let f = Fixture::new(config()).await;
    let producer = f
        .storage
        .produce_file(metadata("fresh-window"))
        .await
        .unwrap();
    let token = producer.session().unwrap().clone();
    f.storage.flush().await.unwrap();
    drop(producer);
    tokio::time::advance(Duration::from_secs(50)).await;
    let recovered = restart(&f).await;
    tokio::time::advance(Duration::from_secs(59)).await;
    let mut resumed = recovered.storage.resume_file(&token).await.unwrap();
    resumed.write_all(b"after recovery").await.unwrap();
    resumed.abort().await.unwrap();
    recovered.finish().await;
    f.finish().await;
}

#[tokio::test]
async fn checkpoint_fixes_length() {
    let f = Fixture::new(config()).await;
    let mut producer = f.storage.produce_file(metadata("fixed-cut")).await.unwrap();
    producer.write_all(b"saved").await.unwrap();
    f.data
        .control
        .block_checkpoint
        .store(true, Ordering::SeqCst);
    {
        let save = f.storage.flush();
        tokio::pin!(save);
        tokio::select! {
            biased;
            _ = &mut save => panic!("checkpoint must block"),
            _ = f.data.control.checkpoint_entered.notified() => {}
        }
        producer.write_all(b"later").await.unwrap();
        f.data
            .control
            .block_checkpoint
            .store(false, Ordering::SeqCst);
        f.data.control.checkpoint_release.notify_one();
        save.await.unwrap();
    }
    {
        let checkpoints = f.data.saved_productions.lock().unwrap();
        assert_eq!(checkpoints["fixed-cut"].0.length, 5);
        assert_eq!(checkpoints["fixed-cut"].1, b"saved");
    }
    f.finish().await;
}

#[tokio::test]
async fn close_retains_production() {
    let f = Fixture::new(config()).await;
    let mut producer = f
        .storage
        .produce_file(metadata("close-restart"))
        .await
        .unwrap();
    producer.write_all(b"retained on close").await.unwrap();
    let token = producer.session().unwrap().clone();
    f.storage.close().await.unwrap();
    assert!(
        !f.data
            .abandoned_files
            .lock()
            .unwrap()
            .contains("close-restart")
    );
    let restored: StorageManager = StorageManager::new(
        StorageBackends {
            primary: f.data.clone(),
            state: f.recovery.clone(),
        },
        config(),
    )
    .unwrap();
    restored.start(&f.runtime).await.unwrap();
    let mut resumed = restored.resume_file(&token).await.unwrap();
    assert_eq!(resumed.length(), 17);
    resumed.abort().await.unwrap();
    restored.close().await.unwrap();
    f.finish().await;
}

#[tokio::test(start_paused = true)]
async fn fence_retries_transient() {
    let f = Fixture::new(config()).await;
    let mut producer = f
        .storage
        .produce_file(metadata("fence-retry"))
        .await
        .unwrap();
    producer.write_all(b"prefix").await.unwrap();
    let token = producer.session().unwrap().clone();
    f.data.control.fence_failures.store(3, Ordering::SeqCst);
    let mut resumed = f.storage.resume_file(&token).await.unwrap();
    assert_eq!(resumed.session().unwrap().generation, 2);
    assert_eq!(resumed.length(), 6);
    assert!(producer.write_all(b"stale").await.is_err());
    assert_eq!(f.data.control.fence_failures.load(Ordering::SeqCst), 0);
    resumed.abort().await.unwrap();
    f.finish().await;
}

#[tokio::test]
async fn failed_recovery_releases() {
    let source = Fixture::new(config()).await;
    let first = source
        .storage
        .produce_file(metadata("first"))
        .await
        .unwrap();
    let second = source
        .storage
        .produce_file(metadata("second"))
        .await
        .unwrap();
    source.storage.flush().await.unwrap();
    for failure in 1..3 {
        let data = crash_image(&source);
        let recovery = Arc::new(Recovery::new(data.history.clone()));
        let settings = config();
        match failure {
            1 => {
                data.saved_productions
                    .lock()
                    .unwrap()
                    .get_mut("first")
                    .unwrap()
                    .0
                    .length += 1
            }
            // File Restoration Succeeded Before The Recovery Owner Failed.
            _ => recovery.control.fail_open.store(true, Ordering::SeqCst),
        }
        let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
            .register(catalog())
            .runtime()
            .await
            .unwrap();
        let storage: StorageManager = StorageManager::new(
            StorageBackends {
                primary: data.clone(),
                state: recovery,
            },
            settings,
        )
        .unwrap();
        assert!(storage.start(&runtime).await.is_err());
        storage.close().await.unwrap();
        assert_eq!(storage.status().read_bytes, 0);
        assert_eq!(storage.status().pending_bytes, 0);
        assert_eq!(data.saved_productions.lock().unwrap().len(), 2);
        runtime.stop().await.unwrap();
    }
    drop((first, second));
    source.finish().await;
}

#[tokio::test]
async fn receipts_replay_restart() {
    use harmos_runtime::Identifier;
    use harmos_runtime::storage::MutationId;
    let f = Fixture::new(config()).await;
    let window = f.storage.mutation_window().unwrap().id;
    let first = MutationId {
        window: window.clone(),
        key: Identifier::new("first-write").unwrap(),
    };
    let second = MutationId {
        window,
        key: Identifier::new("second-write").unwrap(),
    };
    let schema = super::super::schema();
    let item = Identifier::new("record").unwrap();
    let values = [super::super::record(&schema, &item, b"saved")];
    let original = f.storage.write_records_once(&first, &values).await.unwrap();
    f.storage
        .write_records_once(&second, &values)
        .await
        .unwrap();
    f.storage.flush_until(&original).await.unwrap();
    assert!(
        f.data
            .mutation_receipts
            .lock()
            .unwrap()
            .contains_key(&first)
    );
    assert!(
        !f.data
            .mutation_receipts
            .lock()
            .unwrap()
            .contains_key(&second)
    );
    let restored = restart(&f).await;
    let before = restored.storage.cursor_accepted();
    assert_eq!(
        restored
            .storage
            .write_records_once(&first, &values)
            .await
            .unwrap(),
        original
    );
    assert_eq!(restored.storage.cursor_accepted(), before);
    assert!(
        restored
            .storage
            .write_records_once(&second, &values)
            .await
            .is_err()
    );
    assert!(!restored.data.records.lock().unwrap().is_empty());
    restored.finish().await;
    f.finish().await;
}

#[tokio::test]
async fn deletion_survives_restart() {
    let f = Fixture::new(config()).await;
    let metadata = metadata("deleted-restart");
    let reference = f
        .storage
        .write_file(metadata.clone(), b"retained")
        .await
        .unwrap();
    let file = StagedFile {
        history: reference.position.history,
        id: metadata.id.clone(),
    };
    let mutation = harmos_runtime::storage::MutationId {
        window: f.storage.mutation_window().unwrap().id,
        key: harmos_runtime::Identifier::new("delete").unwrap(),
    };
    f.storage.delete_file_once(&mutation, &file).await.unwrap();
    let recovered = restart(&f).await;
    assert_eq!(
        recovered
            .storage
            .delete_file_once(&mutation, &file)
            .await
            .unwrap(),
        file
    );
    assert!(recovered.storage.follow_file(&file, 0).await.is_err());
    assert!(recovered.storage.produce_file(metadata).await.is_err());
    recovered.finish().await;
    f.finish().await;
}
