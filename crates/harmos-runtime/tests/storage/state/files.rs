/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports
use std::{fs::OpenOptions, io::Write, num::NonZeroUsize};
// Third Party Imports
// Local Imports
use harmos_runtime::storage::state::{RecoveryEntry, RecoverySnapshot, StateBackend, StorageStateSettings, file::FileBackend};
use harmos_runtime::storage::{ReadLimits, RecoveryPosition, StorageCursor};

/* === Private Functions === */

fn unsupported_header(history: &harmos_runtime::storage::StoreHistory) -> Vec<u8> {
    use sha2::Digest;

    let value = serde_json::json!({
        "version": 2,
        "after": RecoveryPosition {
            history: history.clone(),
            sequence: 0,
        },
    });
    let json = serde_json::to_vec(&value).unwrap();
    let digest = sha2::Sha256::digest(&json);
    let hex = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    [b"{\"value\":".as_slice(), &json, b",\"sha256\":\"", hex.as_bytes(), b"\"}\n"].concat()
}

/* === Tests === */

#[tokio::test]
async fn checkpoint_survives_reopen() {
    let root = tempfile::tempdir().unwrap();
    let history = crate::storage::history();
    let settings = crate::storage::state_settings();
    let backend = FileBackend::new(root.path().to_owned(), history.clone(), settings);
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
    backend.delete_recovery_session().await.unwrap();

    let mut file = OpenOptions::new().append(true).open(root.path().join("state.jsonl")).unwrap();
    file.write_all(b"{\"value\":").unwrap();
    file.sync_all().unwrap();
    drop(file);

    let reopened = FileBackend::new(root.path().to_owned(), history, settings);
    assert_eq!(reopened.create_recovery_session().await.unwrap().sequence, 3);
    assert_eq!(reopened.get_recovery_snapshot(std::num::NonZeroUsize::new(1024 * 1024).unwrap()).await.unwrap(), Some(snapshot.clone()));
    let through = RecoveryPosition {
        history: snapshot.through.history.clone(),
        sequence: 3,
    };
    let page = reopened
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
    reopened.delete_recovery_session().await.unwrap();
}

#[tokio::test]
async fn corruption_refused_preserved() {
    let root = tempfile::tempdir().unwrap();
    let history = crate::storage::history();
    let settings = crate::storage::state_settings();
    let backend = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    backend.create_recovery_session().await.unwrap();
    let position = RecoveryPosition { history: history.clone(), sequence: 1 };
    let entry = RecoveryEntry {
        position: position.clone(),
        required_data: StorageCursor { history: history.clone(), sequence: 0 },
        payload: b"saved state".to_vec(),
    };
    backend.append_recovery_entries(harmos_storage::StorageBatch::new(vec![entry.clone()])).await.unwrap();
    backend.flush(&position).await.unwrap();
    backend.delete_recovery_session().await.unwrap();

    // A Complete Entry With A Damaged Payload Must Not Be Treated As An Interrupted Append.
    let path = root.path().join("state.jsonl");
    let mut corrupt = std::fs::read(&path).unwrap();
    let encoded = b"c2F2ZWQgc3RhdGU";
    let at = corrupt.windows(encoded.len()).position(|window| window == encoded).unwrap();
    corrupt[at] = b'd';
    std::fs::write(&path, &corrupt).unwrap();
    let reopened = FileBackend::new(root.path().to_owned(), history, settings);
    assert!(reopened.create_recovery_session().await.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), corrupt);
    assert!(reopened.create_recovery_session().await.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), corrupt);
}

#[tokio::test]
async fn state_root_exclusive() {
    let root = tempfile::tempdir().unwrap();
    let history = crate::storage::history();
    let settings = crate::storage::state_settings();
    let first = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    let second = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    first.create_recovery_session().await.unwrap();
    assert!(second.create_recovery_session().await.is_err());
    let position = RecoveryPosition { history: history.clone(), sequence: 1 };
    let entry = RecoveryEntry {
        position: position.clone(),
        required_data: StorageCursor { history: history.clone(), sequence: 0 },
        payload: b"exclusive writer".to_vec(),
    };
    first.append_recovery_entries(harmos_storage::StorageBatch::new(vec![entry.clone()])).await.unwrap();
    first.flush(&position).await.unwrap();
    first.delete_recovery_session().await.unwrap();

    // A Refused Owner Can Acquire The Root After The Original Owner Closes.
    assert_eq!(second.create_recovery_session().await.unwrap(), position);
    let after = RecoveryPosition { history, sequence: 0 };
    let page = second
        .list_recovery_entries(
            &after,
            &position,
            ReadLimits {
                rows: std::num::NonZeroUsize::new(64).unwrap(),
                bytes: std::num::NonZeroUsize::new(64 * 1024).unwrap(),
            },
        )
        .await
        .unwrap();
    assert_eq!(page.entries, vec![entry]);
    assert_eq!(page.next, None);
    assert!(first.create_recovery_session().await.is_err());
    second.delete_recovery_session().await.unwrap();
    assert_eq!(first.create_recovery_session().await.unwrap(), position);
    first.delete_recovery_session().await.unwrap();
}

#[tokio::test]
async fn limits_bound_records() {
    let history = crate::storage::history();
    let at = |sequence| RecoveryPosition { history: history.clone(), sequence };
    let entry = |sequence, bytes: usize| RecoveryEntry {
        position: at(sequence),
        required_data: StorageCursor { history: history.clone(), sequence: 0 },
        payload: vec![7; bytes],
    };
    let snapshot = |bytes: usize| RecoverySnapshot {
        through: at(1),
        required_data: StorageCursor { history: history.clone(), sequence: 0 },
        payload: harmos_storage::state::SnapshotPayload::from_bytes(
            &vec![9; bytes],
            std::num::NonZeroUsize::new(65536).unwrap(),
            harmos_domain::state::Position::ORIGIN,
        )
        .unwrap(),
    };

    // Measure The Encoded Records Written For A Payload Of 3k Bytes.
    let measured = tempfile::tempdir().unwrap();
    let journal = measured.path().join("state.jsonl");
    let backend = FileBackend::new(measured.path().to_owned(), history.clone(), crate::storage::state_settings());
    backend.create_recovery_session().await.unwrap();
    let header = std::fs::metadata(&journal).unwrap().len();
    backend.append_recovery_entries(harmos_storage::StorageBatch::new(vec![entry(1, 3000)])).await.unwrap();
    // Measure Before The Checkpoint Compacts The Journal Back To Its Header.
    let entry_bytes = std::fs::metadata(&journal).unwrap().len() - header;
    backend.set_recovery_snapshot(harmos_storage::StorageBatch::new(vec![snapshot(3000)])).await.unwrap();
    backend.delete_recovery_session().await.unwrap();
    let checkpoint = std::fs::read_to_string(measured.path().join("state.json")).unwrap();
    let checkpoint_bytes = checkpoint.lines().nth(1).unwrap().len() + 1;
    // Base64 Text Plus Envelope, Far Below The Former Decimal Number Arrays.
    assert!(entry_bytes < 4500 && checkpoint_bytes < 4500);

    // A Payload At The Limit Is Accepted; One Byte More Is Refused And Appends Nothing.
    let root = tempfile::tempdir().unwrap();
    let settings = StorageStateSettings {
        max_entry_bytes: NonZeroUsize::new(entry_bytes as usize).unwrap(),
        max_checkpoint_bytes: NonZeroUsize::new(checkpoint_bytes as usize).unwrap(),
    };
    let backend = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    backend.create_recovery_session().await.unwrap();
    backend.append_recovery_entries(harmos_storage::StorageBatch::new(vec![entry(1, 3000)])).await.unwrap();
    assert!(backend.append_recovery_entries(harmos_storage::StorageBatch::new(vec![entry(2, 3001)])).await.is_err());
    assert!(backend.set_recovery_snapshot(harmos_storage::StorageBatch::new(vec![snapshot(3001)])).await.is_err());
    backend.set_recovery_snapshot(harmos_storage::StorageBatch::new(vec![snapshot(3000)])).await.unwrap();
    backend.delete_recovery_session().await.unwrap();

    let reopened = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    assert_eq!(reopened.create_recovery_session().await.unwrap(), at(1));
    assert_eq!(reopened.get_recovery_snapshot(NonZeroUsize::new(checkpoint_bytes as usize).unwrap()).await.unwrap(), Some(snapshot(3000)));
    // A Caller Bound Covers The Snapshot Value, Not The Integrity Envelope.
    let value_bytes = checkpoint.lines().nth(1).unwrap().len() + 1;
    assert_eq!(reopened.get_recovery_snapshot(NonZeroUsize::new(value_bytes).unwrap()).await.unwrap(), Some(snapshot(3000)));
    reopened.delete_recovery_session().await.unwrap();
}

#[tokio::test]
async fn unsupported_format_refused() {
    let root = tempfile::tempdir().unwrap();
    let history = crate::storage::history();
    std::fs::write(root.path().join("state.jsonl"), unsupported_header(&history)).unwrap();

    let backend = FileBackend::new(root.path().to_owned(), history, crate::storage::state_settings());
    let error = backend.create_recovery_session().await.unwrap_err();
    assert!(error.to_string().contains("fresh state root"), "{error}");
}
