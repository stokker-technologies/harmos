/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Upload a Durable File Through Generated Protobuf Batches.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{Fixture, config};

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
async fn uploaded_file_is_durable_and_readable() {
    use harmos_operations::storage::WriteFile;
    use harmos_protocol::storage as wire;
    let fixture = Fixture::new(config()).await;
    fixture.runtime.operations.register::<harmos_operations::Storage>(super::support::Lanes::Open).await.unwrap();
    let mut upload = fixture
        .runtime
        .operations
        .upload_protobuf::<WriteFile>(
            wire::WriteFile {
                metadata: Some(wire::FileMetadata {
                    id: "uploaded".into(),
                    media_type: "application/octet-stream".into(),
                    expected_length: Some(12),
                    ..Default::default()
                }),
            },
            harmos_runtime::operations::StreamSettings {
                items: std::num::NonZeroUsize::MIN,
                bytes: std::num::NonZeroUsize::new(4096).unwrap(),
            },
        )
        .unwrap();
    upload.send(wire::FileBatch { offset: 0, bytes: b"first-".to_vec() }).await.unwrap();
    upload.send(wire::FileBatch { offset: 6, bytes: b"second".to_vec() }).await.unwrap();
    let reference = upload.finish().await.unwrap();
    assert_eq!(reference.length, 12);
    assert_eq!(reference.position, Some(fixture.storage.cursor_saved().into()));
    let file = harmos_runtime::storage::FileReference::try_from(reference).unwrap();
    let mut reader = fixture
        .storage
        .read_file(harmos_runtime::storage::FileRead {
            source: harmos_runtime::storage::FileSource::Stored(file),
            range: harmos_runtime::storage::ByteRange { offset: 0, limit: None },
        })
        .await
        .unwrap();
    let mut bytes = Vec::new();
    while let Some(batch) = reader.next().await.unwrap() {
        bytes.extend_from_slice(batch.bytes());
    }
    assert_eq!(bytes, b"first-second");
    drop(reader);
    fixture.finish().await;
}
