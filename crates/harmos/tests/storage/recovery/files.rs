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
use std::{fs::OpenOptions, io::Write};
// Third Party Imports
// Local Imports
use harmos::storage::state::{RecoveryEntry, RecoverySnapshot, StateBackend, file::FileBackend};
use harmos::storage::{RecoveryPosition, StorageCursor};

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
#[tokio::test]
async fn checkpoint_tail_survives_reopen_and_incomplete_append() {
    let root = tempfile::tempdir().unwrap();
    let history = super::history();
    let settings = super::storage_settings();
    let backend = FileBackend::new(root.path().to_owned(), history.clone(), &settings);
    let at = |sequence| RecoveryPosition {
        history: history.clone(),
        sequence,
    };
    let required_data = StorageCursor {
        history: history.clone(),
        sequence: 0,
    };
    assert_eq!(backend.open().await.unwrap(), at(0));
    let entries = (1..=3)
        .map(|sequence| RecoveryEntry {
            position: at(sequence),
            required_data: required_data.clone(),
            payload: vec![sequence as u8],
        })
        .collect::<Vec<_>>();
    backend.write_entries(&entries).await.unwrap();
    assert_eq!(backend.flush(&at(3)).await.unwrap(), at(3));
    backend.write_entries(&entries).await.unwrap();
    let snapshot = RecoverySnapshot {
        through: at(2),
        required_data,
        payload: b"checkpoint".to_vec(),
    };
    backend.write_snapshot(&snapshot).await.unwrap();
    backend.close().await.unwrap();

    let mut file = OpenOptions::new()
        .append(true)
        .open(root.path().join("state.jsonl"))
        .unwrap();
    file.write_all(b"{\"value\":").unwrap();
    file.sync_all().unwrap();
    drop(file);

    let reopened = FileBackend::new(root.path().to_owned(), history, &settings);
    assert_eq!(reopened.open().await.unwrap().sequence, 3);
    assert_eq!(
        reopened
            .read_snapshot(settings.snapshot_bytes)
            .await
            .unwrap(),
        Some(snapshot.clone())
    );
    let through = RecoveryPosition {
        history: snapshot.through.history.clone(),
        sequence: 3,
    };
    let page = reopened
        .read_entries(&snapshot.through, &through, settings.query_page)
        .await
        .unwrap();
    assert_eq!(page.entries, entries[2..]);
    assert_eq!(page.next, None);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn complete_corruption_is_preserved_and_refused() {
    let root = tempfile::tempdir().unwrap();
    let history = super::history();
    let settings = super::storage_settings();
    let backend = FileBackend::new(root.path().to_owned(), history.clone(), &settings);
    backend.open().await.unwrap();
    let position = RecoveryPosition {
        history: history.clone(),
        sequence: 1,
    };
    let entry = RecoveryEntry {
        position: position.clone(),
        required_data: StorageCursor {
            history: history.clone(),
            sequence: 0,
        },
        payload: b"saved state".to_vec(),
    };
    backend.write_entries(&[entry]).await.unwrap();
    backend.flush(&position).await.unwrap();
    backend.close().await.unwrap();

    // A Complete Invalid Entry Must Not Be Treated As An Interrupted Append.
    let path = root.path().join("state.jsonl");
    let mut file = OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(b"{}\n").unwrap();
    file.sync_all().unwrap();
    drop(file);
    let corrupt = std::fs::read(&path).unwrap();
    let reopened = FileBackend::new(root.path().to_owned(), history, &settings);
    assert!(reopened.open().await.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), corrupt);
    assert!(reopened.open().await.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), corrupt);
}

#[tokio::test]
async fn state_root_has_one_owner_until_close() {
    let root = tempfile::tempdir().unwrap();
    let history = super::history();
    let settings = super::storage_settings();
    let first = FileBackend::new(root.path().to_owned(), history.clone(), &settings);
    let second = FileBackend::new(root.path().to_owned(), history.clone(), &settings);
    first.open().await.unwrap();
    assert!(second.open().await.is_err());
    let position = RecoveryPosition {
        history: history.clone(),
        sequence: 1,
    };
    let entry = RecoveryEntry {
        position: position.clone(),
        required_data: StorageCursor {
            history: history.clone(),
            sequence: 0,
        },
        payload: b"exclusive writer".to_vec(),
    };
    first
        .write_entries(std::slice::from_ref(&entry))
        .await
        .unwrap();
    first.flush(&position).await.unwrap();
    first.close().await.unwrap();

    // A Refused Owner Can Acquire The Root After The Original Owner Closes.
    assert_eq!(second.open().await.unwrap(), position);
    let after = RecoveryPosition {
        history,
        sequence: 0,
    };
    let page = second
        .read_entries(&after, &position, settings.query_page)
        .await
        .unwrap();
    assert_eq!(page.entries, vec![entry]);
    assert_eq!(page.next, None);
    assert!(first.open().await.is_err());
    second.close().await.unwrap();
    assert_eq!(first.open().await.unwrap(), position);
    first.close().await.unwrap();
}
