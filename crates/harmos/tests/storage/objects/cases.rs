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
use crate::backend::storage_settings;
// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
// Third Party Imports

// Local Imports
use super::{Bank, Data, Ledger, Recovery, catalog, history};
use harmos::Runtime;
use harmos::storage::{
    ObjectCatalog, ObjectRetention, StorageBackends, StorageManager, StoredObject,
};
use harmos::{DefinitionId, Identified, Identifier, Message};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
#[derive(Clone, PartialEq)]
struct Document {
    id: String,
    status: String,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
struct LegacyDocument {
    id: String,
    status: String,
}

#[harmos::message]
struct Duplicate {
    id: String,
}

#[harmos::message]
struct ZeroVersion {
    id: String,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
struct Counter {
    id: u64,
    value: u64,
}

/* === Enums === */
#[harmos::catalog]
enum NumericObjects {
    Counter(Counter),
}

#[harmos::catalog]
enum Objects {
    Current(Document),
    Historical(LegacyDocument),
}

#[harmos::catalog]
enum RenamedVariants {
    NothingToDoWithWireNames(Document),
    Old(LegacyDocument),
}

#[harmos::catalog]
enum Ambiguous {
    First(Document),
    Second(Duplicate),
}

#[harmos::catalog]
enum Invalid {
    Zero(ZeroVersion),
}

/* === Traits === */

/* === Implementations === */
impl Identified for Document {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("app.document"), 2)
    }
}

impl StoredObject for Document {
    type Key = String;
    const RETENTION: ObjectRetention = ObjectRetention::Versions(NonZeroUsize::new(64).unwrap());

    fn key(&self) -> &Self::Key {
        &self.id
    }
}

impl Identified for LegacyDocument {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("app.document"), 1)
    }
}

impl StoredObject for LegacyDocument {
    type Key = String;
    const RETENTION: ObjectRetention = ObjectRetention::Versions(NonZeroUsize::new(64).unwrap());

    fn key(&self) -> &Self::Key {
        &self.id
    }
}

impl Identified for Duplicate {
    fn definition() -> (DefinitionId, u32) {
        Document::definition()
    }
}

impl StoredObject for Duplicate {
    type Key = String;
    const RETENTION: ObjectRetention = ObjectRetention::Versions(NonZeroUsize::new(64).unwrap());

    fn key(&self) -> &Self::Key {
        &self.id
    }
}

impl Identified for ZeroVersion {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("app.zero"), 0)
    }
}

impl StoredObject for ZeroVersion {
    type Key = String;
    const RETENTION: ObjectRetention = ObjectRetention::Versions(NonZeroUsize::new(64).unwrap());

    fn key(&self) -> &Self::Key {
        &self.id
    }
}

impl Identified for Counter {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("app.counter"), 1)
    }
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

fn own_contract() {
    {
        scenario_runtime::check("numeric_keys_typed", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let primary = Arc::new(Data::<NumericObjects>::with_catalog(history()));
                    let storage = StorageManager::<NumericObjects>::new(
                        StorageBackends {
                            primary,
                            state: Arc::new(Recovery::new(history())),
                        },
                        storage_settings(),
                    )
                    .unwrap();
                    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .runtime_with_storage(&storage)
                        .await
                        .unwrap();
                    let object = Counter { id: 42, value: 7 };
                    let admitted = storage.write_object(&object).await.unwrap();
                    storage.flush_until(&admitted.position).await.unwrap();
                    let found = storage
                        .read_object::<Counter>(
                            &42,
                            harmos::storage::ObjectVersion::Exact(admitted.version.clone()),
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(found.value, object);
                    assert_eq!(found.version, admitted.version);
                    assert!(
                        storage
                            .read_object::<Counter>(&43, harmos::storage::ObjectVersion::Latest,)
                            .await
                            .unwrap()
                            .is_none()
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("catalog_projects_schema", || {
            Objects::validate().unwrap();
            <() as ObjectCatalog>::validate().unwrap();
            assert!(<() as ObjectCatalog>::definitions().as_ref().is_empty());
            let definitions = Objects::definitions();
            assert_eq!(
                definitions
                    .as_ref()
                    .iter()
                    .map(|d| d.schema().unwrap().version.get())
                    .collect::<Vec<_>>(),
                [2, 1]
            );
            assert!(
                (definitions.as_ref()[0].declaration().fields)()
                    .iter()
                    .any(|field| field.name == "status")
            );
            assert!(Ambiguous::validate().is_err());
            assert!(Invalid::validate().is_err());
        });
    }
    {
        scenario_runtime::check("changes_keep_logical", || {
            let first = Objects::encode(&document("running"), limit()).unwrap();
            let second = Objects::encode(&document("finished"), limit()).unwrap();
            assert_eq!(first.schema, second.schema);
            assert_eq!(first.item, second.item);
            assert_ne!(first.payload, second.payload);
            assert_eq!(
                Objects::decode::<Document>(&second, limit()).unwrap(),
                document("finished")
            );
            assert_eq!(
                RenamedVariants::encode(&document("finished"), limit()).unwrap(),
                second
            );
            Objects::validate_value(&second, limit()).unwrap();
        });
    }
    {
        scenario_runtime::check("schemas_require_typed", || {
            let old = LegacyDocument {
                id: "document-1".into(),
                status: "archived".into(),
            };
            let value = Objects::encode(&old, limit()).unwrap();
            Objects::validate_value(&value, limit()).unwrap();
            assert_eq!(
                Objects::decode::<LegacyDocument>(&value, limit()).unwrap(),
                old
            );
            assert!(Objects::decode::<Document>(&value, limit()).is_err());
            let mut unknown = value;
            unknown.schema.version = std::num::NonZeroU32::new(3).unwrap();
            assert!(Objects::validate_value(&unknown, limit()).is_err());
        });
    }
    {
        scenario_runtime::check("malformed_forged_object", || {
            let original = Objects::encode(&document("running"), limit()).unwrap();
            for case in 0..4 {
                let mut value = original.clone();
                match case {
                    0 => {
                        value.item = harmos::storage::ObjectKey::new(
                            "other",
                            NonZeroUsize::new(4096).unwrap(),
                        )
                        .unwrap()
                    }
                    1 => value.schema.id = Identifier::new("other").unwrap(),
                    2 => value.payload = vec![0xff],
                    _ => value.payload = document("finished").encode_to_vec(),
                }
                if case == 3 {
                    // A value update with the same logical identity is valid.
                    Objects::validate_value(&value, limit()).unwrap();
                } else {
                    assert!(Objects::validate_value(&value, limit()).is_err());
                    assert!(Objects::decode::<Document>(&value, limit()).is_err());
                }
            }
            let invalid = Document {
                id: String::new(),
                status: "new".into(),
            };
            assert!(Objects::encode(&invalid, limit()).is_err());
        });
    }
    {
        scenario_runtime::check("payload_bounds_decoding", || {
            let source = document(&"x".repeat(100));
            let exact = NonZeroUsize::new(source.encoded_len()).unwrap();
            let small = NonZeroUsize::new(source.encoded_len() - 1).unwrap();
            assert!(Objects::encode(&source, small).is_err());
            let value = Objects::encode(&source, exact).unwrap();
            assert!(Objects::decode::<Document>(&value, small).is_err());
            assert!(Objects::validate_value(&value, small).is_err());
            assert_eq!(Objects::decode::<Document>(&value, exact).unwrap(), source);
        });
    }
    {
        scenario_runtime::check("invalid_object_catalog", || {
            let primary = Arc::new(Data::<Ambiguous>::with_catalog(history()));
            let recovery = Arc::new(Recovery::new(history()));
            let result = StorageManager::<Ambiguous>::new(
                StorageBackends {
                    primary: primary.clone(),
                    state: recovery.clone(),
                },
                storage_settings(),
            );
            assert!(matches!(
                result,
                Err(harmos::Error::InvalidInput {
                    field: "object catalog",
                    ..
                })
            ));
            assert!(!primary.control.open.load(Ordering::SeqCst));
            assert!(!recovery.control.open.load(Ordering::SeqCst));
        });
    }
}

/* === Tests === */

#[test]
fn object_lifetime() {
    own_contract();
}
