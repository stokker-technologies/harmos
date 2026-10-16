/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use super::{Fixture, config, save_request, write};
use harmos::Identifier;
use harmos::storage::FileMetadata;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn metadata(name: &str) -> FileMetadata {
    FileMetadata {
        id: Identifier::new(name).unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: None,
        expected_sha256: None,
    }
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("cancelled_save_retains", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut producer = f.storage.produce_file(metadata("snapshot")).await.unwrap();
                    producer.write_all(b"prefix").await.unwrap();
                    let request = save_request(&f, f.storage.cursor_accepted());
                    f.data
                        .control
                        .block_checkpoint
                        .store(true, Ordering::SeqCst);
                    let mut saving = Box::pin(f.storage.save_once(&request.mutation, &request.at));
                    tokio::select! {
                        biased;
                        _ = &mut saving => panic!("save must block"),
                        _ = f.data.control.checkpoint_entered.notified() => {}
                    }
                    drop(saving);
                    assert!(f.storage.status().read_bytes > 0);
                    f.data
                        .control
                        .block_checkpoint
                        .store(false, Ordering::SeqCst);
                    f.data.control.checkpoint_release.notify_one();
                    assert_eq!(
                        f.storage
                            .save_once(&request.mutation, &request.at)
                            .await
                            .unwrap(),
                        request.at
                    );
                    assert!(
                        f.data
                            .mutation_receipts
                            .lock()
                            .unwrap()
                            .contains_key(&request.mutation)
                    );
                    assert_eq!(f.storage.status().read_bytes, 0);
                    producer.write_all(b"later").await.unwrap();
                    f.storage
                        .save_once(&request.mutation, &request.at)
                        .await
                        .unwrap();
                    assert_eq!(
                        f.data.saved_productions.lock().unwrap()["snapshot"].1,
                        b"prefix"
                    );
                    let mut fresh = request.clone();
                    fresh.mutation.key = Identifier::new("fresh-save").unwrap();
                    f.storage
                        .save_once(&fresh.mutation, &fresh.at)
                        .await
                        .unwrap();
                    assert_eq!(
                        f.data.saved_productions.lock().unwrap()["snapshot"].1,
                        b"prefixlater"
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("close_settles_save", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut producer = f
                        .storage
                        .produce_file(metadata("close-save"))
                        .await
                        .unwrap();
                    producer.write_all(b"retained").await.unwrap();
                    let request = save_request(&f, f.storage.cursor_accepted());
                    f.data
                        .control
                        .block_checkpoint
                        .store(true, Ordering::SeqCst);
                    let mut saving = Box::pin(f.storage.save_once(&request.mutation, &request.at));
                    tokio::select! {
                        biased;
                        _ = &mut saving => panic!("save must block"),
                        _ = f.data.control.checkpoint_entered.notified() => {}
                    }
                    drop(saving);
                    let mut closing = Box::pin(f.storage.close());
                    assert!(
                        std::future::poll_fn(|cx| std::task::Poll::Ready(
                            closing.as_mut().poll(cx)
                        ))
                        .await
                        .is_pending()
                    );
                    f.data
                        .control
                        .block_checkpoint
                        .store(false, Ordering::SeqCst);
                    f.data.control.checkpoint_release.notify_one();
                    closing.await.unwrap();
                    assert!(
                        f.data
                            .mutation_receipts
                            .lock()
                            .unwrap()
                            .contains_key(&request.mutation)
                    );
                    assert_eq!(f.storage.status().read_bytes, 0);
                    f.runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("save_rejects_conflicts", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let first = write(&f.storage, "first").await;
                    let request = save_request(&f, first.clone());
                    f.storage
                        .save_once(&request.mutation, &first)
                        .await
                        .unwrap();
                    let later = write(&f.storage, "later").await;
                    f.storage.flush_until(&later).await.unwrap();
                    assert_eq!(
                        f.storage
                            .save_once(&request.mutation, &first)
                            .await
                            .unwrap(),
                        first
                    );
                    assert!(
                        f.storage
                            .save_once(&request.mutation, &later)
                            .await
                            .is_err()
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("save_refuses_snapshot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.budget.read_memory_bytes = std::num::NonZeroUsize::new(128).unwrap();
                    settings.response.batch_bytes = settings.budget.read_memory_bytes;
                    let f = Fixture::new(settings).await;
                    let request = save_request(&f, f.storage.cursor_accepted());
                    assert!(
                        f.storage
                            .save_once(&request.mutation, &request.at)
                            .await
                            .is_err()
                    );
                    assert_eq!(f.storage.status().read_bytes, 0);
                    assert_eq!(f.storage.status().mutation_bytes, 0);
                    f.finish().await;
                });
        });
    }
}
