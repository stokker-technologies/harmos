/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use super::nominal_fixture::*;

/* === Tests === */

#[tokio::test]
async fn file_ranges_read() {
    // Empty, Small And Multi-Batch Files Use Deterministic Binary Contents.
    let n = Nominal::new().await;
    let batch_bytes = storage_settings().response.batch_bytes.get();
    for length in [0, 17, batch_bytes * 2 + 13] {
        let expected = waveforms::binary(length);
        let metadata = FileMetadata {
            id: Identifier::new(format!("binary-{length}")).unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: Some(length as u64),
            expected_sha256: None,
        };
        let reference = n.storage.write_file(metadata, (expected).to_vec()).await.unwrap();
        let range = |offset, limit| ByteRange { offset, limit };
        let read = read_range(&n.storage, &reference, range(0, None)).await;
        assert!(read.offsets.iter().all(|(actual, expected)| actual == expected));
        assert_eq!(read.completed_bytes, read.bytes.len() as u64);
        assert_eq!(read.bytes, expected);

        // Finished References Support Checked Half-Open Byte Ranges.
        let offset = reference.length.min(3);
        let selected = read_range(&n.storage, &reference, range(offset, Some(7))).await;
        assert!(selected.offsets.iter().all(|(actual, expected)| actual == expected));
        assert_eq!(selected.completed_bytes, selected.bytes.len() as u64);
        assert_eq!(selected.bytes, expected[offset as usize..expected.len().min(offset as usize + 7)]);
        let end = read_range(&n.storage, &reference, range(reference.length, None)).await;
        assert!(end.bytes.is_empty());
        assert!(end.offsets.is_empty());
        assert_eq!(end.completed_bytes, 0);
        let beyond = n
            .storage
            .read_file(FileRead {
                source: harmos_domain::model::storage::files::FileSource::Stored(reference.clone()),
                range: range(reference.length + 1, None),
            })
            .await;
        assert!(beyond.is_err());
    }
    n.finish().await;
}
