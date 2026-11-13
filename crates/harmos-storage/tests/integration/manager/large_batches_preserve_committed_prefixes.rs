/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Preserve Bounded Ordered Commits Through Failure Of A Later Admission.

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
    // Keep Large Record And Sample Admissions As Distinct Ordered Commit Units.
    let mut settings = config();
    settings.request.max_write_batch_bytes = NonZeroUsize::new(4 * 1024 * 1024).unwrap();
    let trigger = settings.flush_trigger_bytes();
    let ceiling = settings.max_batch_bytes().get();
    let fixture = Fixture::new(settings).await;
    let key = schema();
    let item = Identifier::new("bounded").unwrap();
    let payload = vec![90; trigger / 10];
    let records = (0..4).map(|_| record(&key, &item, &payload)).collect();
    let prefix = fixture.storage.append_records(records).await.unwrap();
    fixture.data.control.write_failure_at.store(5, Ordering::SeqCst);
    let samples = (1..=6)
        .map(|sequence| StreamInput {
            record: record(&key, &item, &payload),
            stream: item.clone(),
            sequence,
        })
        .collect();
    let accepted = fixture.storage.append_samples(samples).await.unwrap();

    // A Failed Later Commit Cannot Erase Or Rewrite An Earlier Durable Prefix.
    assert!(fixture.storage.flush().await.is_err());
    assert_eq!(fixture.storage.get_saved_position(), prefix);
    fixture.storage.flush().await.unwrap();
    assert_eq!(fixture.storage.get_saved_position(), accepted);
    assert_eq!(fixture.data.records.lock().unwrap().len(), 4);
    assert_eq!(fixture.data.rows.lock().unwrap().len(), 6);
    let writes = fixture.data.control.writes.lock().unwrap().clone();
    assert!(writes[prefix.sequence as usize..].iter().all(|sequence| *sequence > prefix.sequence));
    let commits = fixture.data.control.committed_bytes.lock().unwrap().clone();
    assert_eq!(commits.len(), 2);
    assert!(commits.iter().all(|bytes| *bytes > trigger && *bytes <= ceiling));
    fixture.finish().await;
}
