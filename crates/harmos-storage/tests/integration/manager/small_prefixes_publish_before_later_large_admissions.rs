/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use super::{
    events,
    fixture::{Fixture, config, record, schema},
};
use harmos_storage::{Identifier, StreamInput};
use std::{num::NonZeroUsize, sync::atomic::Ordering};

/* === Tests === */

#[tokio::test(start_paused = true)]
async fn small_prefixes_publish_before_later_large_admissions() {
    let mut settings = config();
    settings.request.max_pending_write_bytes = NonZeroUsize::new(4 * 1024 * 1024).unwrap();
    let trigger = settings.flush_trigger_bytes();
    let fixture = Fixture::new(settings).await;
    let key = schema();
    let item = Identifier::new("ordered").unwrap();
    let input = |sequence, payload: &[u8]| StreamInput {
        record: record(&key, &item, payload),
        stream: item.clone(),
        sequence,
    };
    fixture.data.control.block_write.store(true, Ordering::SeqCst);
    let first = fixture.storage.append_samples(vec![input(1, &[1; 32])]).await.unwrap();
    fixture.storage.append_samples((2..=7).map(|sequence| input(sequence, &vec![90; trigger / 10])).collect()).await.unwrap();
    let last = fixture.storage.append_samples(vec![input(8, &[2; 32])]).await.unwrap();
    let mut flush = Box::pin(fixture.storage.flush());
    assert!(events::until(flush.as_mut(), fixture.data.control.write_entered.notified()).await.is_ok());
    fixture.data.control.write_release.notify_one();
    assert!(events::until(flush.as_mut(), fixture.data.control.write_entered.notified()).await.is_ok());
    assert_eq!(fixture.storage.get_saved_position(), first);
    assert_eq!(fixture.data.rows.lock().unwrap().len(), 1);
    fixture.data.control.block_write.store(false, Ordering::SeqCst);
    fixture.data.control.write_release.notify_one();
    assert_eq!(flush.await.unwrap(), last);
    assert_eq!(*fixture.data.control.write_sizes.lock().unwrap(), vec![1, 6, 1]);
    assert_eq!(fixture.data.rows.lock().unwrap().len(), 8);
    fixture.finish().await;
}
