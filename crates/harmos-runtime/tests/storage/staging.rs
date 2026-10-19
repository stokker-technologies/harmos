/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "staging/mutations.rs"]
mod mutations;
#[path = "staging/production.rs"]
mod production;
#[path = "staging/recovery.rs"]
mod recovery;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::atomic::Ordering;
use std::time::Duration;
// Third Party Imports
// Local Imports
use super::{Fixture, config};
use harmos_runtime::Identifier;
use harmos_runtime::storage::FileMetadata;

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

#[tokio::test]
async fn resume_continues_offset() {
    let f = Fixture::new(config()).await;
    let mut first = f.storage.produce_file(metadata("resume")).await.unwrap();
    first.write_all(b"first").await.unwrap();
    let token = first.session().unwrap().clone();
    let mut resumed = f.storage.resume_file(&token).await.unwrap();
    assert_eq!(resumed.length(), 5);
    assert_eq!(resumed.session().unwrap().generation, token.generation + 1);
    assert!(first.write_all(b"stale").await.is_err());
    assert!(first.finish().await.is_err());
    assert!(first.abort().await.is_err());
    drop(first);
    resumed.write_all(b"second").await.unwrap();
    assert_eq!(resumed.length(), 11);
    let token = resumed.session().unwrap().clone();
    drop(resumed);
    assert!(f.data.abandoned_files.lock().unwrap().is_empty());
    let mut resumed = f.storage.resume_file(&token).await.unwrap();
    assert_eq!(resumed.length(), 11);
    let result = resumed.finish().await.unwrap();
    assert_eq!(result.length, 11);
    assert_eq!(f.data.blobs.lock().unwrap()[result.metadata.id.as_str()].1, b"firstsecond");
    assert!(f.storage.resume_file(&token).await.is_err());
    f.finish().await;
}

#[tokio::test]
async fn resume_reconciles_write() {
    let f = Fixture::new(config()).await;
    let mut file = f.storage.produce_file(metadata("cancelled")).await.unwrap();
    let token = file.session().unwrap().clone();
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    {
        let write = file.write_all(b"retained");
        tokio::pin!(write);
        tokio::select! {
            biased;
            _ = &mut write => panic!("expected blocked physical write"),
            _ = f.data.control.chunk_entered.notified() => {}
        }
    }
    assert!(file.write_all(b"ambiguous").await.is_err());
    drop(file);
    let mut resumed = {
        let resume = f.storage.resume_file(&token);
        tokio::pin!(resume);
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(resume.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        f.data.control.block_chunk.store(false, Ordering::SeqCst);
        f.data.control.chunk_release.notify_one();
        resume.await.unwrap()
    };
    assert_eq!(resumed.length(), 8);
    resumed.write_all(b" bytes").await.unwrap();
    let reference = resumed.finish().await.unwrap();
    assert_eq!(f.data.blobs.lock().unwrap()[reference.metadata.id.as_str()].1, b"retained bytes");
    f.finish().await;
}

#[tokio::test(start_paused = true)]
async fn idle_session_expires() {
    let f = Fixture::new(config()).await;
    let mut file = f.storage.produce_file(metadata("expired")).await.unwrap();
    file.write_all(b"partial").await.unwrap();
    let token = file.session().unwrap().clone();
    tokio::time::advance(Duration::from_secs(61)).await;

    // A Connected Producer Never Expires While Idle.
    file.write_all(b" after idle").await.unwrap();
    assert_eq!(file.session().unwrap(), &token);
    drop(file);
    tokio::time::advance(Duration::from_secs(59)).await;
    assert!(f.data.abandoned_files.lock().unwrap().is_empty());
    tokio::time::advance(Duration::from_secs(1)).await;
    f.data.control.abandoned.notified().await;
    assert!(f.data.abandoned_files.lock().unwrap().contains("expired"));
    assert_eq!(f.data.staged.lock().unwrap()["expired"].1, b"partial after idle");
    assert!(f.storage.resume_file(&token).await.is_err());
    assert_eq!(f.storage.cursor_accepted().sequence, 0);
    let data = f.data.clone();
    f.finish().await;
    assert_eq!(data.staged.lock().unwrap()["expired"].1, b"partial after idle");
}

#[tokio::test(start_paused = true)]
async fn cancelled_resume_expires() {
    let f = Fixture::new(config()).await;
    let unadopted = f.storage.produce_file(metadata("unadopted")).await.unwrap();
    let mut unadopted_token = unadopted.session().unwrap().clone();
    f.data.control.block_fence.store(true, Ordering::SeqCst);
    {
        let resume = f.storage.resume_file(&unadopted_token);
        tokio::pin!(resume);
        tokio::select! {
            biased;
            _ = &mut resume => panic!("expected blocked resume"),
            _ = f.data.control.fence_entered.notified() => {}
        }
    }
    f.data.control.block_fence.store(false, Ordering::SeqCst);
    f.data.control.fence_release.notify_one();

    // The Fence Completes Without A Resumer; Its Generation Expires Unadopted.
    f.data.control.fenced.notified().await;
    assert_eq!(f.data.file_generations.lock().unwrap().get("unadopted"), Some(&2));
    unadopted_token.generation = 2;
    tokio::time::advance(Duration::from_secs(61)).await;
    assert!(f.storage.resume_file(&unadopted_token).await.is_err());
    drop(unadopted);
    f.finish().await;
}

#[tokio::test]
async fn abandon_retains_bytes() {
    let f = Fixture::new(config()).await;
    let mut file = f.storage.produce_file(metadata("abandoned")).await.unwrap();
    file.write_all(b"partial").await.unwrap();
    file.abort().await.unwrap();
    assert_eq!(f.data.staged.lock().unwrap()["abandoned"].1, b"partial");
    assert!(f.storage.produce_file(metadata("abandoned")).await.is_err());
    assert_eq!(f.data.staged.lock().unwrap()["abandoned"].1, b"partial");
    f.finish().await;
}

#[tokio::test]
async fn follow_tails_production() {
    use harmos_runtime::storage::{FilePhase, StagedFile};
    use std::num::NonZeroUsize;
    let f = Fixture::new(config()).await;
    let mut producer = f.storage.produce_file(metadata("growing")).await.unwrap();
    producer.write_all(b"first").await.unwrap();
    let token = producer.session().unwrap();
    let identity = StagedFile { history: token.history.clone(), id: token.id.clone() };
    let mut reader = f.storage.follow_file(&identity, 0).await.unwrap();
    let max = NonZeroUsize::new(3).unwrap();
    assert_eq!(reader.next_batch(max).await.unwrap().unwrap().bytes, b"fir");
    assert_eq!(reader.next_batch(max).await.unwrap().unwrap().bytes, b"st");
    assert_eq!(reader.offset(), 5);
    {
        let next = reader.next_batch(max);
        tokio::pin!(next);
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(next.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        producer.write_all(b"second").await.unwrap();
        assert_eq!(next.await.unwrap().unwrap().bytes, b"sec");
    }
    drop(reader);
    let mut resumed = f.storage.follow_file(&identity, 8).await.unwrap();
    assert_eq!(resumed.next_batch(max).await.unwrap().unwrap().bytes, b"ond");
    producer.finish().await.unwrap();
    assert!(resumed.next_batch(max).await.unwrap().is_none());
    assert_eq!(resumed.progress().phase, FilePhase::Finished);
    assert_eq!(resumed.offset(), 11);
    assert!(f.storage.follow_file(&identity, 12).await.is_err());
    drop(resumed);
    let mut replay = f.storage.follow_file(&identity, 0).await.unwrap();
    let mut all = Vec::new();
    while let Some(chunk) = replay.next_batch(max).await.unwrap() {
        all.extend_from_slice(&chunk.bytes);
    }
    assert_eq!(all, b"firstsecond");
    drop(replay);
    f.finish().await;
}

#[tokio::test]
async fn follow_ends_abandoned() {
    use harmos_runtime::storage::{FilePhase, StagedFile};
    use std::num::NonZeroUsize;
    let f = Fixture::new(config()).await;
    let mut producer = f.storage.produce_file(metadata("partial-reader")).await.unwrap();
    producer.write_all(b"partial").await.unwrap();
    let token = producer.session().unwrap();
    let identity = StagedFile { history: token.history.clone(), id: token.id.clone() };
    let mut reader = f.storage.follow_file(&identity, 0).await.unwrap();
    producer.abort().await.unwrap();
    let max = NonZeroUsize::new(100).unwrap();
    assert_eq!(reader.next_batch(max).await.unwrap().unwrap().bytes, b"partial");
    assert!(reader.next_batch(max).await.unwrap().is_none());
    assert_eq!(reader.progress().phase, FilePhase::Abandoned);
    drop(reader);
    let mut replay = f.storage.follow_file(&identity, 3).await.unwrap();
    assert_eq!(replay.next_batch(max).await.unwrap().unwrap().bytes, b"tial");
    assert!(replay.next_batch(max).await.unwrap().is_none());
    assert_eq!(replay.progress().phase, FilePhase::Abandoned);
    drop(replay);
    f.finish().await;
}

#[tokio::test]
async fn cancelled_follow_offset() {
    use harmos_runtime::storage::StagedFile;
    use std::num::NonZeroUsize;
    let f = Fixture::new(config()).await;
    let mut producer = f.storage.produce_file(metadata("cancel-reader")).await.unwrap();
    producer.write_all(b"first").await.unwrap();
    let token = producer.session().unwrap();
    let identity = StagedFile { history: token.history.clone(), id: token.id.clone() };
    let mut reader = f.storage.follow_file(&identity, 0).await.unwrap();
    f.data.control.block_read.store(true, Ordering::SeqCst);
    {
        let read = reader.next_batch(NonZeroUsize::new(3).unwrap());
        tokio::pin!(read);
        tokio::select! {
            biased;
            _ = &mut read => panic!("expected blocked physical read"),
            _ = f.data.control.read_entered.notified() => {}
        }
    }
    assert_eq!(reader.offset(), 0);
    drop(reader);
    f.data.control.block_read.store(false, Ordering::SeqCst);
    producer.write_all(b"second").await.unwrap();
    assert_eq!(producer.finish().await.unwrap().length, 11);
    f.finish().await;
}

#[tokio::test]
async fn private_files_hidden() {
    use harmos_runtime::storage::StagedFile;
    let f = Fixture::new(config()).await;
    let mut private = f.storage.create_file(metadata("private")).await.unwrap();
    private.write_all(b"private bytes").await.unwrap();
    let identity = StagedFile { history: f.storage.cursor_accepted().history, id: Identifier::new("private").unwrap() };
    assert!(f.storage.follow_file(&identity, 0).await.is_err());
    assert!(f.storage.file_progress(&identity).await.is_err());
    private.abort().await.unwrap();
    f.finish().await;
}
