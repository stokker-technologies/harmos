/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise Resumable Native Files And Durable Mutation Replay With The Storage Fixture.

/* === Imports === */

// Standard Library Imports
use std::{num::NonZeroUsize, sync::Arc};

// Third Party Imports
use sha2::{Digest, Sha256};
use tokio::{sync::Mutex, task::JoinHandle};

// Local Imports
use super::manager::backend::{Data, Recovery, history};
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

pub(super) async fn storage(primary: &Arc<Data>) -> StorageManager {
    let pending = NonZeroUsize::new(1024 * 1024).unwrap();
    let batch = NonZeroUsize::new(4096).unwrap();
    let settings = StorageSettings {
        transfer: Default::default(),
        policy: Default::default(),
        filesystem: harmos_storage::StorageFilesystemSettings {
            poll_interval: std::time::Duration::from_millis(100),
            max_entries: std::num::NonZeroUsize::new(100_000).unwrap(),
            event_capacity: std::num::NonZeroUsize::new(1024).unwrap(),
        },

        request: StorageRequestSettings {
            max_file_productions: std::num::NonZeroUsize::new(256).unwrap(),
            max_mutation_receipts: std::num::NonZeroUsize::new(1024).unwrap(),
            max_pending_queries: NonZeroUsize::new(8).unwrap(),
            max_active_queries: NonZeroUsize::new(2).unwrap(),
            max_active_file_operations: NonZeroUsize::new(1).unwrap(),
            max_pending_write_batches: NonZeroUsize::new(16).unwrap(),
            max_write_batch_bytes: pending,
        },
        response: StorageResponseSettings {
            batch_rows: NonZeroUsize::new(32).unwrap(),
            batch_bytes: batch,
        },
    };

    let backends = StorageBackends {
        primary: primary.clone(),
        state: Arc::new(Recovery::new(history())),
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
    let data = Arc::new(Data::new(history()));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("outside-backend.bin");
    let expected: Vec<_> = (0..20_000).map(|value| (value % 251) as u8).collect();
    std::fs::write(&path, &expected).unwrap();
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
    std::fs::write(&path, b"changed after capture").unwrap();
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
    let data = Arc::new(Data::new(history()));
    use std::future::{Future, poll_fn};
    use std::task::Poll;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mutable.bin");
    let storage = storage(&data).await;
    for (index, consistency, replacement) in [(0, CaptureConsistency::Stable, vec![9; 8193]), (1, CaptureConsistency::Bounded, vec![9; 1])]
    {
        std::fs::write(&path, vec![7; 8192]).unwrap();
        let request = FileCapture {
            source: FileSource::Local(path.clone()),
            range: ByteRange { offset: 0, limit: None },
            metadata: metadata(&format!("mutation-{index}")),
            consistency,
        };
        let mut copying = Box::pin(storage.capture_file(request));
        // On This Current-Thread Executor, Source Open Precedes Target Admission;
        // Mutate Immediately After Admission Before The Worker Can Run The Target.
        poll_fn(|cx| match copying.as_mut().poll(cx) {
            Poll::Pending if storage.status().pending_batches > 0 => Poll::Ready(()),
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => panic!("capture completed before target admission: {result:?}"),
        })
        .await;
        std::fs::write(&path, replacement).unwrap();
        assert!(copying.await.is_err());
        storage.flush().await.unwrap();
    }
    storage.close().await.unwrap();
}

#[tokio::test]
async fn local_follow_changes() {
    let data = Arc::new(Data::new(history()));
    use std::io::Write;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("growing.bin");
    std::fs::write(&path, b"first").unwrap();
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
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"second").unwrap();
    assert_eq!(follower.next().await.unwrap().unwrap().data().unwrap(), b"second");
    std::fs::write(&path, b"x").unwrap();
    assert_eq!(follower.next().await.unwrap().unwrap().event(), &FileEvent::Truncated { length: 1 });
    assert_eq!(follower.next().await.unwrap().unwrap().data().unwrap(), b"x");
    std::fs::rename(&path, path.with_extension("old")).unwrap();
    std::fs::write(&path, b"replacement").unwrap();
    assert_eq!(follower.next().await.unwrap().unwrap().event(), &FileEvent::Replaced);
    assert_eq!(follower.next().await.unwrap().unwrap().data().unwrap(), b"replacement");
    let mut tail = storage
        .follow_file(FileFollow {
            source: source.clone(),
            start: FollowStart::End,
        })
        .await
        .unwrap();
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"tail").unwrap();
    assert_eq!(tail.next().await.unwrap().unwrap().data().unwrap(), b"tail");
    let mut at = storage.follow_file(FileFollow { source, start: FollowStart::At(2) }).await.unwrap();
    assert_eq!(at.next().await.unwrap().unwrap().data().unwrap(), b"placementtail");
    drop((follower, tail, at));
    storage.close().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn native_path_rules() {
    let data = Arc::new(Data::new(history()));
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    let directory = tempfile::tempdir().unwrap();
    let name = std::ffi::OsString::from_vec(vec![b'n', 0xff]);
    let path = directory.path().join(name);
    std::fs::write(&path, b"native bytes").unwrap();
    let storage = storage(&data).await;
    let request = FileRead {
        source: FileSource::Local(path.clone()),
        range: ByteRange { offset: 0, limit: None },
    };
    assert_eq!(storage.hash_file(request.clone()).await.unwrap().bytes_read, 12);
    let link = directory.path().join("linked-file");
    symlink(&path, &link).unwrap();
    assert!(
        storage
            .hash_file(FileRead {
                source: FileSource::Local(link),
                ..request.clone()
            })
            .await
            .is_err()
    );
    let alias = directory.path().join("linked-directory");
    symlink(directory.path(), &alias).unwrap();
    let through = alias.join(path.file_name().unwrap());
    assert!(
        storage
            .hash_file(FileRead {
                source: FileSource::Local(through),
                ..request
            })
            .await
            .is_err()
    );
    storage.close().await.unwrap();
}

#[tokio::test]
async fn sessions_survive_reopen() {
    let data = Arc::new(Data::new(history()));
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
        source: harmos_domain::model::storage::files::FileSource::Stored(reference.clone()),
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
    let data = Arc::new(Data::new(history()));
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
