/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Reject File Upload Batches With Noncontiguous Offsets.

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
async fn rejects_noncontiguous_input() {
    use harmos_operations::storage::WriteFile;
    use harmos_protocol::storage as wire;
    let fixture = Fixture::new(config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let mut upload = fixture
        .runtime
        .operations
        .upload_protobuf::<WriteFile>(
            wire::WriteFile {
                metadata: Some(wire::FileMetadata {
                    id: "gapped".into(),
                    media_type: "application/octet-stream".into(),
                    ..Default::default()
                }),
            },
            harmos_runtime::operations::StreamSettings {
                items: std::num::NonZeroUsize::MIN,
                bytes: std::num::NonZeroUsize::new(4096).unwrap(),
            },
        )
        .unwrap();
    upload.send(wire::FileBatch { offset: 2, bytes: b"gap".to_vec() }).await.unwrap();
    let harmos_runtime::Error::OperationFailed { source, .. } = upload.finish().await.unwrap_err() else {
        panic!("expected input refusal");
    };
    assert!(matches!(source.as_ref().downcast_ref::<harmos_operations::Error>(), Some(harmos_operations::Error::InvalidInput { .. })));
    fixture.finish().await;
}
