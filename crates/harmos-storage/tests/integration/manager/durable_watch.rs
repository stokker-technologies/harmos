/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use super::fixture::{Fixture, config, write};
use harmos_storage::{Error, Identifier, StoredRecord, query::RecordSelection};
use std::{num::NonZeroUsize, sync::atomic::Ordering};

/* === Tests === */

#[tokio::test]
async fn durable_watch_waits_for_commit_advances_filters_and_reports_retention_gaps() {
    let mut settings = config();
    settings.request.max_pending_write_batches = NonZeroUsize::new(2).unwrap();
    let fixture = Fixture::new(settings).await;
    let first = write(&fixture.storage, "initial").await;
    fixture.storage.flush().await.unwrap();
    let selection = RecordSelection {
        schema: super::fixture::schema(),
        scope: None,
        item: Some(Identifier::new("selected").unwrap()),
        source_start: None,
        source_end: None,
    };
    let mut watcher = fixture.storage.follow::<StoredRecord>(Some(&first), selection.clone()).await.unwrap();
    fixture.data.control.block_write.store(true, Ordering::Release);
    let unmatched = write(&fixture.storage, "unmatched").await;
    let mut flush = Box::pin(fixture.storage.flush_until(&unmatched));
    assert!(super::events::until(flush.as_mut(), fixture.data.control.write_entered.notified()).await.is_ok());
    let mut pending = Box::pin(watcher.next_durable());
    assert!(
        std::future::poll_fn(|context| std::task::Poll::Ready(std::future::Future::poll(pending.as_mut(), context).is_pending())).await
    );
    fixture.data.control.block_write.store(false, Ordering::Release);
    fixture.data.control.write_release.notify_one();
    flush.await.unwrap();
    let (page, through) = pending.await.unwrap();
    assert!(page.entries().is_empty());
    assert_eq!(through, unmatched);
    let selected = write(&fixture.storage, "selected").await;
    fixture.storage.flush().await.unwrap();
    let (page, through) = watcher.next_durable().await.unwrap();
    assert_eq!(through, selected);
    assert_eq!(page.entries().len(), 1);
    assert_eq!(page.entries()[0].position, selected);
    drop(watcher);

    let mut slow = fixture.storage.follow::<StoredRecord>(Some(&selected), selection).await.unwrap();
    for item in ["one", "two", "three"] {
        write(&fixture.storage, item).await;
        fixture.storage.flush().await.unwrap();
    }
    assert!(matches!(slow.next_durable().await, Err(Error::Gap { requested, available }) if requested < available));
    drop(slow);
    fixture.finish().await;
}
