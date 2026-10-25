/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The Host Alone: Boot, Serve Its Catalog, Persist Records, Shut Down

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;
use std::sync::atomic::Ordering;

// Third Party Imports
use arrow_array::{Array, Int64Array, StringArray, StructArray};

// Local Imports
use super::fixture::{
    Bank, Data, Identifier, Ledger, Recovery, Runtime, SchemaKey, StorageBackends, StorageManager, catalog, config, history, record, schema,
};
use super::operations::Lanes;
use harmos_runtime::Message;
use harmos_runtime::config::SchemaSettings;
use harmos_runtime::schema::{Described, graph::Schema as Graph};
use harmos_runtime::storage::operations::Query;
use harmos_runtime::storage::query::{QueryArrow, RecordSelection, RowSlice, Source};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_runtime::message]
struct Reading {
    value: i64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

// Every Saved Record Of One Schema As (Item, Value), Through The Query Path
async fn saved(storage: &StorageManager, schema: &SchemaKey) -> Vec<(String, i64)> {
    let query = Query {
        at: storage.cursor_saved(),
        source: Source::Records(RecordSelection { schema: schema.clone(), scope: None, item: None, source_start: None, source_end: None }),
        transforms: Vec::new(),
        slice: RowSlice::default(),
    };
    let mut reader = storage.query(query).await.unwrap();
    let mut rows = Vec::new();
    while let Some(output) = reader.next().await.unwrap() {
        let QueryArrow::Batch(batch) = output else {
            continue;
        };
        let batch = batch.as_record_batch();
        let column = |owner: &str, name: &str| {
            let owner = batch.column_by_name(owner).unwrap();
            let owner = owner.as_any().downcast_ref::<StructArray>().unwrap();
            owner.column_by_name(name).unwrap().clone()
        };
        let items = column("_storage", "item");
        let items = items.as_any().downcast_ref::<StringArray>().unwrap();
        let values = column("payload", "value");
        let values = values.as_any().downcast_ref::<Int64Array>().unwrap();
        for row in 0..batch.num_rows() {
            rows.push((items.value(row).to_owned(), values.value(row)));
        }
    }

    rows
}

fn served(runtime: &Runtime<Bank>) -> Vec<String> {
    runtime.operations.definitions().into_iter().map(|definition| definition.id.to_string()).collect()
}

/* === Tests === */

#[tokio::test]
async fn runtime_baseline_walks() {
    let _report = crate::report::system();
    // Boot The Host Over A Journal On Disk
    let root = tempfile::tempdir().unwrap();
    let data = Arc::new(Data::<()>::journaled(root.path(), history()).unwrap());
    let recovery = Arc::new(Recovery::new(history()));
    let backends = StorageBackends { primary: data.clone(), state: recovery.clone() };
    let storage: StorageManager = StorageManager::new(backends, config()).unwrap();
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(Lanes::catalog())
        .runtime_with_storage(&storage)
        .await
        .unwrap();
    assert!(storage.status().running);

    // Read The Catalog Before And After Serving Storage
    assert!(!served(&runtime).iter().any(|id| id.starts_with("storage.")), "nothing is served before the host registers it");
    harmos_runtime::storage::operations::register(&runtime.operations, "storage", Lanes::Open, Arc::new(storage.clone())).unwrap();
    let catalogue = served(&runtime);
    for required in ["storage.query", "storage.write_records", "storage.read_file"] {
        assert!(catalogue.iter().any(|id| id == required), "missing {required}");
    }

    // Declare A Schema, Write Records Of It And Read Them Back Once Saved
    let schema = schema();
    let declaration = <Reading as Described>::DEFINITION;
    let graph = Graph::from_declaration(declaration, &SchemaSettings::default()).unwrap();
    storage.write_schema(&schema, &graph).await.unwrap();
    let first = Identifier::new("first").unwrap();
    let second = Identifier::new("second").unwrap();
    let low = Message::encode_to_vec(&Reading { value: -3 });
    let high = Message::encode_to_vec(&Reading { value: 8 });
    let records = [record(&schema, &first, &low), record(&schema, &second, &high)];
    let written = storage.write_records(&records).await.unwrap();
    storage.flush().await.unwrap();
    assert!(storage.cursor_saved().sequence >= written.sequence);
    let saved = saved(&storage, &schema).await;
    assert_eq!(saved, [("first".to_owned(), -3), ("second".to_owned(), 8)]);

    // Shut Down; The Runtime Closes The Storage It Owns
    runtime.stop().await.unwrap();
    assert!(!storage.status().running);
    assert_eq!(data.control.closes.load(Ordering::SeqCst), 1);
    assert_eq!(recovery.control.closes.load(Ordering::SeqCst), 1);
}
