/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use harmos_runtime::storage::state::{RecoveryEntry, RecoverySnapshot, StateBackend};
use harmos_runtime::storage::{ReadLimits, RecoveryPosition, StorageCursor};

/* === Tests === */

#[tokio::test]
async fn checkpoint_survives_reopen() {
    let history = crate::storage::history();
    let backend = harmos_storage::memory::RecoveryBackend::new(history.clone());
    let at = |sequence| RecoveryPosition { history: history.clone(), sequence };
    let required_data = StorageCursor { history: history.clone(), sequence: 0 };
    assert_eq!(backend.create_recovery_session().await.unwrap(), at(0));
    let entries = (1..=3)
        .map(|sequence| RecoveryEntry {
            position: at(sequence),
            required_data: required_data.clone(),
            payload: vec![sequence as u8],
        })
        .collect::<Vec<_>>();
    backend.append_recovery_entries(harmos_storage::StorageBatch::new(entries.clone())).await.unwrap();
    assert_eq!(backend.flush(&at(3)).await.unwrap(), at(3));
    backend.append_recovery_entries(harmos_storage::StorageBatch::new(entries.clone())).await.unwrap();
    let snapshot = RecoverySnapshot {
        through: at(2),
        required_data,
        payload: harmos_storage::state::SnapshotPayload::from_bytes(
            b"checkpoint",
            std::num::NonZeroUsize::new(65536).unwrap(),
            harmos_domain::state::Position::ORIGIN,
        )
        .unwrap(),
    };
    backend.set_recovery_snapshot(harmos_storage::StorageBatch::new(vec![snapshot.clone()])).await.unwrap();
    backend.flush(&at(3)).await.unwrap();
    backend.delete_recovery_session().await.unwrap();

    assert_eq!(backend.create_recovery_session().await.unwrap().sequence, 3);
    assert_eq!(backend.get_recovery_snapshot(std::num::NonZeroUsize::new(1024 * 1024).unwrap()).await.unwrap(), Some(snapshot.clone()));
    let through = RecoveryPosition {
        history: snapshot.through.history.clone(),
        sequence: 3,
    };
    let page = backend
        .list_recovery_entries(
            &snapshot.through,
            &through,
            ReadLimits {
                rows: std::num::NonZeroUsize::new(64).unwrap(),
                bytes: std::num::NonZeroUsize::new(64 * 1024).unwrap(),
            },
        )
        .await
        .unwrap();
    assert_eq!(page.entries, entries[2..]);
    assert_eq!(page.next, None);
    backend.delete_recovery_session().await.unwrap();
}
