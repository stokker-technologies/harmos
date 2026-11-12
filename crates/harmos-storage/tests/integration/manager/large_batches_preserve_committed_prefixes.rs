/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Split Large Accepted Batches And Resume After A Partial Durable Commit.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{num::NonZeroUsize, sync::atomic::Ordering};

// Third Party Imports

// Local Imports
use super::fixture::{Fixture, config, record, schema};
use harmos_storage::{Identifier, StreamInput};

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

#[tokio::test(start_paused = true)]
async fn large_batches_preserve_committed_prefixes() {
    // Admit One Batch Larger Than The Provider Write Target.
    let mut settings = config();
    settings.request.max_pending_write_bytes = NonZeroUsize::new(4 * 1024 * 1024).unwrap();
    let target = settings.flush_batch_bytes();
    let ceiling = settings.max_batch_bytes().get();
    let fixture = Fixture::new(settings).await;
    let key = schema();
    let item = Identifier::new("bounded").unwrap();
    let payload = vec![90; target / 10];
    let records = (0..10).map(|_| record(&key, &item, &payload)).collect();
    let accepted = fixture.storage.append_records(records).await.unwrap();
    fixture.data.control.write_failure_at.store(5, Ordering::SeqCst);

    // Keep The First Durable Piece When The Next Piece Fails.
    assert!(fixture.storage.flush().await.is_err());
    let prefix = fixture.storage.get_saved_position();
    assert!(prefix.sequence > 0 && prefix.sequence < accepted.sequence);
    fixture.storage.flush().await.unwrap();
    assert_eq!(fixture.storage.get_saved_position(), accepted);
    assert_eq!(fixture.data.records.lock().unwrap().len(), 10);
    let writes = fixture.data.control.writes.lock().unwrap().clone();
    assert!(writes[prefix.sequence as usize..].iter().all(|sequence| *sequence > prefix.sequence));

    // Split Samples Using The Same Encoded Byte Bound.
    let samples = (1..=10)
        .map(|sequence| StreamInput {
            record: record(&key, &item, &payload),
            stream: item.clone(),
            sequence,
        })
        .collect();
    let accepted = fixture.storage.append_samples(samples).await.unwrap();
    fixture.storage.flush().await.unwrap();
    assert_eq!(fixture.storage.get_saved_position(), accepted);
    assert_eq!(fixture.data.rows.lock().unwrap().len(), 10);
    let commits = fixture.data.control.committed_bytes.lock().unwrap().clone();
    assert!(commits.len() > 2);
    assert!(commits.iter().all(|bytes| *bytes <= target && *bytes <= ceiling));
    fixture.finish().await;
}
