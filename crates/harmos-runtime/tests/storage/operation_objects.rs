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
use harmos_runtime::storage::{MutationId, ObjectRetention, ObjectValue, StorageBackends, StorageManager, StoredObject};
use harmos_runtime::{Identified, Identifier, Runtime};

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

#[tokio::test]
async fn native_object_exchange_preserves_conflicts_replay_and_retained_reads() {
    use harmos_operations::storage::{GetObject, GetObjectRequest, SchemaInput, SetObject, SetObjectRequest, set_object_response::Outcome};
    use harmos_runtime::schema::{Described, graph::Schema};

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
        setup.settings().storage(&storage).register(catalog());
    })
    .await
    .unwrap();
    crate::support::app::register(&runtime, harmos_operations::storage_catalog(), Default::default()).await.unwrap();
    let first = value("document", "first");
    let request = SetObjectRequest {
        mutation: Some(MutationId {
            window: storage.mutation_window().unwrap().id,
            key: Identifier::new("create").unwrap(),
        }),
        expected_revision: vec![],
        create_if_absent: true,
        schema: Some(SchemaInput {
            key: Some(first.schema.clone()),
            graph: Some(Schema::from_declaration(<Document as Described>::DEFINITION, &Default::default()).unwrap()),
        }),
        value: Some(first.clone()),
    };
    let created = runtime.operations.call_native::<SetObject>(request.clone()).await.unwrap();
    let Some(Outcome::Applied(applied)) = &created.outcome else {
        panic!("Expected Applied");
    };
    assert_eq!(applied.value.as_ref(), Some(&first));
    assert!(applied.receipt.as_ref().unwrap().completed);
    let get = GetObjectRequest {
        schema: Some(first.schema.clone()),
        item: first.item.to_string(),
        at: None,
    };
    let old = runtime.operations.call_native::<GetObject>(get.clone()).await.unwrap();
    assert!(old.exists);
    assert_eq!(old.value, Some(first));
    assert_eq!(old.revision, applied.revision);

    let second = value("document", "second");
    let mut update = request.clone();
    update.mutation.as_mut().unwrap().key = Identifier::new("update").unwrap();
    update.expected_revision = old.revision.clone();
    update.create_if_absent = false;
    update.value = Some(second.clone());
    let updated = runtime.operations.call_native::<SetObject>(update.clone()).await.unwrap();
    let Some(Outcome::Applied(current)) = &updated.outcome else {
        panic!("Expected Applied");
    };
    assert_ne!(current.revision, old.revision);
    assert_eq!(current.value.as_ref(), Some(&second));
    assert_eq!(runtime.operations.call_native::<SetObject>(update.clone()).await.unwrap(), updated);
    assert_eq!(runtime.operations.call_native::<SetObject>(request).await.unwrap(), created);

    update.mutation.as_mut().unwrap().key = Identifier::new("stale").unwrap();
    let conflict = runtime.operations.call_native::<SetObject>(update).await.unwrap();
    let Some(Outcome::Conflict(conflict)) = conflict.outcome else {
        panic!("Expected Conflict");
    };
    assert_eq!(conflict.current_revision, current.revision);
    let retained = runtime.operations.call_native::<GetObject>(GetObjectRequest { at: old.resolved_at, ..get.clone() }).await.unwrap();
    assert_eq!(retained.value, old.value);
    assert_eq!(retained.revision, old.revision);
    assert_eq!(runtime.operations.call_native::<GetObject>(get).await.unwrap().value, Some(second));
    crate::support::app::shutdown(&runtime).await.unwrap();
}
