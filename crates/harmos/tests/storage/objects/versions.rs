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

// Third Party Imports

// Local Imports
use super::*;
use harmos::storage::{ObjectVersionToken, StorageBatch, StoragePart};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("version_survives_uncertain", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let admitted = f.storage.write_object(&document("running")).await.unwrap();
                    let json = serde_json::to_vec(&admitted.version).unwrap();
                    let version: ObjectVersionToken = serde_json::from_slice(&json).unwrap();
                    assert_eq!(version, admitted.version);
                    f.primary.control.flush_failures.store(1, Ordering::SeqCst);
                    assert!(f.storage.flush().await.is_err());
                    assert_eq!(
                        f.primary.objects.lock().unwrap()[&admitted.position.sequence].version,
                        version
                    );
                    f.storage.flush_until(&admitted.position).await.unwrap();
                    f.storage
                        .wait_replicas(&admitted.position, &[Identifier::new("remote").unwrap()])
                        .await
                        .unwrap();
                    assert_eq!(
                        f.primary.objects.lock().unwrap()[&admitted.position.sequence].version,
                        version
                    );
                    assert_eq!(
                        f.replica.objects.lock().unwrap()[&admitted.position.sequence].version,
                        version
                    );
                    f.storage.close().await.unwrap();
                    let reopened = StorageManager::new(
                        StorageBackends {
                            primary: f.primary.clone(),
                            recovery: Arc::new(Recovery::new(history())),
                            replicas: Vec::new(),
                        },
                        StorageSettings::default(),
                    )
                    .unwrap();
                    reopened.start(&f.runtime.services).await.unwrap();
                    let found = reopened
                        .read_object::<Document>(
                            &"document-1".to_owned(),
                            ObjectVersion::Exact(version.clone()),
                            limits().bytes,
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(found.version, version);
                    assert_eq!(found.position, admitted.position);
                    assert_eq!(found.value, document("running"));
                    let later = reopened.write_object(&document("finished")).await.unwrap();
                    assert_ne!(later.version, version);
                    reopened.close().await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("tokens_cannot_substituted", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let first = f.storage.write_object(&document("first")).await.unwrap();
                    let mut other = document("second");
                    other.id = "document-2".into();
                    let second = f.storage.write_object(&other).await.unwrap();
                    assert_ne!(first.version, second.version);
                    f.storage.flush().await.unwrap();
                    assert!(
                        f.storage
                            .read_object::<Document>(
                                &other.id,
                                ObjectVersion::Exact(first.version),
                                limits().bytes
                            )
                            .await
                            .unwrap()
                            .is_none()
                    );
                    assert!(
                        f.storage
                            .read_object::<Document>(
                                &"document-1".to_owned(),
                                ObjectVersion::Exact(second.version),
                                limits().bytes
                            )
                            .await
                            .unwrap()
                            .is_none()
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("version_pages_points", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let first = f.storage.write_object(&document("first")).await.unwrap();
                    let second = f.storage.write_object(&document("second")).await.unwrap();
                    f.storage.flush().await.unwrap();
                    let original =
                        f.primary.objects.lock().unwrap()[&first.position.sequence].clone();
                    let mut forged = original.clone();
                    forged.version = second.version.clone();
                    let entries = [forged.clone()];
                    let parts = [StoragePart::Objects(&entries)];
                    assert!(StorageBatch::new(&parts, limits().bytes).is_err());
                    f.primary
                        .objects
                        .lock()
                        .unwrap()
                        .insert(first.position.sequence, forged);
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    // The backend cannot match the genuine token after it has been corrupted.
                    assert!(
                        f.storage
                            .read_object_in::<Document>(
                                &view,
                                &key(),
                                ObjectVersion::Exact(first.version),
                                limits().bytes
                            )
                            .await
                            .unwrap()
                            .is_none()
                    );
                    let mut reader = f.storage.query_in(&view, revisions()).unwrap();
                    assert!(reader.next_page().await.is_err());
                    assert!(reader.next_page().await.is_err());
                    drop(reader);
                    drop(view);
                    // Make the corrupt version the latest row as well, exercising point validation.
                    let mut corrupted_latest =
                        f.primary.objects.lock().unwrap()[&second.position.sequence].clone();
                    corrupted_latest.version = original.version.clone();
                    f.primary
                        .objects
                        .lock()
                        .unwrap()
                        .insert(second.position.sequence, corrupted_latest);
                    assert!(
                        f.storage
                            .read_object::<Document>(
                                &"document-1".to_owned(),
                                ObjectVersion::Latest,
                                limits().bytes
                            )
                            .await
                            .is_err()
                    );
                    f.primary
                        .objects
                        .lock()
                        .unwrap()
                        .insert(first.position.sequence, original);
                    // Restore the authentic admission token before replica cleanup.
                    let mut restored =
                        f.primary.objects.lock().unwrap()[&second.position.sequence].clone();
                    restored.version = second.version;
                    f.primary
                        .objects
                        .lock()
                        .unwrap()
                        .insert(second.position.sequence, restored);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("projection_preserves_fields", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let admitted = f.storage.write_object(&document("original")).await.unwrap();
                    f.storage.flush().await.unwrap();
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    let saved = view.saved().unwrap();
                    let stored = f
                        .storage
                        .read_object_in::<Document>(
                            &view,
                            &"document-1".to_owned(),
                            ObjectVersion::Latest,
                            limits().bytes,
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    let original = stored.value.status.as_ptr();
                    f.storage.write_object(&document("later")).await.unwrap();
                    f.storage.flush().await.unwrap();
                    let projected = view
                        .project_owned(stored, NonZeroUsize::new(128).unwrap(), |revision, _| {
                            assert_eq!(revision.position, admitted.position);
                            assert_eq!(revision.version, admitted.version);
                            assert_eq!(revision.value.status.as_ptr(), original);
                            Ok(revision.value.status.into_bytes())
                        })
                        .unwrap();
                    assert_eq!(projected.as_ptr(), original);
                    assert_eq!(projected.as_slice(), b"original");
                    assert_eq!(view.saved().unwrap(), saved);
                    drop(projected);
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("projection_keeps_view", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = StorageSettings::default();
                    settings.query_execution.timeout = Duration::from_secs(2);
                    let f = Fixture::new(settings).await;
                    f.storage.write_object(&document("original")).await.unwrap();
                    f.storage.flush().await.unwrap();
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    let stored = f
                        .storage
                        .read_object_in::<Document>(
                            &view,
                            &"document-1".to_owned(),
                            ObjectVersion::Latest,
                            limits().bytes,
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    let retry = f
                        .storage
                        .read_object_in::<Document>(
                            &view,
                            &"document-1".to_owned(),
                            ObjectVersion::Latest,
                            limits().bytes,
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    let worker_view = view.clone();
                    let (ready, started) = tokio::sync::oneshot::channel();
                    let (release, released) = std::sync::mpsc::channel();
                    let mut accepted = tokio::task::spawn_blocking(move || {
                        worker_view.project_owned(
                            stored,
                            NonZeroUsize::new(128).unwrap(),
                            |revision, _| {
                                ready.send(()).unwrap();
                                released.recv().unwrap();
                                Ok(revision.value.status.into_bytes())
                            },
                        )
                    });
                    started.await.unwrap();
                    let held = f.storage.status().read_bytes;
                    let mut waiting =
                        Box::pin(tokio::time::timeout(Duration::from_secs(1), &mut accepted));
                    assert!(
                        std::future::poll_fn(|context| {
                            use std::future::Future;
                            std::task::Poll::Ready(waiting.as_mut().poll(context).is_pending())
                        })
                        .await
                    );
                    tokio::time::advance(Duration::from_secs(3)).await;
                    assert!(waiting.await.is_err());
                    assert_eq!(f.storage.status().read_bytes, held);
                    release.send(()).unwrap();
                    assert!(matches!(
                        accepted.await.unwrap(),
                        Err(harmos::Error::ResourceExhausted { .. })
                    ));
                    let called = std::cell::Cell::new(false);
                    assert!(matches!(
                        view.clone().project_owned(
                            retry,
                            NonZeroUsize::new(128).unwrap(),
                            |revision, _| {
                                called.set(true);
                                Ok(revision.value.status.into_bytes())
                            }
                        ),
                        Err(harmos::Error::ResourceExhausted { .. })
                    ));
                    assert!(!called.get());
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("projection_keeps_input", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = StorageSettings::default();
                    // Include The Fixture's Pinned Maps And Retention Coverage Metadata.
                    // The Independently Asserted Decoded Profile Remains 4096 Bytes.
                    settings.query_execution.memory_bytes = NonZeroUsize::new(20 * 1024).unwrap();
                    settings.query_decoded_bytes = NonZeroUsize::new(4096).unwrap();
                    let f = Fixture::new(settings).await;
                    let admitted = f.storage.write_object(&document("original")).await.unwrap();
                    f.storage.flush().await.unwrap();
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    let stored = f
                        .storage
                        .read_object_in::<Document>(
                            &view,
                            &"document-1".to_owned(),
                            ObjectVersion::Latest,
                            limits().bytes,
                        )
                        .await
                        .unwrap()
                        .unwrap()
                        .try_into_shared()
                        .unwrap();
                    let held = f.storage.status().read_bytes;
                    assert_eq!(view.decoded_bytes().unwrap().get(), 4096);
                    assert_eq!(
                        view.execution_limits().unwrap().unwrap().memory_bytes.get(),
                        20 * 1024
                    );
                    assert_eq!(f.storage.status().read_bytes, held);
                    let pointer = stored.value.status.as_ptr();
                    let refused = view.project_shared::<_, Vec<u8>>(
                        &stored,
                        NonZeroUsize::new(128).unwrap(),
                        |_, _| {
                            Err(harmos::Error::InvalidInput {
                                field: "projection",
                                reason: "retry actual input",
                            })
                        },
                    );
                    assert!(refused.is_err());
                    assert_eq!(f.storage.status().read_bytes, held);
                    assert_eq!(stored.value.status.as_ptr(), pointer);
                    let projected = view
                        .project_shared(
                            &stored.clone(),
                            NonZeroUsize::new(128).unwrap(),
                            |revision, _| {
                                assert_eq!(revision.position, admitted.position);
                                assert_eq!(revision.version, admitted.version);
                                assert_eq!(revision.value.status.as_ptr(), pointer);
                                Ok(revision.value.status.as_bytes().to_vec())
                            },
                        )
                        .unwrap();
                    assert_eq!(projected.as_slice(), b"original");
                    assert_eq!(stored.value.status.as_ptr(), pointer);
                    drop((projected, stored, view));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("revision_not_valid", || {
            let value = serde_json::json!({"history": history(), "revision": 0});
            assert!(serde_json::from_value::<ObjectVersionToken>(value).is_err());
        });
    }
}

/* === Tests === */
