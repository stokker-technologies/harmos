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
use std::sync::atomic::Ordering;
// Third Party Imports

// Local Imports
use super::{Fixture, config, record, schema, write};
use harmos_storage::StorageCursor;
use harmos_storage::{Error, Identifier};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn foreign_cursor_refused() {
    let f = Fixture::new(config()).await;
    let first = write(&f.storage, "first").await;
    let mut foreign = first.clone();
    foreign.history.store = Identifier::new("other-store").unwrap();
    assert!(matches!(f.storage.flush_until(&foreign).await, Err(Error::Domain(harmos_domain::Error::DifferentHistory))));
    foreign = first.clone();
    foreign.history.epoch = Identifier::new("other-epoch").unwrap();
    assert!(matches!(f.storage.flush_until(&foreign).await, Err(Error::Domain(harmos_domain::Error::DifferentHistory))));
    let future = StorageCursor {
        sequence: first.sequence + 1,
        ..first.clone()
    };
    assert!(matches!(f.storage.flush_until(&future).await, Err(Error::InvalidInput { field: "storage cursor", .. })));
    assert!(f.data.control.writes.lock().unwrap().is_empty());
    assert_eq!(f.storage.cursor_accepted(), first);
    assert_eq!(f.storage.cursor_saved().sequence, 0);
    f.finish().await;
}

#[tokio::test]
async fn flush_preserves_batches() {
    let f = Fixture::new(config()).await;
    let first = write(&f.storage, "first").await;
    let second = write(&f.storage, "second").await;
    assert_eq!(f.storage.flush_until(&first).await.unwrap(), first);
    assert_eq!(f.storage.cursor_accepted(), second);
    assert_eq!(f.data.control.writes.lock().unwrap().as_slice(), &[1]);
    assert_eq!(f.storage.status().pending_batches, 1);
    assert_eq!(f.storage.flush_until(&first).await.unwrap(), first);
    assert_eq!(f.data.control.flushes.lock().unwrap().as_slice(), &[1]);
    assert_eq!(f.storage.flush().await.unwrap(), second);

    // An Interior Cursor Flushes Its Entire Atomic Batch, Keeping Later Work Queued.
    let key = schema();
    let item = Identifier::new("item").unwrap();
    let records = vec![record(&key, &item, b"one"); 3];
    let end = f.storage.write_records((records).to_vec()).await.unwrap();
    let later = write(&f.storage, "later").await;
    let middle = StorageCursor { sequence: 4, ..end.clone() };
    assert_eq!(f.storage.flush_until(&middle).await.unwrap(), end);
    assert_eq!(end.sequence, 5);
    assert_eq!(f.data.control.writes.lock().unwrap().as_slice(), &[1, 2, 3, 4, 5]);
    assert_eq!(f.storage.cursor_accepted(), later);
    assert_eq!(f.storage.status().pending_batches, 1);
    f.finish().await;
}

#[tokio::test]
async fn cancellation_retains_flush() {
    let f = Fixture::new(config()).await;
    let first = write(&f.storage, "first").await;
    f.data.control.block_flush.store(true, Ordering::SeqCst);
    {
        let flush = f.storage.flush_until(&first);
        tokio::pin!(flush);
        tokio::select! {
            biased;
            _ = &mut flush => panic!("backend flush is blocked"),
            _ = f.data.control.flush_entered.notified() => {}
        }
    }
    let later = write(&f.storage, "later").await;
    f.data.control.block_flush.store(false, Ordering::SeqCst);
    f.data.control.flush_release.notify_one();
    assert_eq!(f.storage.flush_until(&first).await.unwrap(), first);
    assert_eq!(f.storage.cursor_accepted(), later);
    assert_eq!(f.data.control.writes.lock().unwrap().as_slice(), &[1]);
    assert_eq!(f.storage.status().pending_batches, 1);
    f.finish().await;
}

#[tokio::test]
async fn receipt_releases_covered() {
    let f = Fixture::new(config()).await;
    let first = write(&f.storage, "first").await;
    let second = write(&f.storage, "second").await;
    f.data.control.block_write.store(true, Ordering::SeqCst);
    f.data.control.flush_advance.store(second.sequence as usize, Ordering::SeqCst);
    {
        let all = f.storage.flush();
        tokio::pin!(all);
        tokio::select! {
            biased;
            _ = &mut all => panic!("backend write is blocked"),
            _ = f.data.control.write_entered.notified() => {}
        }
    }
    {
        let earlier = f.storage.flush_until(&first);
        tokio::pin!(earlier);
        assert!(super::pending(earlier.as_mut()).await, "backend write is still blocked");
        f.data.control.block_write.store(false, Ordering::SeqCst);
        f.data.control.write_release.notify_one();
        assert_eq!(earlier.await.unwrap(), second);
    }
    assert_eq!(f.data.control.flushes.lock().unwrap().as_slice(), &[2]);
    assert_eq!(f.storage.status().pending_batches, 0);
    assert_eq!(f.storage.status().pending_bytes, 0);
    assert_eq!(f.storage.flush_until(&second).await.unwrap(), second);
    assert_eq!(f.data.control.flushes.lock().unwrap().as_slice(), &[2]);
    f.finish().await;
}

#[tokio::test]
async fn uncertain_batch_acknowledge() {
    let f = Fixture::new(config()).await;
    let first = write(&f.storage, "first").await;
    let second = write(&f.storage, "second").await;
    f.data.control.flush_failures.store(1, Ordering::SeqCst);
    assert!(f.storage.flush().await.is_err());
    assert_eq!(f.storage.cursor_saved().sequence, 0);
    assert_eq!(f.storage.status().pending_batches, 2);
    assert_eq!(f.data.saved.load(Ordering::SeqCst), 2);
    assert_eq!(f.storage.flush_until(&first).await.unwrap(), second);
    assert_eq!(*f.data.control.writes.lock().unwrap(), [1, 2, 1]);
    assert_eq!(f.storage.status().pending_bytes, 0);
    assert_eq!(f.data.records.lock().unwrap().len(), 2);
    f.finish().await;
}

#[tokio::test]
async fn receipt_cannot_cover() {
    let f = Fixture::new(config()).await;
    let first = write(&f.storage, "first").await;
    let second = write(&f.storage, "second").await;
    f.data.control.flush_advance.store(second.sequence as usize, Ordering::SeqCst);
    assert!(matches!(f.storage.flush_until(&first).await,
        Err(Error::Persistence { source }) if matches!(source.as_ref(),
            Error::InvalidInput { field: "data write receipt", .. })));
    assert_eq!(f.storage.cursor_saved().sequence, 0);
    assert_eq!(f.storage.status().pending_batches, 2);
    assert_eq!(*f.data.control.writes.lock().unwrap(), [1]);
    // Undo this deliberately forged backend receipt before retrying real data.
    f.data.control.flush_advance.store(0, Ordering::SeqCst);
    f.data.saved.store(0, Ordering::SeqCst);
    assert_eq!(f.storage.flush().await.unwrap(), second);
    f.finish().await;
}

/* === Tests === */

#[tokio::test(start_paused = true)]
async fn saved_wait_passive() {
    let f = Fixture::new(config()).await;
    let before = f.storage.cursor_saved();
    let first = write(&f.storage, "passive-first").await;
    let second = write(&f.storage, "passive-second").await;
    {
        let mut waiting = Box::pin(f.storage.wait_saved_after(&before));
        assert!(super::fixture::pending(waiting.as_mut()).await);
        assert_eq!(f.storage.cursor_saved(), before);
        assert_eq!(f.storage.cursor_accepted(), second);
        assert!(f.data.control.writes.lock().unwrap().is_empty());
        assert!(f.data.control.flushes.lock().unwrap().is_empty());
        drop(waiting);
    }
    assert_eq!(f.storage.status().pending_batches, 2);
    assert!(f.data.control.writes.lock().unwrap().is_empty());
    assert!(f.data.control.flushes.lock().unwrap().is_empty());
    // The Observer Does Not Drive This Explicit Persistence Request.
    assert_eq!(f.storage.flush_until(&second).await.unwrap(), second);
    assert_eq!(f.storage.wait_saved_after(&before).await.unwrap(), second);
    assert!(second.sequence > first.sequence);
    let saved = f.storage.cursor_saved();
    let waiting = f.storage.wait_saved_after(&saved);
    let third = write(&f.storage, "snapshot-race").await;
    f.storage.flush_until(&third).await.unwrap();
    assert_eq!(waiting.await.unwrap(), third);
    let closed = f.storage.clone();
    f.finish().await;
    assert!(matches!(closed.wait_saved_after(&third).await, Err(Error::Closing { .. })));
    assert_eq!(closed.wait_saved_after(&before).await.unwrap(), third);
}
