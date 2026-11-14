/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use std::num::NonZeroUsize;
use harmos_storage::{Identifier, StoreHistory, StorageCursor};
use harmos_storage::state::StateBackend;

/* === Tests === */

#[tokio::test]
async fn snapshot_requires_singleton() {
    use harmos_storage::{StorageBatch, state::RecoverySnapshot};
    let history = StoreHistory {
        store: Identifier::new("snapshot.test").unwrap(),
        epoch: Identifier::new("one").unwrap(),
    };
    let backend = harmos_storage::memory::RecoveryBackend::new(history.clone());
    let through = backend.create_recovery_session().await.unwrap();
    let snapshot = RecoverySnapshot {
        required_data: StorageCursor { history, sequence: 0 },
        through,
        payload: harmos_storage::state::SnapshotPayload::from_bytes(
            &[7],
            std::num::NonZeroUsize::new(65536).unwrap(),
            harmos_domain::state::Position::ORIGIN,
        )
        .unwrap(),
    };
    assert!(backend.set_recovery_snapshot(StorageBatch::new(Vec::new())).await.is_err());
    assert!(backend.set_recovery_snapshot(StorageBatch::new(vec![snapshot.clone(), snapshot.clone()])).await.is_err());
    assert!(backend.get_recovery_snapshot(NonZeroUsize::new(4096).unwrap()).await.unwrap().is_none());
    backend.set_recovery_snapshot(StorageBatch::new(vec![snapshot.clone()])).await.unwrap();
    backend.flush(&snapshot.through).await.unwrap();
    assert_eq!(backend.get_recovery_snapshot(NonZeroUsize::new(4096).unwrap()).await.unwrap(), Some(snapshot));
    backend.delete_recovery_session().await.unwrap();
}

