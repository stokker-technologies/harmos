/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Directory Selection, Retained Read Ownership And Native Subscription Stories

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::borrow::Borrow;
use std::time::Duration;
// Third Party Imports
// Local Imports
use super::manager::fixture::{Fixture, config};
use harmos_domain::model::storage::{DirectoryEntry, DirectoryEvent, DirectoryRule, DirectorySelection};
use harmos_storage::{DirectoryWatcher, ReadBatch};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn selection(root: &std::path::Path) -> DirectorySelection {
    DirectorySelection {
        root: root.to_owned(),
        recursive: true,
        rules: vec![DirectoryRule { pattern: "**".into(), include: true }],
    }
}

async fn matching(watcher: &mut DirectoryWatcher, predicate: impl Fn(&DirectoryEvent) -> bool) -> ReadBatch<DirectoryEvent> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let item = watcher.next().await.unwrap().expect("native watcher remains subscribed");
            if predicate(&item) {
                return item;
            }
        }
    })
    .await
    .expect("native event arrives")
}

/* === Tests === */

#[tokio::test]
async fn selection_retains_charges() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("private/released")).unwrap();
    std::fs::write(root.path().join("keep.json"), b"keep").unwrap();
    std::fs::write(root.path().join("private/secret.json"), b"secret").unwrap();
    std::fs::write(root.path().join("private/released/data.json"), b"released").unwrap();
    let fixture = Fixture::new(config()).await;
    let mut request = selection(root.path());
    request.rules = vec![
        DirectoryRule {
            pattern: "**/*.json".into(),
            include: true,
        },
        DirectoryRule {
            pattern: "private/**".into(),
            include: false,
        },
        DirectoryRule {
            pattern: "private/released/**".into(),
            include: true,
        },
    ];
    let mut reader = fixture.storage.list_directory(request).await.unwrap();
    assert!(reader.completion().is_err());
    let mut rows = Vec::new();
    while let Some(entry) = reader.next().await.unwrap() {
        rows.push(entry);
    }
    assert_eq!(reader.completion().unwrap().entries, 2);
    rows.sort_by(|left, right| left.path.cmp(&right.path));
    assert_eq!(rows.iter().map(|row| row.path.as_str()).collect::<Vec<_>>(), ["keep.json", "private/released/data.json"]);
    assert_eq!(rows[1].length, 8);
    let retained = rows[0].clone();
    let owner = retained.owner();
    let borrowed: &DirectoryEntry = owner.as_ref().borrow();
    assert_eq!(borrowed.path.as_ptr(), retained.path.as_ptr());

    drop(rows);
    drop(reader);
    drop(retained);
    drop(owner);
    fixture.finish().await;
}

#[tokio::test]
async fn excluded_entries_bounded() {
    let root = tempfile::tempdir().unwrap();
    for name in ["one", "two", "three"] {
        std::fs::write(root.path().join(name), b"x").unwrap();
    }
    let mut settings = config();
    settings.filesystem.max_entries = std::num::NonZeroUsize::new(2).unwrap();
    let fixture = Fixture::new(settings).await;
    let mut request = selection(root.path());
    request.rules.clear();
    let mut reader = fixture.storage.list_directory(request).await.unwrap();
    assert!(matches!(reader.next().await, Err(harmos_storage::Error::ResourceExhausted { resource: "directory entries" })));
    drop(reader);
    fixture.finish().await;
}

#[tokio::test]
async fn subscribed_before_scan() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("existing"), b"old").unwrap();
    let fixture = Fixture::new(config()).await;
    let mut watcher = fixture.storage.watch_directory(selection(root.path()), true).await.unwrap();
    std::fs::write(root.path().join("created"), b"new").unwrap();
    let mut existing = Vec::new();
    loop {
        let event = watcher.next().await.unwrap().unwrap();
        match &*event {
            DirectoryEvent::Existing(entry) => existing.push(entry.path.clone()),
            DirectoryEvent::InitialScanComplete => break,
            other => panic!("initial scan ordering: {other:?}"),
        }
    }
    assert!(existing.iter().any(|path| path == "existing"));
    matching(&mut watcher, |event| matches!(event, DirectoryEvent::Created(entry) if entry.path == "created")).await;
    std::fs::write(root.path().join("created"), b"modified").unwrap();
    matching(&mut watcher, |event| matches!(event, DirectoryEvent::Modified(entry) if entry.path == "created")).await;
    std::fs::remove_file(root.path().join("created")).unwrap();
    matching(&mut watcher, |event| matches!(event, DirectoryEvent::Removed { path } if path == "created")).await;
    drop(watcher);
    fixture.finish().await;
}

#[cfg(unix)]
#[tokio::test]
async fn symbolic_directories_skipped() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), b"outside").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("linked")).unwrap();
    let fixture = Fixture::new(config()).await;
    let mut reader = fixture.storage.list_directory(selection(root.path())).await.unwrap();
    assert!(reader.next().await.unwrap().is_none());
    assert_eq!(reader.completion().unwrap().entries, 0);
    drop(reader);
    fixture.finish().await;
}

#[tokio::test]
async fn close_wakes_watch() {
    let root = tempfile::tempdir().unwrap();
    let fixture = Fixture::new(config()).await;
    let mut watcher = fixture.storage.watch_directory(selection(root.path()), false).await.unwrap();
    assert!(matches!(&*watcher.next().await.unwrap().unwrap(), DirectoryEvent::InitialScanComplete));

    let mut pending = Box::pin(watcher.next());
    assert!(super::manager::fixture::pending(pending.as_mut()).await);
    fixture.storage.close().await.unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(10), &mut pending).await.unwrap().unwrap().is_none());
    drop(pending);
    drop(watcher);
    fixture.finish().await;
}
