/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{Fixture, Lanes, Operations, config};
use harmos::Identifier;
use harmos::storage::operations::{FileBytes, FileSelection, FollowFile, ReadFiles};
use harmos::storage::{FileMetadata, FilePhase, StagedFile};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
async fn setup() -> (Fixture, harmos::storage::StorageFile, FileSelection) {
    let fixture = Fixture::new(config()).await;
    Operations::register(
        &fixture.runtime.operations,
        "storage",
        Lanes::Open,
        Arc::new(fixture.storage.clone()),
    )
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
    producer.write_all(b"prefix").await.unwrap();
    let session = producer.session().unwrap();
    let request = FileSelection {
        file: StagedFile {
            history: session.history.clone(),
            id: session.id.clone(),
        },
        offset: 0,
        chunk_bytes: 2,
    };
    (fixture, producer, request)
}

fn limits() -> harmos::operations::StreamSettings {
    harmos::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(1024).unwrap(),
    }
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("read_file_fixes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(5), async {
                        let (fixture, mut producer, request) = setup().await;
                        let (sender, mut receiver) =
                            harmos::operations::stream::<FileBytes>(limits()).unwrap();
                        let instance = fixture
                            .runtime
                            .operations
                            .submit_endpoints::<ReadFiles, _>(request, sender)
                            .unwrap();
                        let first = receiver.recv().await.unwrap();
                        assert_eq!(first.offset, 0);
                        let mut bytes = first.bytes.clone();
                        drop(first);
                        producer.write_all(b"later").await.unwrap();
                        producer.finish().await.unwrap();
                        while let Some(chunk) = receiver.recv().await {
                            assert_eq!(chunk.offset, bytes.len() as u64);
                            assert!(chunk.bytes.len() <= 2);
                            bytes.extend_from_slice(&chunk.bytes);
                        }
                        assert_eq!(bytes, b"prefix");
                        let result = instance.await.unwrap();
                        assert_eq!(result.length, 6);
                        assert_eq!(result.phase, FilePhase::Producing);
                        fixture.finish().await;
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("reconnects_never_cancel", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(5), async {
                        let (fixture, mut producer, mut request) = setup().await;
                        let (sender, mut receiver) =
                            harmos::operations::stream::<FileBytes>(limits()).unwrap();
                        let instance = fixture
                            .runtime
                            .operations
                            .submit_endpoints::<FollowFile, _>(request.clone(), sender)
                            .unwrap();
                        let mut offset = 0;
                        while offset < 6 {
                            let chunk = receiver.recv().await.unwrap();
                            assert_eq!(chunk.offset, offset);
                            offset += chunk.bytes.len() as u64;
                        }
                        instance.cancel();
                        assert!(instance.await.is_err());
                        drop(receiver);
                        producer.write_all(b"later").await.unwrap();
                        request.offset = offset;
                        let (sender, mut receiver) =
                            harmos::operations::stream::<FileBytes>(limits()).unwrap();
                        let instance = fixture
                            .runtime
                            .operations
                            .submit_endpoints::<FollowFile, _>(request, sender)
                            .unwrap();
                        let mut bytes = Vec::new();
                        while offset < 11 {
                            let chunk = receiver.recv().await.unwrap();
                            assert_eq!(chunk.offset, offset);
                            bytes.extend_from_slice(&chunk.bytes);
                            offset += chunk.bytes.len() as u64;
                        }
                        assert_eq!(bytes, b"later");
                        producer.abort().await.unwrap();
                        assert!(receiver.recv().await.is_none());
                        let result = instance.await.unwrap();
                        assert_eq!(result.length, 11);
                        assert_eq!(result.phase, FilePhase::Abandoned);
                        fixture.finish().await;
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("operations_reject_future", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (fixture, mut producer, request) = setup().await;
                    for invalid in [
                        FileSelection {
                            offset: 7,
                            ..request.clone()
                        },
                        FileSelection {
                            chunk_bytes: 0,
                            ..request
                        },
                    ] {
                        let (sender, mut receiver) =
                            harmos::operations::stream::<FileBytes>(limits()).unwrap();
                        let result = fixture
                            .runtime
                            .operations
                            .submit_endpoints::<FollowFile, _>(invalid, sender);
                        if let Ok(instance) = result {
                            assert!(instance.await.is_err());
                        }
                        assert!(receiver.recv().await.is_none());
                    }
                    producer.abort().await.unwrap();
                    fixture.finish().await;
                });
        });
    }
}
