/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "generated_object_exchange.rs"]
mod exchange;

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
use harmos_operations::storage::WriteObjects;
use harmos_protocol::storage::write_receipt::Receipt as WriteReceipt;
use harmos_runtime::storage::{MutationId, ObjectRetention, ObjectValue, ObjectVersion, StorageBackends, StorageManager, StoredObject};
use harmos_runtime::{Identified, Identifier, Runtime};
use harmos_storage::writes::DataBatch;

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

fn objects_catalog() -> harmos_domain::model::storage::ObjectCatalog {
    harmos_domain::model::storage::ObjectCatalog::new().entry(harmos_domain::model::storage::ObjectDefinition::of::<Document>())
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
    objects_catalog().encode(&Document { id: key.into(), text: text.into() }, NonZeroUsize::new(4096).unwrap()).unwrap()
}

#[tokio::test]
async fn object_batch_atomic() {
    let storage = StorageManager::with_catalog(
        objects_catalog(),
        StorageBackends {
            primary: Arc::new(Data::with_catalog(objects_catalog(), history())),
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

async fn query_fixture() -> (Runtime<Bank>, StorageManager, Arc<Data>) {
    let data = Arc::new(Data::with_catalog(objects_catalog(), history()));
    let storage = StorageManager::with_catalog(
        objects_catalog(),
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
    runtime.operations.register_catalog(harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default()).await.unwrap();
    (runtime, storage, data)
}

/* === Tests === */
