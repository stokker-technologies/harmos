/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "generated_future_offset.rs"]
mod future_offset;
#[path = "generated_read.rs"]
mod read;
#[path = "generated_reconnect.rs"]
mod reconnect;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports

// Local Imports
use super::{Fixture, config};
use harmos_operations::storage::{FollowProduction, ReadFile};
use harmos_protocol::storage as wire;
use harmos_runtime::Identifier;
use harmos_runtime::storage::{FileMetadata, StagedFile};
use prost::Message;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

async fn setup() -> (Fixture, harmos_runtime::storage::StorageFile, wire::FollowProduction) {
    let fixture = Fixture::new(config()).await;
    fixture
        .runtime
        .operations
        .register_catalog(harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let mut producer = fixture
        .storage
        .produce_file(FileMetadata {
            id: Identifier::new("recording").unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: None,
            expected_sha256: None,
        })
        .await
        .unwrap();
    producer.write_all((b"prefix").to_vec()).await.unwrap();
    let session = producer.session().unwrap();
    let request = wire::FollowProduction {
        file: Some(
            StagedFile {
                history: session.history.clone(),
                id: session.id.clone(),
            }
            .into(),
        ),
        offset: 0,
    };
    (fixture, producer, request)
}

fn limits() -> Option<harmos_runtime::operations::WireStreamLimits> {
    let settings = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(1024).unwrap(),
    };
    Some(harmos_runtime::operations::WireStreamLimits { input: settings, output: settings })
}

async fn receive(streams: &harmos_runtime::operations::WireStreams) -> Option<wire::FileBatch> {
    streams.receive_current().await.unwrap().map(|bytes| wire::FileBatch::decode(bytes.as_slice()).unwrap())
}

/* === Tests === */
