/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::super::{Bank, Data, Ledger, Recovery, catalog, config, history};
use harmos_runtime::storage::{MutationId, ObjectRetention, ObjectValue, ObjectVersion, StorageBackends, StorageManager, StoredObject};
use harmos_runtime::{Identified, Identifier};

/* === Structs === */

#[derive(harmos_runtime::Message, Debug, Default, Clone, PartialEq)]
struct Document {
    id: String,
    text: String,
}

/* === Enums === */

fn objects_catalog() -> harmos_domain::model::storage::ObjectCatalog {
    harmos_domain::model::storage::ObjectCatalog::new().entry(harmos_domain::model::storage::ObjectDefinition::of::<Document>())
}

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
    let runtime = crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
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
    crate::support::app::shutdown(&runtime).await.unwrap();
}
