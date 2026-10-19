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

// Standard Library Imports
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;

// Third Party Imports
use arrow_array::{Array, Float64Array, ListArray, StructArray, UInt32Array};
use harmos_domain::operations::storage::Query;
use harmos_storage::query::{QueryArrow, RecordSelection, RowSlice, Source};
use harmos_storage::*;
use harmos_storage::{Identifier, Message};

// Local Imports
use super::backend::{Data, history, storage_settings};
use super::fixture::state_settings;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_domain::message]
struct Payload {
    label: Option<String>,
    values: Vec<f64>,
    event: Event,
}

#[harmos_domain::message]
struct Expansion {
    values: Vec<String>,
}

// Journaled Storage Holding Saved Records Of One Declared Schema.
struct Mapped {
    _directory: tempfile::TempDir,
    storage: StorageManager,
    schema: SchemaKey,
    at: StorageCursor,
}

/* === Enums === */

#[harmos_domain::message]
enum Event {
    Reading(i64),
    Empty,
}

/* === Traits === */

/* === Implementations === */

impl Mapped {
    async fn new<T: harmos_domain::schema::Described + Message>(settings: StorageSettings, id: &str, payloads: &[T]) -> Self {
        // Start Storage Over A Real Journal And File State Root.
        let directory = tempfile::tempdir().unwrap();
        let data = Data::<()>::journaled(directory.path(), history()).unwrap();
        let state_root = directory.path().join("state");
        let state = harmos_storage::state::file::FileBackend::new(state_root, history(), state_settings());
        let backends = StorageBackends { primary: Arc::new(data), state: Arc::new(state) };
        let storage = StorageManager::new(backends, settings).unwrap();
        super::fixture::start(&storage).await;

        // Declare The Schema And Save One Record Per Payload.
        let schema =
            SchemaKey { id: Identifier::new(id).unwrap(), version: NonZeroU32::MIN, payload_encoding: PayloadEncoding::PostcardV1 };
        let graph =
            harmos_domain::schema::graph::Schema::from_declaration(T::DEFINITION, &harmos_domain::config::SchemaSettings::default())
                .unwrap();
        storage.write_schema(&schema, &graph).await.unwrap();
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
        Self { _directory: directory, storage, schema, at }
    }

    fn query(&self, item: Option<&str>) -> Query {
        let item = item.map(|item| {
            let identifier = Identifier::new(item);
            identifier.unwrap()
        });
        Query {
            at: self.at.clone(),
            source: Source::Records(RecordSelection {
                schema: self.schema.clone(),
                scope: None,
                item,
                source_start: None,
                source_end: None,
            }),
            transforms: vec![],
            slice: RowSlice { offset: 0, limit: None },
        }
    }

    async fn finish(self) {
        self.storage.close().await.unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn payloads() -> [Payload; 2] {
    [
        Payload { label: None, values: vec![0.0, -1.0, 2.0], event: Event::Reading(-7) },
        Payload { label: Some("second".into()), values: vec![], event: Event::Empty },
    ]
}

fn compact() -> [Expansion; 1] {
    [Expansion { values: vec![String::new(); 4096] }]
}

/* === Tests === */

#[tokio::test]
async fn selection_preserves_schema() {
    let m = Mapped::new(storage_settings(), "mapping", &payloads()).await;
    let mut reader = m.storage.query(m.query(None)).await.unwrap();
    let Some(QueryArrow::Schema(announced)) = reader.next().await.unwrap() else {
        panic!("schema first");
    };
    let Some(QueryArrow::Batch(batch)) = reader.next().await.unwrap() else {
        panic!("data batch");
    };

    // Optional, Repeated And Enum Fields Map To Nullable, List And Variant Columns.
    let payload = batch.as_record_batch().column_by_name("payload").unwrap().as_any().downcast_ref::<StructArray>().unwrap();
    assert!(payload.column_by_name("label").unwrap().is_null(0));
    let values = payload.column_by_name("values").unwrap().as_any().downcast_ref::<ListArray>().unwrap();
    let first = values.value(0);
    assert_eq!(first.as_any().downcast_ref::<Float64Array>().unwrap().values().as_ref(), &[0.0, -1.0, 2.0]);
    let event = payload.column_by_name("event").unwrap().as_any().downcast_ref::<StructArray>().unwrap();
    let discriminants = event.column_by_name("__variant").unwrap().as_any().downcast_ref::<UInt32Array>().unwrap();
    assert_eq!(discriminants.values().as_ref(), &[0, 1]);
    assert!(!event.column(1).is_null(0));
    assert!(event.column(1).is_null(1));
    assert!(event.column(2).is_null(0));
    assert!(!event.column(2).is_null(1));
    drop(batch);
    assert!(reader.next().await.unwrap().is_none());
    assert_eq!(reader.completion().unwrap().rows, 2);
    drop(reader);

    // A Selection Without Rows Announces The Same Schema And Completes Empty.
    let query = m.query(Some("absent"));
    let mut empty = m.storage.query(query).await.unwrap();
    let Some(QueryArrow::Schema(empty_schema)) = empty.next().await.unwrap() else {
        panic!("empty schema");
    };
    assert_eq!(announced.as_schema(), empty_schema.as_schema());
    assert!(empty.next().await.unwrap().is_none());
    assert_eq!(empty.completion().unwrap().rows, 0);
    drop(empty);
    drop(announced);
    drop(empty_schema);
    m.finish().await;
}

#[tokio::test]
async fn admitted_queries_progress() {
    // Four Admitted Queries Progress While Their Schema Output Is Retained.
    let m = Mapped::new(storage_settings(), "mapping", &payloads()).await;
    let mut readers = Vec::new();
    for _ in 0..4 {
        let query = m.query(None);
        let reader = m.storage.query(query).await.unwrap();
        readers.push(reader);
    }
    let queries = readers.iter_mut().map(|reader| async move {
        let mut rows = 0;
        while let Some(output) = reader.next().await.unwrap() {
            if let QueryArrow::Batch(batch) = output {
                rows += batch.as_record_batch().num_rows();
            }
        }
        assert_eq!(reader.completion().unwrap().rows, 2);
        rows
    });
    let results = futures_util::future::join_all(queries).await;
    assert_eq!(results, [2, 2, 2, 2]);
    drop(readers);
    m.finish().await;
}

#[tokio::test]
async fn expansion_exceeds_budget() {
    // A Compact Repeated Payload Fits Its Encoded Page, But Not Its Expansion.
    let mut settings = storage_settings();
    settings.budget.read_memory_bytes = NonZeroUsize::new(512 * 1024).unwrap();
    let payload = compact();
    assert!(Message::encode_to_vec(&payload[0]).len() < 16 * 1024);
    let m = Mapped::new(settings.clone(), "expansion", &payload).await;
    let query = m.query(Some("row-0"));
    let mut reader = m.storage.query(query).await.unwrap();
    assert!(matches!(reader.next().await.unwrap(), Some(QueryArrow::Schema(_))));
    let failure = reader.next().await.unwrap_err();
    assert!(
        matches!(failure, harmos_storage::Error::ResourceExhausted { .. }),
        "expanded row must be refused by memory admission: {failure:?}"
    );
    assert!(reader.completion().is_err());
    assert!(m.storage.status().read_bytes <= settings.budget.read_memory_bytes.get());
    drop(reader);

    // Refusal Leaves Admission Usable And Does Not Publish A Partial Batch.
    let query = m.query(Some("absent"));
    let mut empty = m.storage.query(query).await.unwrap();
    assert!(matches!(empty.next().await.unwrap(), Some(QueryArrow::Schema(_))));
    assert!(empty.next().await.unwrap().is_none());
    assert_eq!(empty.completion().unwrap().rows, 0);
    drop(empty);
    m.finish().await;
}

#[tokio::test]
async fn conversion_grows_reservation() {
    // Conversion Can Grow Beyond Its Initial Reservation Within The Shared Budget.
    let settings = storage_settings();
    let m = Mapped::new(settings.clone(), "expansion", &compact()).await;
    let query = m.query(Some("row-0"));
    let mut reader = m.storage.query(query).await.unwrap();
    assert!(matches!(reader.next().await.unwrap(), Some(QueryArrow::Schema(_))));
    let Some(QueryArrow::Batch(batch)) = reader.next().await.unwrap() else {
        panic!("expanded data batch");
    };
    assert_eq!(batch.as_record_batch().num_rows(), 1);
    let retained = batch.as_record_batch().clone();
    let payload = retained.column_by_name("payload").unwrap().as_any().downcast_ref::<StructArray>().unwrap();
    let values = payload.column_by_name("values").unwrap().as_any().downcast_ref::<ListArray>().unwrap().value(0);
    let strings = values.as_any().downcast_ref::<arrow_array::StringArray>().unwrap();
    assert_eq!(strings.len(), 4096);
    assert!(strings.iter().all(|value| value == Some("")));
    assert!(reader.next().await.unwrap().is_none());
    assert_eq!(reader.completion().unwrap().rows, 1);

    // Zero-Copy Clones And Zero-Length Slices Still Own The Charged Allocation.
    let empty = retained.column(0).slice(retained.num_rows(), 0);
    drop(values);
    drop(batch);
    drop(reader);
    assert!(m.storage.status().read_bytes > 0);
    assert!(m.storage.status().read_bytes <= settings.budget.read_memory_bytes.get());
    drop(retained);
    assert!(m.storage.status().read_bytes > 0);
    drop(empty);
    m.finish().await;
}
