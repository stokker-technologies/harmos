/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */
/* === Modules === */
/* === Re-Exports === */
/* === Imports === */
use super::backend::{Data, Recovery, history, storage_settings};
use super::support::{Bank, Ledger, catalog};
use arrow_array::{Array, Float64Array, ListArray, StructArray, UInt32Array};
use harmos::storage::query::{Query, QueryOutput, RecordSelection, RowSlice, Source};
use harmos::storage::*;
use harmos::{Identifier, Message, Runtime};
use std::num::NonZeroU32;
use std::sync::Arc;
/* === Constants === */
/* === Macros === */
/* === Type Aliases === */
/* === Structs === */
#[harmos::message]
struct Payload {
    label: Option<String>,
    values: Vec<f64>,
    event: Event,
}
/* === Enums === */
#[harmos::message]
enum Event {
    Reading(i64),
    Empty,
}
/* === Traits === */
/* === Implementations === */
/* === Public Functions === */
/* === Private Functions === */
/* === Tests === */
#[tokio::test]
async fn mapping() {
    let directory = tempfile::tempdir().unwrap();
    let settings = storage_settings();
    let data = Arc::new(Data::<()>::journaled(directory.path(), history()).unwrap());
    let storage = StorageManager::new(
        StorageBackends {
            primary: data,
            state: Arc::new(Recovery::new(history())),
            replicas: vec![],
        },
        settings,
    )
    .unwrap();
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .runtime_with_storage(&storage)
        .await
        .unwrap();
    let schema = SchemaKey {
        id: Identifier::new("mapping").unwrap(),
        version: NonZeroU32::new(1).unwrap(),
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = harmos::schema::graph::Schema::from_declaration(
        <Payload as harmos::schema::Described>::DEFINITION,
        &harmos::config::SchemaSettings::default(),
    )
    .unwrap();
    storage.write_schema(&schema, &graph).await.unwrap();
    let payloads = [
        Payload {
            label: None,
            values: vec![0.0, -1.0, 2.0],
            event: Event::Reading(-7),
        },
        Payload {
            label: Some("second".into()),
            values: vec![],
            event: Event::Empty,
        },
    ];
    for (index, payload) in payloads.iter().enumerate() {
        let item = Identifier::new(format!("row-{index}")).unwrap();
        storage
            .write_records(&[RecordWrite {
                schema: &schema,
                scope: None,
                item: &item,
                source: None,
                source_time: None,
                payload: &payload.encode_to_vec(),
            }])
            .await
            .unwrap();
    }
    let at = storage.flush().await.unwrap();
    let request = |item| Query {
        at: at.clone(),
        source: Source::Records(RecordSelection {
            schema: schema.clone(),
            scope: None,
            item,
            source_start: None,
            source_end: None,
        }),
        transforms: vec![],
        slice: RowSlice {
            offset: 0,
            limit: None,
        },
    };
    let mut reader = storage.query(request(None)).await.unwrap();
    let Some(QueryOutput::Schema(announced)) = reader.next().await.unwrap() else {
        panic!("schema first");
    };
    let Some(QueryOutput::Batch(batch)) = reader.next().await.unwrap() else {
        panic!("data batch");
    };
    let payload = batch
        .as_record_batch()
        .column_by_name("payload")
        .unwrap()
        .as_any()
        .downcast_ref::<StructArray>()
        .unwrap();
    assert!(payload.column_by_name("label").unwrap().is_null(0));
    let values = payload
        .column_by_name("values")
        .unwrap()
        .as_any()
        .downcast_ref::<ListArray>()
        .unwrap();
    let first = values.value(0);
    assert_eq!(
        first
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap()
            .values()
            .as_ref(),
        &[0.0, -1.0, 2.0]
    );
    let event = payload
        .column_by_name("event")
        .unwrap()
        .as_any()
        .downcast_ref::<StructArray>()
        .unwrap();
    let discriminants = event
        .column_by_name("__variant")
        .unwrap()
        .as_any()
        .downcast_ref::<UInt32Array>()
        .unwrap();
    assert_eq!(discriminants.values().as_ref(), &[0, 1]);
    assert!(!event.column(1).is_null(0));
    assert!(event.column(1).is_null(1));
    assert!(event.column(2).is_null(0));
    assert!(!event.column(2).is_null(1));
    // A Raw Zero-Copy Clone Retains Its Storage Memory Charge After Reader Drop.
    let retained = batch.as_record_batch().clone();
    drop(batch);
    while reader.next().await.unwrap().is_some() {}
    drop(reader);
    assert!(storage.status().read_bytes > 0);
    drop(retained);

    let mut empty = storage
        .query(request(Some(Identifier::new("absent").unwrap())))
        .await
        .unwrap();
    let Some(QueryOutput::Schema(empty_schema)) = empty.next().await.unwrap() else {
        panic!("empty schema");
    };
    assert_eq!(announced.as_schema(), empty_schema.as_schema());
    assert!(empty.next().await.unwrap().is_none());
    assert_eq!(empty.completion().unwrap().rows, 0);
    drop(empty);

    // Four Admitted Queries Must Progress While Earlier Schema Output Is Retained.
    let mut readers = Vec::new();
    for _ in 0..4 {
        readers.push(storage.query(request(None)).await.unwrap());
    }
    let results = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        futures_util::future::join_all(readers.iter_mut().map(|reader| async move {
            let mut rows = 0;
            while let Some(output) = reader.next().await.unwrap() {
                if let QueryOutput::Batch(batch) = output {
                    rows += batch.as_record_batch().num_rows();
                }
            }
            assert_eq!(reader.completion().unwrap().rows, 2);
            rows
        })),
    )
    .await
    .expect("four admitted queries must make progress");
    assert_eq!(results, [2, 2, 2, 2]);
    drop(readers);
    drop(announced);
    drop(empty_schema);
    storage.close().await.unwrap();
    runtime.stop().await.unwrap();
}

#[harmos::message]
struct Expansion {
    values: Vec<String>,
}

#[tokio::test]
async fn mapping_exhaustion() {
    // A Compact Repeated Payload Fits Its Encoded Page, But Not Its Expansion.
    let directory = tempfile::tempdir().unwrap();
    let mut settings = storage_settings();
    settings.budget.read_memory_bytes = std::num::NonZeroUsize::new(512 * 1024).unwrap();
    settings.query_execution.memory_bytes = std::num::NonZeroUsize::new(128 * 1024).unwrap();
    settings.query_execution_ceiling.memory_bytes = settings.budget.read_memory_bytes;
    settings.query_decoded_bytes = std::num::NonZeroUsize::new(8 * 1024).unwrap();
    settings.query_decoded_bytes_ceiling = settings.query_decoded_bytes;
    settings.query_page = ReadLimits {
        rows: std::num::NonZeroUsize::MIN,
        bytes: std::num::NonZeroUsize::new(16 * 1024).unwrap(),
    };
    settings.query_page_ceiling = ReadLimits {
        rows: std::num::NonZeroUsize::MIN,
        bytes: std::num::NonZeroUsize::new(64 * 1024).unwrap(),
    };
    let data = Arc::new(Data::<()>::journaled(directory.path(), history()).unwrap());
    let storage = StorageManager::new(
        StorageBackends {
            primary: data,
            state: Arc::new(Recovery::new(history())),
            replicas: vec![],
        },
        settings.clone(),
    )
    .unwrap();
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .runtime_with_storage(&storage)
        .await
        .unwrap();
    let schema = SchemaKey {
        id: Identifier::new("expansion").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = harmos::schema::graph::Schema::from_declaration(
        <Expansion as harmos::schema::Described>::DEFINITION,
        &harmos::config::SchemaSettings::default(),
    )
    .unwrap();
    storage.write_schema(&schema, &graph).await.unwrap();
    let item = Identifier::new("compact").unwrap();
    let payload = Expansion {
        values: vec![String::new(); 4096],
    }
    .encode_to_vec();
    assert!(payload.len() < settings.query_page.bytes.get());
    storage
        .write_records(&[RecordWrite {
            schema: &schema,
            scope: None,
            item: &item,
            source: None,
            source_time: None,
            payload: &payload,
        }])
        .await
        .unwrap();
    let at = storage.flush().await.unwrap();
    let request = |item| Query {
        at: at.clone(),
        source: Source::Records(RecordSelection {
            schema: schema.clone(),
            scope: None,
            item,
            source_start: None,
            source_end: None,
        }),
        transforms: vec![],
        slice: RowSlice {
            offset: 0,
            limit: None,
        },
    };
    let mut reader = storage.query(request(Some(item))).await.unwrap();
    assert!(matches!(
        reader.next().await.unwrap(),
        Some(QueryOutput::Schema(_))
    ));
    let failure = reader.next().await.unwrap_err();
    assert!(
        matches!(failure, harmos::Error::ResourceExhausted { .. }),
        "expanded row must be refused by memory admission: {failure:?}"
    );
    assert!(reader.completion().is_err());
    assert!(storage.status().read_bytes <= settings.budget.read_memory_bytes.get());
    drop(reader);

    // Refusal Leaves Admission Usable And Does Not Publish A Partial Batch.
    let mut empty = storage
        .query(request(Some(Identifier::new("absent").unwrap())))
        .await
        .unwrap();
    assert!(matches!(
        empty.next().await.unwrap(),
        Some(QueryOutput::Schema(_))
    ));
    assert!(empty.next().await.unwrap().is_none());
    assert_eq!(empty.completion().unwrap().rows, 0);
    drop(empty);
    storage.close().await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn mapping_growth() {
    // Conversion Can Grow Beyond Its Initial Reservation Within The Shared Budget.
    let directory = tempfile::tempdir().unwrap();
    let mut settings = storage_settings();
    settings.budget.read_memory_bytes = std::num::NonZeroUsize::new(16 * 1024 * 1024).unwrap();
    settings.query_execution.memory_bytes = std::num::NonZeroUsize::new(128 * 1024).unwrap();
    settings.query_execution_ceiling.memory_bytes = settings.budget.read_memory_bytes;
    settings.query_decoded_bytes = std::num::NonZeroUsize::new(8 * 1024).unwrap();
    settings.query_decoded_bytes_ceiling = settings.query_decoded_bytes;
    settings.query_page = ReadLimits {
        rows: std::num::NonZeroUsize::MIN,
        bytes: std::num::NonZeroUsize::new(16 * 1024).unwrap(),
    };
    settings.query_page_ceiling = ReadLimits {
        rows: std::num::NonZeroUsize::MIN,
        bytes: std::num::NonZeroUsize::new(64 * 1024).unwrap(),
    };
    let data = Arc::new(Data::<()>::journaled(directory.path(), history()).unwrap());
    let storage = StorageManager::new(
        StorageBackends {
            primary: data,
            state: Arc::new(Recovery::new(history())),
            replicas: vec![],
        },
        settings.clone(),
    )
    .unwrap();
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .runtime_with_storage(&storage)
        .await
        .unwrap();
    let schema = SchemaKey {
        id: Identifier::new("expansion").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = harmos::schema::graph::Schema::from_declaration(
        <Expansion as harmos::schema::Described>::DEFINITION,
        &harmos::config::SchemaSettings::default(),
    )
    .unwrap();
    storage.write_schema(&schema, &graph).await.unwrap();
    let item = Identifier::new("compact").unwrap();
    let payload = Expansion {
        values: vec![String::new(); 4096],
    }
    .encode_to_vec();
    assert!(payload.len() < settings.query_page.bytes.get());
    storage
        .write_records(&[RecordWrite {
            schema: &schema,
            scope: None,
            item: &item,
            source: None,
            source_time: None,
            payload: &payload,
        }])
        .await
        .unwrap();
    let at = storage.flush().await.unwrap();
    let request = |item| Query {
        at: at.clone(),
        source: Source::Records(RecordSelection {
            schema: schema.clone(),
            scope: None,
            item,
            source_start: None,
            source_end: None,
        }),
        transforms: vec![],
        slice: RowSlice {
            offset: 0,
            limit: None,
        },
    };
    let mut reader = storage.query(request(Some(item))).await.unwrap();
    assert!(matches!(
        reader.next().await.unwrap(),
        Some(QueryOutput::Schema(_))
    ));
    let Some(QueryOutput::Batch(batch)) = reader.next().await.unwrap() else {
        panic!("expanded data batch");
    };
    assert_eq!(batch.as_record_batch().num_rows(), 1);
    let retained = batch.as_record_batch().clone();
    let payload = retained
        .column_by_name("payload")
        .unwrap()
        .as_any()
        .downcast_ref::<StructArray>()
        .unwrap();
    let values = payload
        .column_by_name("values")
        .unwrap()
        .as_any()
        .downcast_ref::<ListArray>()
        .unwrap()
        .value(0);
    let strings = values
        .as_any()
        .downcast_ref::<arrow_array::StringArray>()
        .unwrap();
    assert_eq!(strings.len(), 4096);
    assert!(strings.iter().all(|value| value == Some("")));
    assert!(reader.next().await.unwrap().is_none());
    assert_eq!(reader.completion().unwrap().rows, 1);
    drop(values);
    drop(batch);
    drop(reader);
    assert!(storage.status().read_bytes > 0);
    assert!(storage.status().read_bytes <= settings.budget.read_memory_bytes.get());
    drop(retained);
    storage.close().await.unwrap();
    runtime.stop().await.unwrap();
}
