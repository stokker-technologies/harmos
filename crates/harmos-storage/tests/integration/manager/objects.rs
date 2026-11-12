/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use super::backend::storage_settings;
// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
// Third Party Imports

// Local Imports
use super::backend::{Data, Recovery, history};
use harmos_domain::{Identified, Identifier, Message};
use harmos_storage::{ObjectRetention, StorageBackends, StorageManager, StoredObject};

/* === Structs === */

#[derive(harmos_domain::Message, Debug, Default, Clone, PartialEq)]
struct Document {
    id: String,
    status: String,
}

#[derive(harmos_domain::Message, Debug, Default, Clone, PartialEq)]
struct DifferentDocument {
    id: String,
    status: String,
    annotation: u32,
}

#[derive(harmos_domain::Message, Clone, Debug, Default)]
struct Duplicate {
    id: String,
    status: String,
}

#[derive(harmos_domain::Message, Debug, Default, Clone, PartialEq)]
struct Counter {
    id: u64,
    value: u64,
}

/* === Enums === */

fn numeric_objects_catalog() -> harmos_domain::model::storage::ObjectCatalog {
    harmos_domain::model::storage::ObjectCatalog::new().entry(harmos_domain::model::storage::ObjectDefinition::of::<Counter>())
}

fn objects_catalog() -> harmos_domain::model::storage::ObjectCatalog {
    harmos_domain::model::storage::ObjectCatalog::new()
        .entry(harmos_domain::model::storage::ObjectDefinition::of::<Document>())
        .entry(harmos_domain::model::storage::ObjectDefinition::of::<DifferentDocument>())
}

fn ambiguous_catalog() -> harmos_domain::model::storage::ObjectCatalog {
    harmos_domain::model::storage::ObjectCatalog::new()
        .entry(harmos_domain::model::storage::ObjectDefinition::of::<Document>())
        .entry(harmos_domain::model::storage::ObjectDefinition::of::<Duplicate>())
}

/* === Implementations === */

impl Identified for Document {
    const ID: &'static str = "app.document";
}

impl StoredObject for Document {
    type Key = String;
    const RETENTION: ObjectRetention = ObjectRetention::Versions(NonZeroUsize::new(64).unwrap());

    fn key(&self) -> &Self::Key {
        &self.id
    }
}

impl Identified for DifferentDocument {
    const ID: &'static str = "app.document";
}

impl StoredObject for DifferentDocument {
    type Key = String;
    const RETENTION: ObjectRetention = ObjectRetention::Versions(NonZeroUsize::new(64).unwrap());

    fn key(&self) -> &Self::Key {
        &self.id
    }
}

impl Identified for Duplicate {
    const ID: &'static str = "app.document";
}

impl StoredObject for Duplicate {
    type Key = String;
    const RETENTION: ObjectRetention = ObjectRetention::Versions(NonZeroUsize::new(64).unwrap());

    fn key(&self) -> &Self::Key {
        &self.id
    }
}

impl Identified for Counter {
    const ID: &'static str = "app.counter";
}

impl StoredObject for Counter {
    type Key = u64;
    const RETENTION: ObjectRetention = ObjectRetention::Latest;

    fn key(&self) -> &Self::Key {
        &self.id
    }
}

/* === Private Functions === */

fn limit() -> NonZeroUsize {
    NonZeroUsize::new(4096).unwrap()
}

fn document(status: &str) -> Document {
    Document {
        id: "document-1".into(),
        status: status.into(),
    }
}

#[tokio::test]
async fn numeric_keys_roundtrip() {
    let primary = Arc::new(Data::with_catalog(numeric_objects_catalog(), history()));
    let storage = StorageManager::with_catalog(
        numeric_objects_catalog(),
        StorageBackends {
            primary,
            state: Arc::new(Recovery::new(history())),
        },
        storage_settings(),
    )
    .unwrap();
    super::fixture::start(&storage).await;
    let object = Counter { id: 42, value: 7 };
    let admitted = storage.write_object(&object).await.unwrap();
    storage.flush_until(&admitted.position).await.unwrap();
    let found = storage.read_object::<Counter>(&42, harmos_storage::ObjectVersion::Exact(admitted.version.clone())).await.unwrap().unwrap();
    assert_eq!(found.value, object);
    assert_eq!(found.version, admitted.version);
    assert!(storage.read_object::<Counter>(&43, harmos_storage::ObjectVersion::Latest,).await.unwrap().is_none());
    storage.close().await.unwrap();
}

#[path = "objects/object_codec_preserves_identity_and_refuses_forged_values.rs"]
mod object_codec_preserves_identity_and_refuses_forged_values;

#[test]
fn ambiguous_catalog_refused() {
    let primary = Arc::new(Data::with_catalog(ambiguous_catalog(), history()));
    let recovery = Arc::new(Recovery::new(history()));
    let result = StorageManager::with_catalog(
        ambiguous_catalog(),
        StorageBackends {
            primary: primary.clone(),
            state: recovery.clone(),
        },
        storage_settings(),
    );
    assert!(
        matches!(&result, Err(harmos_storage::Error::Domain(harmos_domain::Error::InvalidInput { field: "object catalog", .. }))),
        "{:?}",
        result.err()
    );
    assert!(!primary.control.open.load(Ordering::SeqCst));
    assert!(!recovery.control.open.load(Ordering::SeqCst));
}
