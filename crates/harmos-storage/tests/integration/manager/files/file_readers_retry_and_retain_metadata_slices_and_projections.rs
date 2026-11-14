/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

#[tokio::test]
async fn file_readers_retry_and_retain_metadata_slices_and_projections() {
    // Failed Reads Keep Their Offset And Can Retry The Same File.
    let f = Fixture::new(config()).await;
    let file = f.storage.write_file(metadata("owned", b"abc"), b"abc".to_vec()).await.unwrap();
    let mut reader = f.storage.read_file(whole(&file)).await.unwrap();
    f.data.control.corrupt_file.store(true, Ordering::SeqCst);
    assert!(reader.next().await.is_err());
    assert_eq!(reader.offset(), 0);
    f.data.control.corrupt_file.store(false, Ordering::SeqCst);
    f.storage.flush().await.unwrap();
    assert_eq!(f.storage.status().pending_batches, 0);
    assert_eq!(reader.next().await.unwrap().unwrap().bytes(), b"abc");

    // Completion Survives A Pending Backend Snapshot Release.
    f.data.control.block_release.store(true, Ordering::SeqCst);
    assert!(reader.next().await.unwrap().is_none());
    f.data.control.release_entered.notified().await;
    assert_eq!(reader.completion().unwrap().bytes, file.length);
    drop(reader);
    f.data.control.block_release.store(false, Ordering::SeqCst);
    f.data.control.release_release.notify_one();
    f.data.snapshots_released().await;

    // Projections Transfer The Buffer; Shared Slices Refuse That Transfer.
    for sliced in [false, true] {
        let session = f.storage.clone();
        let mut reader = session.read_file(whole(&file)).await.unwrap();
        let batch = reader.next().await.unwrap().unwrap();
        let pointer = batch.bytes().as_ptr();
        let slice = sliced.then(|| batch.slice(1..3));
        let called = std::cell::Cell::new(false);
        let result = batch.try_map(|batch| {
            called.set(true);
            Ok::<_, Error>(batch.bytes)
        });
        assert_eq!(called.get(), !sliced);
        drop(reader);
        drop(session);
        f.data.snapshots_released().await;
        if let Some(slice) = slice {
            assert!(result.is_err());
            assert_eq!(slice.bytes(), b"bc");
        } else {
            let projected = result.unwrap();
            assert_eq!(projected.as_ptr(), pointer);
            assert_eq!(projected.as_slice(), b"abc");
        }
    }
    f.finish().await;
}
