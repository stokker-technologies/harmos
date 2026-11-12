/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Local Imports
use super::*;

/* === Tests === */

#[tokio::test]
async fn opened_clones_keep_independent_read_offsets() {
    use std::io::{Seek, SeekFrom, Write};
    let mut policy = config();
    policy.response.batch_bytes = NonZeroUsize::new(31).unwrap();
    let f = Fixture::new(policy).await;
    let expected: Vec<u8> = (0..127).map(|index| (index % 251) as u8).collect();
    let (_source_owner, mut source) = shareable_tempfile();
    source.write_all(&expected).unwrap();
    let mut observer = source.try_clone().unwrap();
    observer.seek(SeekFrom::Start(11)).unwrap();
    let session = f.storage.clone();
    let mut left = session.read_opened_file(source.try_clone().unwrap(), ByteRange { offset: 0, limit: Some(63) }).await.unwrap();
    let mut right = session.read_opened_file(source, ByteRange { offset: 64, limit: Some(63) }).await.unwrap();
    let mut chunks = 0;
    loop {
        let (first, second) = tokio::join!(left.next(), right.next());
        let (first, second) = (first.unwrap(), second.unwrap());
        assert_eq!(observer.stream_position().unwrap(), 11);
        let (Some(first), Some(second)) = (first, second) else {
            break;
        };
        chunks += 1;
        let left_offset = first.offset() as usize;
        let right_offset = second.offset() as usize;
        assert_eq!(first.bytes(), &expected[left_offset..left_offset + first.bytes().len()]);
        assert_eq!(second.bytes(), &expected[right_offset..right_offset + second.bytes().len()]);
    }
    assert!(chunks > 1, "interleaved reads cross the response batch boundary");
    assert_eq!(left.completion().unwrap().bytes, 63);
    assert_eq!(right.completion().unwrap().bytes, 63);
    drop((left, right, session));
    f.finish().await;
}
