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
#[path = "generated_reconnect.rs"]
mod reconnect;

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports

// Local Imports
use super::{Fixture, config};
use harmos_operations::storage::WatchFileProduction;
use harmos_protocol::storage as wire;
use harmos_protocol::v1;
use harmos_runtime::Identifier;
use harmos_runtime::storage::{FileMetadata, StagedFile};
use prost::Message;

/* === Private Functions === */

async fn setup() -> (Fixture, harmos_runtime::storage::StorageFile, v1::StorageWatchFileProductionRequest) {
    let fixture = Fixture::new(config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
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
    let request = v1::StorageWatchFileProductionRequest {
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

async fn receive_production(streams: &harmos_runtime::operations::WireStreams) -> Option<wire::FileBatch> {
    let bytes = streams.receive_current().await.unwrap()?;
    match v1::StorageWatchFileProductionResponse::decode(bytes.as_slice()).unwrap().event {
        Some(v1::storage_watch_file_production_response::Event::Data(batch)) => Some(batch),
        Some(v1::storage_watch_file_production_response::Event::Progress(_)) => None,
        None => panic!("production observation requires an event"),
    }
}
