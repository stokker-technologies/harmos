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

// Third Party Imports

// Local Imports
use super::{Fixture, Lanes, Operations, config};
use harmos::storage::operations::{
    WriteRecords, WriteRecordsRequest, WriteStream, WriteStreamRequest,
};
use harmos::storage::{MutationId, RecordInput, StreamInput};
use harmos::{Identifier, MemorySize, Message};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn input(name: &str) -> RecordInput {
    RecordInput {
        schema: super::super::schema(),
        scope: None,
        item: Identifier::new(name).unwrap(),
        source: None,
        source_time: None,
        payload: name.as_bytes().to_vec(),
    }
}
fn request(f: &Fixture, key: &str) -> WriteRecordsRequest {
    WriteRecordsRequest {
        mutation: MutationId {
            window: f.storage.mutation_window().unwrap().id,
            key: Identifier::new(key).unwrap(),
        },
        records: vec![input("first"), input("second")],
        source: Identifier::new("bench").unwrap(),
    }
}
async fn fixture() -> Fixture {
    let f = Fixture::new(config()).await;
    Operations::register(
        &f.runtime.operations,
        "storage",
        Lanes::Open,
        Arc::new(f.storage.clone()),
    )
    .unwrap();
    f
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("matching_writes_share", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = fixture().await;
                    let request = request(&f, "records");
                    let (left, right) = tokio::join!(
                        f.runtime.operations.call::<WriteRecords>(request.clone()),
                        f.runtime.operations.call::<WriteRecords>(request.clone())
                    );
                    let original = left.unwrap();
                    assert_eq!(right.unwrap(), original);
                    assert_eq!(original.sequence, 2);
                    let later = f
                        .runtime
                        .operations
                        .call::<WriteRecords>(self::request(&f, "later"))
                        .await;
                    assert!(later.is_ok());
                    assert_eq!(
                        f.runtime
                            .operations
                            .call::<WriteRecords>(request.clone())
                            .await
                            .unwrap(),
                        original
                    );
                    assert_eq!(f.storage.cursor_accepted().sequence, 4);
                    let mut conflicting = request.clone();
                    conflicting.records[0].payload.push(0);
                    assert!(
                        f.runtime
                            .operations
                            .call::<WriteRecords>(conflicting)
                            .await
                            .is_err()
                    );
                    f.storage.flush().await.unwrap();
                    assert_eq!(f.data.records.lock().unwrap().len(), 4);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("stream_writes_preserve", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = fixture().await;
                    let record = request(&f, "shared-key");
                    let request = WriteStreamRequest {
                        mutation: record.mutation.clone(),
                        rows: vec![StreamInput {
                            record: input("sample"),
                            stream: Identifier::new("sensor").unwrap(),
                            sequence: 999,
                        }],
                    };
                    let original = f
                        .runtime
                        .operations
                        .call::<WriteStream>(request.clone())
                        .await
                        .unwrap();
                    assert_eq!(
                        f.runtime
                            .operations
                            .call::<WriteStream>(request)
                            .await
                            .unwrap(),
                        original
                    );
                    assert!(
                        f.runtime
                            .operations
                            .call::<WriteRecords>(record)
                            .await
                            .is_err()
                    );
                    f.storage.flush().await.unwrap();
                    assert_eq!(f.data.rows.lock().unwrap()[&1].sequence, 999);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("pressure_preserves_admission", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.max_mutations = NonZeroUsize::MIN;
                    let f = Fixture::new(settings).await;
                    let request = request(&f, "first");
                    let values = request
                        .records
                        .iter()
                        .map(RecordInput::borrowed)
                        .collect::<Vec<_>>();
                    let accepted = f
                        .storage
                        .write_records_once(&request.mutation, &values)
                        .await
                        .unwrap();
                    let next = self::request(&f, "next");
                    assert!(
                        f.storage
                            .write_records_once(&next.mutation, &values)
                            .await
                            .is_err()
                    );
                    assert_eq!(f.storage.cursor_accepted(), accepted);
                    assert_eq!(
                        f.storage
                            .write_records_once(&request.mutation, &values)
                            .await
                            .unwrap(),
                        accepted
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("empty_mutations_rejected", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = fixture().await;
                    let mut request = request(&f, "empty");
                    request.records.clear();
                    assert!(
                        f.runtime
                            .operations
                            .call::<WriteRecords>(request)
                            .await
                            .is_err()
                    );
                    assert_eq!(f.storage.cursor_accepted().sequence, 0);
                    assert_eq!(f.storage.status().mutation_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("repeated_input_codec", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = fixture().await;
                    let request = request(&f, "codec");
                    let bytes = request.encode_to_vec();
                    let memory = WriteRecordsRequest::decoding_memory(&bytes).unwrap();
                    let decoded = WriteRecordsRequest::decode(&bytes).unwrap();
                    assert_eq!(decoded, request);
                    assert!(decoded.memory_size() <= memory);
                    let mut malformed = bytes;
                    malformed.extend_from_slice(&[0x12, 1, 0xff]);
                    assert!(WriteRecordsRequest::decoding_memory(&malformed).is_err());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("failure_preserves_accepted", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use std::sync::atomic::Ordering;
                    let f = fixture().await;
                    let request = request(&f, "accepted");
                    let values = request
                        .records
                        .iter()
                        .map(RecordInput::borrowed)
                        .collect::<Vec<_>>();
                    let accepted = f
                        .storage
                        .write_records_once(&request.mutation, &values)
                        .await
                        .unwrap();
                    f.data.control.failures.store(1, Ordering::SeqCst);
                    assert!(f.storage.flush().await.is_err());
                    assert_eq!(
                        f.storage
                            .write_records_once(&request.mutation, &values)
                            .await
                            .unwrap(),
                        accepted
                    );
                    assert!(f.storage.status().failure.is_some());
                    f.storage.flush().await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("refusal_releases_queue", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.mutation_memory_bytes = NonZeroUsize::new(128).unwrap();
                    let f = Fixture::new(settings).await;
                    let request = request(&f, "too-large");
                    let values = request
                        .records
                        .iter()
                        .map(RecordInput::borrowed)
                        .collect::<Vec<_>>();
                    assert!(
                        f.storage
                            .write_records_once(&request.mutation, &values)
                            .await
                            .is_err()
                    );
                    assert_eq!(f.storage.cursor_accepted().sequence, 0);
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    f.storage.write_records(&values).await.unwrap();
                    assert_eq!(f.storage.cursor_accepted().sequence, 2);
                    f.finish().await;
                });
        });
    }
}
