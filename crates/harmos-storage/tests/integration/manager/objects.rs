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

use super::backend::storage_settings;
// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
// Third Party Imports

// Local Imports
use super::backend::{Data, Recovery, history};
use harmos_domain::{Identified, Identifier, Message};
use harmos_storage::{ObjectCatalog, ObjectRetention, StorageBackends, StorageManager, StoredObject};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_domain::message]
#[derive(Clone, PartialEq)]
struct Document {
    id: String,
    status: String,
}

#[harmos_domain::message]
#[derive(Clone, PartialEq)]
struct DifferentDocument {
    id: String,
    status: String,
    annotation: u32,
}

#[harmos_domain::message]
struct Duplicate {
    id: String,
    status: String,
}

#[harmos_domain::message]
#[derive(Clone, PartialEq)]
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

fn renamed_variants_catalog() -> harmos_domain::model::storage::ObjectCatalog {
    harmos_domain::model::storage::ObjectCatalog::new()
        .entry(harmos_domain::model::storage::ObjectDefinition::of::<Document>())
        .entry(harmos_domain::model::storage::ObjectDefinition::of::<DifferentDocument>())
}

fn ambiguous_catalog() -> harmos_domain::model::storage::ObjectCatalog {
    harmos_domain::model::storage::ObjectCatalog::new()
        .entry(harmos_domain::model::storage::ObjectDefinition::of::<Document>())
        .entry(harmos_domain::model::storage::ObjectDefinition::of::<Duplicate>())
}

/* === Traits === */

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

/* === Public Functions === */

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

#[test]
fn catalog_projects_schema() {
    objects_catalog().validate().unwrap();
    ObjectCatalog::new().validate().unwrap();
    assert!(ObjectCatalog::new().definitions().is_empty());
    let catalog = objects_catalog();
    let definitions = catalog.definitions();
    assert_eq!(
        definitions.iter().map(|d| d.schema().unwrap().version.get()).collect::<Vec<_>>(),
        [Document::definition().1, DifferentDocument::definition().1]
    );
    assert!((definitions[0].declaration().fields)().iter().any(|field| field.name == "status"));
    assert!(ambiguous_catalog().validate().is_err());
}

#[test]
fn updates_keep_identity() {
    let first = objects_catalog().encode(&document("running"), limit()).unwrap();
    let second = objects_catalog().encode(&document("finished"), limit()).unwrap();
    assert_eq!(first.schema, second.schema);
    assert_eq!(first.item, second.item);
    assert_ne!(first.payload, second.payload);
    assert_eq!(objects_catalog().decode::<Document>(&second, limit()).unwrap(), document("finished"));
    assert_eq!(renamed_variants_catalog().encode(&document("finished"), limit()).unwrap(), second);
    objects_catalog().validate_value(&second, limit()).unwrap();
}

#[test]
fn decode_requires_matching_shape() {
    let old = DifferentDocument {
        id: "document-1".into(),
        status: "archived".into(),
        annotation: 7,
    };
    let value = objects_catalog().encode(&old, limit()).unwrap();
    objects_catalog().validate_value(&value, limit()).unwrap();
    assert_eq!(objects_catalog().decode::<DifferentDocument>(&value, limit()).unwrap(), old);
    assert!(objects_catalog().decode::<Document>(&value, limit()).is_err());
    let mut unknown = value;
    unknown.schema.version = std::num::NonZeroU32::new(unknown.schema.version.get().wrapping_add(1).max(1)).unwrap();
    assert!(objects_catalog().validate_value(&unknown, limit()).is_err());
}

#[test]
fn forged_values_refused() {
    let original = objects_catalog().encode(&document("running"), limit()).unwrap();
    for case in 0..4 {
        let mut value = original.clone();
        match case {
            0 => value.item = harmos_storage::ObjectKey::new("other", NonZeroUsize::new(4096).unwrap()).unwrap(),
            1 => value.schema.id = Identifier::new("other").unwrap(),
            2 => value.payload = vec![0xff],
            _ => value.payload = document("finished").encode(),
        }
        if case == 3 {
            // A value update with the same logical identity is valid.
            objects_catalog().validate_value(&value, limit()).unwrap();
        } else {
            assert!(objects_catalog().validate_value(&value, limit()).is_err());
            assert!(objects_catalog().decode::<Document>(&value, limit()).is_err());
        }
    }
    let invalid = Document { id: String::new(), status: "new".into() };
    assert!(objects_catalog().encode(&invalid, limit()).is_err());
}

#[test]
fn payload_limit_bounds() {
    let source = document(&"x".repeat(100));
    let exact = NonZeroUsize::new(source.encoded_size()).unwrap();
    let small = NonZeroUsize::new(source.encoded_size() - 1).unwrap();
    assert!(objects_catalog().encode(&source, small).is_err());
    let value = objects_catalog().encode(&source, exact).unwrap();
    assert!(objects_catalog().decode::<Document>(&value, small).is_err());
    assert!(objects_catalog().validate_value(&value, small).is_err());
    assert_eq!(objects_catalog().decode::<Document>(&value, exact).unwrap(), source);
}

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

/* === Tests === */
