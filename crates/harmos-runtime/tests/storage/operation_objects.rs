/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "generated_object_follow.rs"]
mod follow;
#[path = "generated_object_pruned.rs"]
mod pruned;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports
use arrow_array::{StringArray, StructArray};
use arrow_schema::{DataType, Field, Fields};

// Local Imports
use super::super::{Bank, Data, Ledger, Recovery, catalog, config, history};
use super::Lanes;
use harmos_operations::storage::WriteObjects;
use harmos_runtime::storage::{
    MutationId, ObjectCatalog, ObjectRetention, ObjectValue, ObjectVersion, StorageBackends, StorageManager, StoredObject,
};
use harmos_runtime::{Identified, Identifier, Runtime};
use harmos_storage::writes::{DataBatch, WriteReceipt};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_runtime::message]
#[derive(Clone, PartialEq)]
struct Document {
    id: String,
    text: String,
}

/* === Enums === */

#[harmos_domain::catalog]
enum Objects {
    Document(Document),
}

/* === Traits === */

/* === Implementations === */

impl Identified for Document {
    const ID: &'static str = "test.document";
}

impl StoredObject for Document {
    type Key = String;
    const RETENTION: ObjectRetention = ObjectRetention::Versions(NonZeroUsize::new(8).unwrap());
    fn key(&self) -> &String {
        &self.id
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn value(key: &str, text: &str) -> ObjectValue {
    Objects::encode(&Document { id: key.into(), text: text.into() }, NonZeroUsize::new(4096).unwrap()).unwrap()
}

#[tokio::test]
async fn object_exchange_saves() {
    let storage = StorageManager::<Objects>::new(
        StorageBackends {
            primary: Arc::new(Data::<Objects>::with_catalog(history())),
            state: Arc::new(Recovery::new(history())),
        },
        config(),
    )
    .unwrap();
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(16 * 1024 * 1024).unwrap()),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    runtime.operations.register::<harmos_operations::Storage>(Lanes::Open).await.unwrap();
    let first = value("a/b", "first");
    let second = value("second", "two");
    // Register Authored Schema Before Sending Its Typed Payload.
    let document = Document {
        id: "bootstrap".into(),
        text: "schema".into(),
    };
    storage.write_object(&document).await.unwrap();
    storage.flush().await.unwrap();
    let fields: Fields = vec![Field::new("id", DataType::Utf8, false), Field::new("text", DataType::Utf8, false)].into();
    let payload = StructArray::new(
        fields,
        vec![Arc::new(StringArray::from(vec!["a/b", "second"])), Arc::new(StringArray::from(vec!["first", "two"]))],
        None,
    );
    let data = super::writes::envelope(payload, &[first.item.as_str(), second.item.as_str()], &[0, 1]);
    let request = harmos_operations::storage::WriteObjects { scope: None };
    let mut writer = runtime.operations.exchange::<WriteObjects>(request, super::writes::settings(), super::writes::settings()).unwrap();
    writer
        .send(DataBatch {
            sequence: 1,
            destination: first.schema.id.clone(),
            schema: first.schema,
            data: data.into(),
        })
        .await
        .unwrap();
    writer.finish_input();
    let mut accepted = None;
    while let Some(receipt) = writer.next().await.unwrap() {
        match &*receipt {
            WriteReceipt::Accepted(receipt) => accepted = Some(receipt.cursor.clone()),
            WriteReceipt::Durable(receipt) => {
                assert_eq!(accepted.as_ref(), Some(&receipt.cursor))
            }
        }
    }
    let completion = writer.join().await.unwrap();
    assert_eq!(completion.batches, 1);
    assert_eq!(storage.cursor_saved(), completion.cursor);
    let exact = storage.read_object::<Document>(&"a/b".into(), ObjectVersion::Latest).await.unwrap().unwrap();
    assert_eq!(exact.value.text, "first");
    drop(exact);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn object_batch_atomic() {
    let storage = StorageManager::<Objects>::new(
        StorageBackends {
            primary: Arc::new(Data::<Objects>::with_catalog(history())),
            state: Arc::new(Recovery::new(history())),
        },
        config(),
    )
    .unwrap();
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(16 * 1024 * 1024).unwrap()),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    let mutation = MutationId {
        window: storage.mutation_window().unwrap().id,
        key: Identifier::new("atomic").unwrap(),
    };
    let good = value("valid", "value");
    let mut unknown = good.clone();
    unknown.schema.id = Identifier::new("unregistered").unwrap();
    let mut malformed = good.clone();
    malformed.payload = vec![255];
    let mut forged = good.clone();
    forged.item = value("different", "value").item;
    for bad in [unknown, malformed, forged] {
        assert!(storage.write_objects_once(&mutation, vec![good.clone(), bad]).await.is_err());
        assert_eq!(storage.cursor_accepted().sequence, 0);
    }
    assert!(storage.write_objects_once(&mutation, vec![]).await.is_err());
    let admitted = storage.write_objects_once(&mutation, vec![good]).await.unwrap();
    assert_eq!(admitted[0].position.sequence, 2);
    runtime.shutdown().await.unwrap();
}

async fn query_fixture() -> (Runtime<Bank>, StorageManager<Objects>, Arc<Data<Objects>>) {
    let data = Arc::new(Data::<Objects>::with_catalog(history()));
    let storage = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: Arc::new(Recovery::new(history())),
        },
        config(),
    )
    .unwrap();
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(catalog()).operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(16 * 1024 * 1024).unwrap()),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    runtime.operations.register::<harmos_operations::Storage>(Lanes::Open).await.unwrap();
    (runtime, storage, data)
}

/* === Tests === */
