/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

#[tokio::test(start_paused = true)]
async fn file_roundtrip_recovers() {
    let large = (0..32).map(|n| (n % 251) as u8).collect::<Vec<_>>();
    let cases = [("boundary", large), ("small", b"one chunk".to_vec())];
    for (id, bytes) in cases {
        let mut policy = config();
        policy.response.batch_bytes = NonZeroUsize::new(31).unwrap();
        policy.transfer.chunk_bytes = Some(31);
        let f = Fixture::new(policy).await;
        let mut file = f.storage.create_file(metadata(id, &bytes)).await.unwrap();
        f.data.control.chunk_failures.store(1, Ordering::SeqCst);
        file.write_all((bytes).to_vec()).await.unwrap();
        let offsets = f.data.control.chunk_offsets.lock().unwrap().clone();
        assert_eq!(&offsets[..2], &[0, 0]);
        if bytes.len() <= 31 {
            assert_eq!(offsets, [0, 0]);
        }
        assert!(f.data.control.chunk_sizes.lock().unwrap().iter().all(|n| *n <= 31));

        // An Uncertain Publication Reuses Its Position Without Duplicating The Blob.
        f.data.control.finish_failures.store(1, Ordering::SeqCst);
        let reference = file.finish().await.unwrap();
        assert_eq!(reference.position.sequence, 1);
        assert_eq!(reference.length, bytes.len() as u64);
        assert!(f.data.control.finish_positions.lock().unwrap().iter().all(|position| *position == 1));
        assert_eq!(f.data.blobs.lock().unwrap().len(), 1);
        assert!(f.data.files.lock().unwrap().is_empty());
        file.abort().await.unwrap();
        assert_eq!(f.data.blobs.lock().unwrap().len(), 1);

        // Bounded Reads Reconstruct The Exact Published Bytes.
        let mut reader = f.storage.read_file(whole(&reference)).await.unwrap();
        let first = reader.next().await.unwrap().unwrap();
        assert!(first.bytes().len() <= 31);
        assert_eq!(first.bytes(), &bytes[..first.bytes().len()]);
        if bytes.len() <= 31 {
            assert_eq!(first.bytes(), bytes);
        }
        let mut actual = first.bytes().to_vec();
        while let Some(batch) = reader.next().await.unwrap() {
            assert!(batch.bytes().len() <= 31);
            actual.extend_from_slice(batch.bytes());
        }
        assert_eq!(actual, bytes);
        drop(reader);

        // Logical Publication Remains Separate From Durable Query Visibility.
        assert_eq!(f.storage.status().saved.sequence, 0);
        let mut durable = f
            .storage
            .query(harmos_storage::Query {
                at: f.storage.get_saved_position(),
                source: harmos_storage::query::Source::Files(harmos_storage::query::FileSelection { scope: None, item: None }),
                transforms: Vec::new(),
                slice: harmos_storage::query::RowSlice::default(),
            })
            .await
            .unwrap();
        assert!(matches!(durable.next().await.unwrap(), Some(harmos_storage::query::QueryArrow::Schema(_))));
        assert!(durable.next().await.unwrap().is_none());
        f.storage.flush().await.unwrap();
        assert_eq!(f.storage.status().saved.sequence, reference.position.sequence);
        drop(durable);
        f.finish().await;
    }
}
