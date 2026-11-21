/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise Resumable Native Files And Durable Mutation Replay With The Storage Fixture.

/* === Modules === */

#[cfg(unix)]
mod unix;

/* === Imports === */

// Standard Library Imports
use std::future::{self, Future};
use std::io::Write;
use std::task::Poll;
use std::{fs, num::NonZeroUsize, sync::Arc, time::Duration};

// Third Party Imports
use sha2::{Digest, Sha256};
use tokio::{sync::Mutex, task::JoinHandle};

// Local Imports
use super::manager::backend::Backend;
use super::manager::backend::RecoveryBackend;
use super::manager::backend::history;
use harmos_domain::model::storage::files::{CaptureConsistency, FileEvent, FileSource, FollowStart};
use harmos_storage::*;

/* === Structs === */

struct Task(Mutex<Option<JoinHandle<Result<()>>>>);

/* === Implementations === */

impl WorkerOwner for Task {
    fn stop(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            if let Some(task) = self.0.lock().await.take() {
                task.await.map_err(|source| Error::OperationFailed {
                    operation: "join file session worker".into(),
                    source: Arc::new(source),
                })??;
            }
            Ok(())
        })
    }
}

/* === Private Functions === */

pub(super) async fn storage(primary: &Arc<Backend>) -> StorageManager {
    let pending = NonZeroUsize::new(1024 * 1024).unwrap();
    let batch = NonZeroUsize::new(4096).unwrap();
    let settings = StorageSettings {
        transfer: Default::default(),
        policy: Default::default(),
        filesystem: StorageFilesystemSettings {
            poll_interval: Duration::from_millis(100),
            max_entries: NonZeroUsize::new(100_000).unwrap(),
            event_capacity: NonZeroUsize::new(1024).unwrap(),
        },

        request: StorageRequestSettings {
            max_file_productions: NonZeroUsize::new(256).unwrap(),
            max_mutation_receipts: NonZeroUsize::new(1024).unwrap(),
            max_pending_queries: NonZeroUsize::new(8).unwrap(),
            max_active_queries: NonZeroUsize::new(2).unwrap(),
            max_active_file_operations: NonZeroUsize::new(1).unwrap(),
            max_pending_write_batches: NonZeroUsize::new(16).unwrap(),
            max_pending_write_bytes: pending,
        },
        response: StorageResponseSettings {
            batch_rows: NonZeroUsize::new(32).unwrap(),
            batch_bytes: batch,
        },
    };

    let backends = StorageBackends {
        primary: primary.clone(),
        state: Arc::new(RecoveryBackend::new(history())),
    };
    let storage = StorageManager::new(backends, settings).unwrap();
    storage
        .start_with(Arc::new(()), |mut worker| {
            let task = tokio::spawn(async move {
                worker.run().await?;
                worker.cleanup().await
            });
            Ok(Box::new(Task(Mutex::new(Some(task)))))
        })
        .await
        .unwrap();
    storage
}

pub(super) fn mutation(storage: &StorageManager, key: &str) -> MutationId {
    MutationId {
        window: storage.mutation_window().unwrap().id,
        key: Identifier::new(key).unwrap(),
    }
}

/* === Tests === */

pub(super) fn metadata(id: &str) -> FileMetadata {
    FileMetadata {
        id: Identifier::new(id).unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: None,
        expected_sha256: None,
    }
}

async fn contents(storage: &StorageManager, source: FileSource, range: ByteRange) -> Vec<u8> {
    let mut reader = storage.read_file(FileRead { source, range }).await.unwrap();
    let mut bytes = Vec::new();
    while let Some(batch) = reader.next().await.unwrap() {
        bytes.extend_from_slice(batch.bytes());
    }
    assert_eq!(reader.completion().unwrap().bytes, bytes.len() as u64);
    bytes
}

#[tokio::test]
async fn local_file_transfer() {
    let data = Arc::new(Backend::new(history()));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("outside-backend.bin");
    let expected: Vec<_> = (0..20_000).map(|value| (value % 251) as u8).collect();
    fs::write(&path, &expected).unwrap();
    let storage = storage(&data).await;
    let source = FileSource::Local(path.clone());
    let whole = ByteRange { offset: 0, limit: None };
    let mut reader = storage.read_file(FileRead { source: source.clone(), range: whole }).await.unwrap();
    let batch = reader.next().await.unwrap().unwrap();
    let view = batch.slice(2..10);
    assert_eq!(view.bytes().as_ptr(), batch.bytes().as_ptr().wrapping_add(2));
    drop((reader, batch));
    assert_eq!(view.bytes(), &expected[2..10]);
    drop(view);

    let hash = storage.hash_file(FileRead { source: source.clone(), range: whole }).await.unwrap();
    assert_eq!(hash.bytes_read, expected.len() as u64);
    assert_eq!(hash.sha256, <[u8; 32]>::from(Sha256::digest(&expected)));
    let captured = storage
        .capture_file(FileCapture {
            source,
            range: whole,
            metadata: metadata("captured"),
            consistency: CaptureConsistency::Stable,
        })
        .await
        .unwrap();
    assert!(storage.status().saved.covers(&captured.position).unwrap());
    fs::write(&path, b"changed after capture").unwrap();
    let stored = FileSource::Stored(captured.clone());
    assert_eq!(contents(&storage, stored.clone(), whole).await, expected);
    assert_eq!(storage.hash_file(FileRead { source: stored.clone(), range: whole }).await.unwrap(), hash);
    let range = ByteRange { offset: 17, limit: Some(83) };
    let second = storage
        .capture_file(FileCapture {
            source: stored,
            range,
            metadata: metadata("independent"),
            consistency: CaptureConsistency::Bounded,
        })
        .await
        .unwrap();
    assert_ne!(captured.position, second.position);
    assert_eq!(contents(&storage, FileSource::Stored(second), whole).await, expected[17..100]);
    let mut follow = storage
        .follow_file(FileFollow {
            source: FileSource::Stored(captured),
            start: FollowStart::End,
        })
        .await
        .unwrap();
    assert!(follow.next().await.unwrap().is_none());
    assert_eq!(follow.completion().unwrap().bytes, 0);
    drop(follow);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn capture_rejects_mutation() {
    let data = Arc::new(Backend::new(history()));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mutable.bin");
    let storage = storage(&data).await;
    for (index, consistency, replacement) in [(0, CaptureConsistency::Stable, vec![9; 8193]), (1, CaptureConsistency::Bounded, vec![9; 1])]
    {
        fs::write(&path, vec![7; 8192]).unwrap();
        let request = FileCapture {
            source: FileSource::Local(path.clone()),
            range: ByteRange { offset: 0, limit: None },
            metadata: metadata(&format!("mutation-{index}")),
            consistency,
        };
        let mut copying = Box::pin(storage.capture_file(request));
        // On This Current-Thread Executor, Source Open Precedes Target Admission;
        // Mutate Immediately After Admission Before The Worker Can Run The Target.
        future::poll_fn(|cx| match copying.as_mut().poll(cx) {
            Poll::Pending if storage.status().pending_batches > 0 => Poll::Ready(()),
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => panic!("capture completed before target admission: {result:?}"),
        })
        .await;
        fs::write(&path, replacement).unwrap();
        assert!(copying.await.is_err());
        storage.flush().await.unwrap();
    }
    storage.close().await.unwrap();
}

#[tokio::test]
async fn local_follow_changes() {
    let data = Arc::new(Backend::new(history()));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("growing.bin");
    fs::write(&path, b"first").unwrap();
    let storage = storage(&data).await;
    let source = FileSource::Local(path.clone());
    let mut follower = storage
        .follow_file(FileFollow {
            source: source.clone(),
            start: FollowStart::Beginning,
        })
        .await
        .unwrap();
    assert_eq!(follower.next().await.unwrap().unwrap().data().unwrap(), b"first");
    fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"second").unwrap();
    assert_eq!(follower.next().await.unwrap().unwrap().data().unwrap(), b"second");
    fs::write(&path, b"x").unwrap();
    assert_eq!(follower.next().await.unwrap().unwrap().event(), &FileEvent::Truncated { length: 1 });
    assert_eq!(follower.next().await.unwrap().unwrap().data().unwrap(), b"x");
    fs::rename(&path, path.with_extension("old")).unwrap();
    fs::write(&path, b"replacement").unwrap();
    assert_eq!(follower.next().await.unwrap().unwrap().event(), &FileEvent::Replaced);
    assert_eq!(follower.next().await.unwrap().unwrap().data().unwrap(), b"replacement");
    let mut tail = storage
        .follow_file(FileFollow {
            source: source.clone(),
            start: FollowStart::End,
        })
        .await
        .unwrap();
    fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"tail").unwrap();
    assert_eq!(tail.next().await.unwrap().unwrap().data().unwrap(), b"tail");
    let mut at = storage.follow_file(FileFollow { source, start: FollowStart::At(2) }).await.unwrap();
    assert_eq!(at.next().await.unwrap().unwrap().data().unwrap(), b"placementtail");
    drop((follower, tail, at));
    storage.close().await.unwrap();
}

#[tokio::test]
async fn sessions_survive_reopen() {
    let data = Arc::new(Backend::new(history()));
    let expected: &[u8] = b"prefix and suffix";
    let prefix: &[u8] = b"prefix";
    let suffix: &[u8] = b" and suffix";
    let digest: [u8; 32] = Sha256::digest(expected).into();
    let metadata = FileMetadata {
        id: Identifier::new("resumable").unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: Some(expected.len() as u64),
        expected_sha256: Some(digest),
    };

    // Persist A Public Prefix And Its Original Mutation Outcomes.
    let first = storage(&data).await;
    let create = mutation(&first, "create");
    let append = mutation(&first, "prefix");
    let created = first.create_file_once(&create, &metadata).await.unwrap();
    let written = first.append_file_once(&append, &created.session, 0, prefix.to_vec()).await.unwrap();
    assert_eq!(written.length, prefix.len() as u64);
    let file = StagedFile {
        history: created.session.history.clone(),
        id: created.session.id.clone(),
    };
    first.close().await.unwrap();

    // Exact Retries Replay Persisted Outcomes; Resume Fences The Old Session.
    let second = storage(&data).await;
    assert_eq!(second.create_file_once(&create, &metadata).await.unwrap(), created);
    assert_eq!(second.append_file_once(&append, &created.session, 0, prefix.to_vec()).await.unwrap(), written);
    assert!(second.append_file_once(&append, &created.session, 0, b"changed".to_vec()).await.is_err());
    let mut following = second.follow_production(&file, 0).await.unwrap();
    let batch = following.next_batch(NonZeroUsize::new(4096).unwrap()).await.unwrap().unwrap();
    assert_eq!(batch.offset(), 0);
    assert_eq!(batch.bytes(), prefix);
    drop(following);

    let resume = mutation(&second, "resume");
    let resumed = second.resume_file_once(&resume, &written.session).await.unwrap();
    assert_eq!(resumed.session.generation, written.session.generation + 1);
    assert_eq!(resumed.length, prefix.len() as u64);
    assert_eq!(second.resume_file_once(&resume, &written.session).await.unwrap(), resumed);
    let stale = mutation(&second, "stale");
    assert!(second.append_file_once(&stale, &written.session, written.length, suffix.to_vec()).await.is_err());

    let append_tail = mutation(&second, "suffix");
    let completed = second.append_file_once(&append_tail, &resumed.session, resumed.length, suffix.to_vec()).await.unwrap();
    assert_eq!(completed.length, expected.len() as u64);
    let finish = mutation(&second, "finish");
    let reference = second.finish_file_once(&finish, &resumed.session).await.unwrap();
    assert_eq!(reference.length, expected.len() as u64);
    assert_eq!(reference.sha256, digest);
    assert_eq!(second.finish_file_once(&finish, &resumed.session).await.unwrap(), reference);
    second.close().await.unwrap();

    // Reopen The Finished Publication And Verify Every Stored Byte.
    let third = storage(&data).await;
    assert_eq!(
        third.get_file_production(&file).await.unwrap(),
        FileProgress {
            length: expected.len() as u64,
            phase: FilePhase::Finished
        }
    );
    assert_eq!(third.finish_file_once(&finish, &resumed.session).await.unwrap(), reference);
    assert_eq!(third.resume_file_once(&resume, &written.session).await.unwrap(), resumed);
    let request = FileRead {
        source: FileSource::Stored(reference.clone()),
        range: ByteRange { offset: 0, limit: None },
    };
    let mut reader = third.read_file(request).await.unwrap();
    let mut actual = Vec::new();
    while let Some(batch) = reader.next().await.unwrap() {
        actual.extend_from_slice(batch.bytes());
    }
    assert_eq!(actual, expected);
    assert_eq!(reader.completion().unwrap().bytes, expected.len() as u64);
    drop(reader);
    let mut following = third.follow_production(&file, 0).await.unwrap();
    assert_eq!(following.next_batch(NonZeroUsize::new(4096).unwrap()).await.unwrap().unwrap().bytes(), expected);
    assert!(following.next_batch(NonZeroUsize::new(4096).unwrap()).await.unwrap().is_none());
    assert_eq!(following.progress().phase, FilePhase::Finished);
    drop(following);

    let deletion = mutation(&third, "delete");
    assert_eq!(third.delete_file_once(&deletion, &file).await.unwrap(), file);
    assert_eq!(third.delete_file_once(&deletion, &file).await.unwrap(), file);
    assert!(third.get_file_production(&file).await.is_err());
    third.close().await.unwrap();

    // Deletion And Earlier Terminal Replies Keep Their Original Identities.
    let fourth = storage(&data).await;
    assert_eq!(fourth.delete_file_once(&deletion, &file).await.unwrap(), file);
    assert_eq!(fourth.finish_file_once(&finish, &resumed.session).await.unwrap(), reference);
    assert!(fourth.get_file_production(&file).await.is_err());
    fourth.close().await.unwrap();
}

#[tokio::test]
async fn immutable_reader_survives_deletion_and_stops_after_snapshot_release() {
    let data = Arc::new(Backend::new(history()));
    let storage = storage(&data).await;
    let bytes = (0..20_000).map(|i| (i % 251) as u8).collect::<Vec<_>>();
    let mut producer = storage.create_file(metadata("leased-reader")).await.unwrap();
    producer.write_all(bytes.clone()).await.unwrap();
    let reference = producer.finish().await.unwrap();
    storage.flush().await.unwrap();
    let request = FileRead {
        source: FileSource::Stored(reference.clone()),
        range: ByteRange { offset: 0, limit: None },
    };
    let mut reader = storage.read_file(request.clone()).await.unwrap();
    let file = StagedFile {
        history: reference.position.history.clone(),
        id: reference.metadata.id.clone(),
    };
    storage.delete_file_once(&mutation(&storage, "delete-leased"), &file).await.unwrap();
    let mut actual = Vec::new();
    while let Some(batch) = reader.next().await.unwrap() {
        actual.extend_from_slice(batch.bytes());
    }
    assert_eq!(actual, bytes);
    assert_eq!(reader.completion().unwrap().bytes, bytes.len() as u64);
    drop(reader);
    assert!(storage.read_file(request).await.is_err());
    drop(producer);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn discovery_conformance_is_exact_bounded_and_unfinished_only() {
    // Create Unfinished Files in Deliberately Unsorted Order
    let data = Arc::new(Backend::new(history()));
    let storage = storage(&data).await;
    let scope = Identifier::new("scope").unwrap();
    let child = Identifier::new("scope.child").unwrap();
    let mut expected = Vec::new();
    for (id, selected) in [("file-c", &scope), ("file-a", &scope), ("file-b", &scope), ("child", &child)] {
        let metadata = FileMetadata {
            scope: Some(selected.clone()),
            ..metadata(id)
        };
        let created = storage.create_file_once(&mutation(&storage, id), &metadata).await.unwrap();
        if selected == &scope {
            expected.push(FileProduction {
                session: created.session,
                accepted_length: 0,
                durable_length: 0,
            });
        }
    }
    expected.sort_by(|left, right| left.session.id.cmp(&right.session.id));

    // Exclude Published Files, Private Transfers and Unscoped Files
    let mut published = storage
        .produce_file(FileMetadata {
            scope: Some(scope.clone()),
            ..metadata("published")
        })
        .await
        .unwrap();
    published.write_all(b"published".to_vec()).await.unwrap();
    published.finish().await.unwrap();
    let private = storage
        .create_file(FileMetadata {
            scope: Some(scope.clone()),
            ..metadata("private")
        })
        .await
        .unwrap();
    storage.create_file_once(&mutation(&storage, "unscoped"), &metadata("unscoped")).await.unwrap();
    conformance::file_productions(&storage, &scope, &expected).unwrap();
    conformance::file_productions(&storage, &Identifier::new("missing").unwrap(), &[]).unwrap();

    // Enforce Bytes and Foreign-History Refusal
    let limits = ReadLimits {
        rows: NonZeroUsize::MAX,
        bytes: NonZeroUsize::MIN,
    };
    assert!(matches!(storage.list_file_productions(&scope, None, None, limits), Err(Error::ResourceExhausted { .. })));
    let foreign = StorageCursor {
        history: StoreHistory {
            store: Identifier::new("foreign").unwrap(),
            epoch: Identifier::new("epoch").unwrap(),
        },
        sequence: 0,
    };
    assert!(
        storage
            .list_file_productions(
                &scope,
                None,
                Some(&foreign),
                ReadLimits {
                    rows: NonZeroUsize::MIN,
                    bytes: storage.settings().response.batch_bytes,
                }
            )
            .is_err()
    );
    drop(private);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn discovery_recovers_lost_create_responses_and_resumes_pages() {
    // Lose the Committed Create Reply and Persist Another File's Byte Prefix
    let data = Arc::new(Backend::new(history()));
    let first = storage(&data).await;
    let scope = Identifier::new("restart").unwrap();
    let lost = FileMetadata {
        scope: Some(scope.clone()),
        ..metadata("file-a")
    };
    first.create_file_once(&mutation(&first, "lost-create"), &lost).await.unwrap();
    let metadata = FileMetadata {
        scope: Some(scope.clone()),
        ..metadata("file-b")
    };
    let created = first.create_file_once(&mutation(&first, "create-b"), &metadata).await.unwrap();
    first.append_file_once(&mutation(&first, "append-b"), &created.session, 0, b"prefix".to_vec()).await.unwrap();
    let resumed = first.resume_file_once(&mutation(&first, "resume-b"), &created.session).await.unwrap();
    let limits = ReadLimits {
        rows: NonZeroUsize::MIN,
        bytes: first.settings().response.batch_bytes,
    };
    let page = first.list_file_productions(&scope, None, None, limits).unwrap();
    assert_eq!(page.productions[0].session.id, lost.id);
    assert!(page.next.is_some());
    first.close().await.unwrap();

    // Recover Through the Existing Startup Owner and Continue by Durable Identity
    let second = storage(&data).await;
    let next = second.list_file_productions(&scope, page.next.as_ref(), Some(&page.at), limits).unwrap();
    assert_eq!(
        next.productions,
        vec![FileProduction {
            session: resumed.session.clone(),
            accepted_length: 6,
            durable_length: 6,
        }]
    );
    assert!(next.next.is_none());
    let expected = vec![page.productions[0].clone(), next.productions[0].clone()];
    conformance::file_productions(&second, &scope, &expected).unwrap();
    for (index, production) in expected.iter().enumerate() {
        let claimed = second.resume_file_once(&mutation(&second, &format!("claim-{index}")), &production.session).await.unwrap();
        assert!(claimed.session.generation > production.session.generation);
        second.abandon_file_once(&mutation(&second, &format!("abandon-{index}")), &claimed.session).await.unwrap();
    }
    conformance::file_productions(&second, &scope, &[]).unwrap();
    second.close().await.unwrap();
}
