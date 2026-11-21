/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Prove Native File Path Rules

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::ffi::OsString;
use std::os::unix::{self, ffi::OsStringExt};
use std::{fs, sync::Arc};

// Third Party Imports

// Local Imports
use super::{Backend, history, storage};
use harmos_storage::{ByteRange, FileRead, FileSource};

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
async fn native_path_rules() {
    let data = Arc::new(Backend::new(history()));
    let directory = tempfile::tempdir().unwrap();
    let name = OsString::from_vec(vec![b'n', 0xff]);
    let path = directory.path().join(name);
    fs::write(&path, b"native bytes").unwrap();
    let storage = storage(&data).await;
    let request = FileRead {
        source: FileSource::Local(path.clone()),
        range: ByteRange { offset: 0, limit: None },
    };
    assert_eq!(storage.hash_file(request.clone()).await.unwrap().bytes_read, 12);
    let link = directory.path().join("linked-file");
    unix::fs::symlink(&path, &link).unwrap();
    assert!(
        storage
            .hash_file(FileRead {
                source: FileSource::Local(link),
                ..request.clone()
            })
            .await
            .is_err()
    );
    let alias = directory.path().join("linked-directory");
    unix::fs::symlink(directory.path(), &alias).unwrap();
    let through = alias.join(path.file_name().unwrap());
    assert!(
        storage
            .hash_file(FileRead {
                source: FileSource::Local(through),
                ..request
            })
            .await
            .is_err()
    );
    storage.close().await.unwrap();
}
