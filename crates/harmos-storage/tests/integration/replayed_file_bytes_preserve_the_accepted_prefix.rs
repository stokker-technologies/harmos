/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Preserve Accepted Upload Bytes Across Replays and Restart.

/* === Imports === */

use super::file_sessions::{metadata, mutation, storage};
use super::manager::backend::{Data, history};
use sha2::{Digest, Sha256};
use std::sync::Arc;

/* === Tests === */

#[tokio::test(start_paused = true)]
async fn replayed_file_bytes_preserve_the_accepted_prefix() {
    let data = Arc::new(Data::new(history()));
    let first = storage(&data).await;
    let created = first.create_file_once(&mutation(&first, "create"), &metadata("prefix")).await.unwrap();
    let initial = mutation(&first, "initial");
    let written = first.append_file_once(&initial, &created.session, 0, b"accepted-prefix".to_vec()).await.unwrap();
    data.control.production_read_bytes.store(3, std::sync::atomic::Ordering::SeqCst);
    let replay = mutation(&first, "another-request");
    assert_eq!(first.append_file_once(&replay, &created.session, 0, b"accepted-prefix".to_vec()).await.unwrap(), written);
    let subset = mutation(&first, "subset");
    assert_eq!(first.append_file_once(&subset, &created.session, 9, b"prefix".to_vec()).await.unwrap(), written);
    let conflict = mutation(&first, "different-bytes");
    assert!(first.append_file_once(&conflict, &created.session, 0, b"conflict".to_vec()).await.is_err());
    assert!(first.append_file_once(&initial, &created.session, 0, b"different".to_vec()).await.is_err());
    assert!(first.append_file_once(&mutation(&first, "overlap"), &created.session, 14, b"overlap".to_vec()).await.is_err());
    assert!(first.append_file_once(&mutation(&first, "gap"), &created.session, 16, b"gap".to_vec()).await.is_err());
    assert_eq!(data.control.chunk_offsets.lock().unwrap().as_slice(), &[0]);
    first.flush().await.unwrap();
    first.close().await.unwrap();

    let second = storage(&data).await;
    assert_eq!(second.append_file_once(&replay, &created.session, 0, b"accepted-prefix".to_vec()).await.unwrap(), written);
    assert!(second.append_file_once(&conflict, &created.session, 0, b"conflict".to_vec()).await.is_err());
    let resumed = second.resume_file_once(&mutation(&second, "resume"), &created.session).await.unwrap();
    assert!(second.append_file_once(&mutation(&second, "fenced"), &created.session, 0, b"accepted-prefix".to_vec()).await.is_err());
    let extended = second.append_file_once(&mutation(&second, "tail"), &resumed.session, resumed.length, b"-tail".to_vec()).await.unwrap();
    assert_eq!(extended.length, b"accepted-prefix-tail".len() as u64);
    let reference = second.finish_file_once(&mutation(&second, "publish"), &resumed.session).await.unwrap();
    assert_eq!(reference.length, extended.length);
    assert_eq!(reference.sha256, <[u8; 32]>::from(Sha256::digest(b"accepted-prefix-tail")));
    second.close().await.unwrap();
}
