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

#[path = "query_objects.rs"]
mod fields;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::super::{Bank, Data, Ledger, Recovery, catalog, config, history};
use super::Lanes;
use harmos::storage::operations::{Operations, WriteObjects, WriteObjectsRequest};
use harmos::storage::{
    MutationId, ObjectCatalog, ObjectRetention, ObjectValue, ObjectVersion, StorageBackends,
    StorageManager, StoredObject,
};
use harmos::{DefinitionId, Identified, Identifier, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
#[derive(Clone, PartialEq)]
struct Document {
    id: String,
    text: String,
}

#[harmos::message]
struct DocumentV2 {
    id: String,
    text: String,
}

/* === Enums === */
#[harmos::catalog]
enum Objects {
    Document(Document),
    V2(DocumentV2),
}

/* === Traits === */

/* === Implementations === */
impl Identified for Document {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("test.document"), 1)
    }
}
impl StoredObject for Document {
    type Key = String;
    const RETENTION: ObjectRetention = ObjectRetention::Versions(NonZeroUsize::new(8).unwrap());
    fn key(&self) -> &String {
        &self.id
    }
}

impl Identified for DocumentV2 {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("test.document"), 2)
    }
}
impl StoredObject for DocumentV2 {
    type Key = String;
    const RETENTION: ObjectRetention = Document::RETENTION;
    fn key(&self) -> &String {
        &self.id
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn value(key: &str, text: &str) -> ObjectValue {
    Objects::encode(
        &Document {
            id: key.into(),
            text: text.into(),
        },
        NonZeroUsize::new(4096).unwrap(),
    )
    .unwrap()
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("object_writes_preserve", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let storage = StorageManager::new(
                        StorageBackends {
                            primary: Arc::new(Data::<Objects>::with_catalog(history())),
                            recovery: Arc::new(Recovery::new(history())),
                            replicas: Vec::new(),
                        },
                        config(),
                    )
                    .unwrap();
                    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .register(Lanes::catalog())
                        .runtime_with_storage(&storage)
                        .await
                        .unwrap();
                    Operations::register(
                        &runtime.operations,
                        "storage",
                        Lanes::Open,
                        Arc::new(storage.clone()),
                    )
                    .unwrap();
                    let mutation = MutationId {
                        window: storage.mutation_window().unwrap().id,
                        key: Identifier::new("group").unwrap(),
                    };
                    let request = WriteObjectsRequest {
                        mutation: mutation.clone(),
                        objects: vec![value("a/b", "first"), value("second", "two")],
                    };
                    let response = runtime
                        .operations
                        .call::<WriteObjects>(request.clone())
                        .await
                        .unwrap();
                    assert_eq!(response.objects.len(), 2);
                    assert_eq!(response.objects[0].position.sequence, 2);
                    assert_eq!(response.objects[1].position.sequence, 4);
                    assert_eq!(storage.cursor_saved().sequence, 0);
                    let native = storage
                        .write_object(&Document {
                            id: "a/b".into(),
                            text: "later".into(),
                        })
                        .await
                        .unwrap();
                    assert_eq!(native.position.sequence, 6);
                    let repeated = storage
                        .write_objects_once(&mutation, &request.objects)
                        .await
                        .unwrap();
                    assert_eq!(repeated, response.objects);
                    assert_eq!(storage.cursor_accepted(), native.position);
                    let mut conflict = request.clone();
                    conflict.objects[0] = value("a/b", "conflict");
                    assert!(
                        runtime
                            .operations
                            .call::<WriteObjects>(conflict)
                            .await
                            .is_err()
                    );
                    storage.flush().await.unwrap();
                    let exact = storage
                        .read_object::<Document>(
                            &"a/b".into(),
                            ObjectVersion::Exact(response.objects[0].version.clone()),
                            NonZeroUsize::new(4096).unwrap(),
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(exact.value.text, "first");
                    assert_eq!(
                        runtime
                            .operations
                            .call::<WriteObjects>(request)
                            .await
                            .unwrap()
                            .objects,
                        response.objects
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("object_writes_reject", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let storage = StorageManager::new(
                        StorageBackends {
                            primary: Arc::new(Data::<Objects>::with_catalog(history())),
                            recovery: Arc::new(Recovery::new(history())),
                            replicas: Vec::new(),
                        },
                        config(),
                    )
                    .unwrap();
                    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .register(Lanes::catalog())
                        .runtime_with_storage(&storage)
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
                        assert!(
                            storage
                                .write_objects_once(&mutation, &[good.clone(), bad])
                                .await
                                .is_err()
                        );
                        assert_eq!(storage.cursor_accepted().sequence, 0);
                    }
                    assert!(storage.write_objects_once(&mutation, &[]).await.is_err());
                    let admitted = storage
                        .write_objects_once(&mutation, &[good])
                        .await
                        .unwrap();
                    assert_eq!(admitted[0].position.sequence, 2);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("shared_object_receipts", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use std::sync::atomic::Ordering;
                    let data = Arc::new(Data::<Objects>::with_catalog(history()));
                    let storage = StorageManager::new(
                        StorageBackends {
                            primary: data.clone(),
                            recovery: Arc::new(Recovery::new(history())),
                            replicas: Vec::new(),
                        },
                        config(),
                    )
                    .unwrap();
                    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .runtime_with_storage(&storage)
                        .await
                        .unwrap();
                    let saved = MutationId {
                        window: storage.mutation_window().unwrap().id,
                        key: Identifier::new("saved").unwrap(),
                    };
                    let lost = MutationId {
                        key: Identifier::new("unsaved").unwrap(),
                        ..saved.clone()
                    };
                    let objects = vec![value("first", "saved"), value("second", "saved")];
                    let receipts = storage.write_objects_once(&saved, &objects).await.unwrap();
                    let save = MutationId {
                        key: Identifier::new("save").unwrap(),
                        ..saved.clone()
                    };
                    storage
                        .save_once(&save, &receipts[1].position)
                        .await
                        .unwrap();
                    storage
                        .write_objects_once(&lost, &[value("third", "lost")])
                        .await
                        .unwrap();
                    // Reconstruct Only The Backend's Durable Prefix, Without Graceful Close.
                    let recovered = Arc::new(Data::<Objects>::with_catalog(history()));
                    let frontier = data.saved.load(Ordering::SeqCst);
                    recovered.saved.store(frontier, Ordering::SeqCst);
                    *recovered.schemas.lock().unwrap() = data
                        .schemas
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|(position, _)| **position <= frontier as u64)
                        .map(|(position, value)| (*position, value.clone()))
                        .collect();
                    *recovered.objects.lock().unwrap() = data
                        .objects
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|(position, _)| **position <= frontier as u64)
                        .map(|(position, value)| (*position, value.clone()))
                        .collect();
                    *recovered.mutation_receipts.lock().unwrap() =
                        data.mutation_receipts.lock().unwrap().clone();
                    let restored = StorageManager::new(
                        StorageBackends {
                            primary: recovered,
                            recovery: Arc::new(Recovery::new(history())),
                            replicas: Vec::new(),
                        },
                        config(),
                    )
                    .unwrap();
                    let restarted = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .runtime_with_storage(&restored)
                        .await
                        .unwrap();
                    assert_ne!(restored.mutation_window().unwrap().id, saved.window);
                    assert_eq!(
                        restored.write_objects_once(&saved, &objects).await.unwrap(),
                        receipts
                    );
                    assert!(
                        restored
                            .write_objects_once(&lost, &[value("third", "lost")])
                            .await
                            .is_err()
                    );
                    assert_eq!(restored.cursor_accepted(), receipts[1].position);
                    restarted.stop().await.unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("latest_page_revisions", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::storage::{ObjectVersions, operations::ObjectSelection};
                    let (runtime, storage, _) = query_fixture().await;
                    let old = storage
                        .write_object(&Document {
                            id: "path/key".into(),
                            text: "old".into(),
                        })
                        .await
                        .unwrap();
                    storage
                        .write_object(&Document {
                            id: "other".into(),
                            text: "other".into(),
                        })
                        .await
                        .unwrap();
                    let latest = storage
                        .write_object(&Document {
                            id: "path/key".into(),
                            text: "new".into(),
                        })
                        .await
                        .unwrap();
                    assert!(
                        query_objects(&runtime, ObjectSelection::default())
                            .await
                            .is_empty()
                    );
                    storage.flush().await.unwrap();
                    let mut request = ObjectSelection::default();
                    request.selection.page_rows = 1;
                    let values = query_objects(&runtime, request.clone()).await;
                    assert_eq!(values.len(), 2);
                    assert_eq!(values[0].value.item.as_str(), "other");
                    assert_eq!(values[1].version, latest.version);
                    request.schema = Some(value("path/key", "").schema);
                    request.key = Some(value("path/key", "").item);
                    request.versions = ObjectVersions::Revisions;
                    request.selection.descending = true;
                    let values = query_objects(&runtime, request.clone()).await;
                    assert_eq!(
                        values
                            .iter()
                            .map(|value| value.version.clone())
                            .collect::<Vec<_>>(),
                        vec![latest.version.clone(), old.version.clone()]
                    );
                    request.selection.through = Some(old.position.sequence);
                    request.versions = ObjectVersions::Latest;
                    let values = query_objects(&runtime, request).await;
                    assert_eq!(values.len(), 1);
                    assert_eq!(values[0].version, old.version);
                    let upgraded = storage
                        .write_object(&DocumentV2 {
                            id: "path/key".into(),
                            text: "version two".into(),
                        })
                        .await
                        .unwrap();
                    storage.flush().await.unwrap();
                    let request = ObjectSelection {
                        schema: Some(value("path/key", "").schema),
                        key: Some(value("path/key", "").item),
                        ..Default::default()
                    };
                    assert!(query_objects(&runtime, request).await.is_empty());
                    let request = ObjectSelection {
                        key: Some(value("path/key", "").item),
                        ..Default::default()
                    };
                    let values = query_objects(&runtime, request).await;
                    assert_eq!(values.len(), 1);
                    assert_eq!(values[0].version, upgraded.version);
                    assert_eq!(values[0].value.schema.version.get(), 2);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("follow_resumes_saved", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::storage::{
                        ObjectEntry, ObjectVersions,
                        operations::{FollowObjects, ObjectSelection},
                    };
                    tokio::time::timeout(std::time::Duration::from_secs(5), async {
                        let (runtime, storage, _) = query_fixture().await;
                        let first = storage
                            .write_object(&Document {
                                id: "key".into(),
                                text: "saved".into(),
                            })
                            .await
                            .unwrap();
                        storage.flush().await.unwrap();
                        let mut request = ObjectSelection {
                            versions: ObjectVersions::Revisions,
                            ..Default::default()
                        };
                        request.selection.page_rows = 1;
                        let limits = harmos::operations::StreamSettings {
                            items: NonZeroUsize::MIN,
                            bytes: NonZeroUsize::new(4096).unwrap(),
                        };
                        let (sender, mut receiver) =
                            harmos::operations::stream::<ObjectEntry>(limits).unwrap();
                        let call = runtime
                            .operations
                            .submit_endpoints::<FollowObjects, _>(request.clone(), sender)
                            .unwrap();
                        let received = receiver.recv().await.unwrap();
                        assert_eq!(received.version, first.version);
                        drop(received);
                        let second = storage
                            .write_object(&Document {
                                id: "key".into(),
                                text: "live".into(),
                            })
                            .await
                            .unwrap();
                        let received = receiver.recv().await.unwrap();
                        assert_eq!(received.version, second.version);
                        drop(received);
                        storage.flush().await.unwrap();
                        call.cancel();
                        assert!(call.await.is_err());
                        drop(receiver);
                        request.selection.after = Some(second.position);
                        let (sender, mut receiver) =
                            harmos::operations::stream::<ObjectEntry>(limits).unwrap();
                        let call = runtime
                            .operations
                            .submit_endpoints::<FollowObjects, _>(request, sender)
                            .unwrap();
                        let third = storage
                            .write_object(&Document {
                                id: "key".into(),
                                text: "reconnected".into(),
                            })
                            .await
                            .unwrap();
                        let received = receiver.recv().await.unwrap();
                        assert_eq!(received.version, third.version);
                        drop(received);
                        call.cancel();
                        assert!(call.await.is_err());
                        drop(receiver);
                        runtime.stop().await.unwrap();
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("encoded_queries_verify", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::storage::EncodedObjectQuery;
                    let (runtime, storage, data) = query_fixture().await;
                    storage
                        .write_object(&Document {
                            id: "key".into(),
                            text: "value".into(),
                        })
                        .await
                        .unwrap();
                    storage.flush().await.unwrap();
                    let mut query = EncodedObjectQuery::default();
                    query.options.decoded_bytes = Some(NonZeroUsize::new(128).unwrap());
                    let mut reader = storage.query(query).await.unwrap();
                    assert!(reader.next_page().await.is_err());
                    drop(reader);
                    let mut query = EncodedObjectQuery {
                        schema: Some(value("key", "").schema),
                        ..Default::default()
                    };
                    query.schema.as_mut().unwrap().id = Identifier::new("unknown").unwrap();
                    assert!(storage.query(query).await.is_err());
                    data.objects.lock().unwrap().get_mut(&2).unwrap().value.item =
                        value("forged", "").item;
                    let mut reader = storage.query(EncodedObjectQuery::default()).await.unwrap();
                    assert!(reader.next_page().await.is_err());
                    drop(reader);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("follow_reports_pruned", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::storage::{
                        ObjectEntry, ObjectVersions,
                        operations::{FollowObjects, ObjectSelection},
                    };
                    tokio::time::timeout(std::time::Duration::from_secs(5), async {
                        let (runtime, storage, data) = query_fixture().await;
                        for index in 0..9 {
                            storage
                                .write_object(&Document {
                                    id: "key".into(),
                                    text: index.to_string(),
                                })
                                .await
                                .unwrap();
                        }
                        storage.flush().await.unwrap();
                        while data.objects.lock().unwrap().len() != 8 {
                            tokio::task::yield_now().await;
                        }
                        let request = ObjectSelection {
                            versions: ObjectVersions::Revisions,
                            ..Default::default()
                        };
                        let (sender, mut receiver) = harmos::operations::stream::<ObjectEntry>(
                            harmos::operations::StreamSettings {
                                items: NonZeroUsize::MIN,
                                bytes: NonZeroUsize::new(4096).unwrap(),
                            },
                        )
                        .unwrap();
                        let call = runtime
                            .operations
                            .submit_endpoints::<FollowObjects, _>(request, sender)
                            .unwrap();
                        let error = call.await.err().unwrap();
                        assert!(
                            error.to_string().contains("retained data begins"),
                            "{error}"
                        );
                        assert!(receiver.recv().await.is_none());
                        runtime.stop().await.unwrap();
                    })
                    .await
                    .unwrap();
                });
        });
    }
}

async fn query_fixture() -> (Runtime<Bank>, StorageManager<Objects>, Arc<Data<Objects>>) {
    let data = Arc::new(Data::<Objects>::with_catalog(history()));
    let storage = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            recovery: Arc::new(Recovery::new(history())),
            replicas: Vec::new(),
        },
        config(),
    )
    .unwrap();
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(Lanes::catalog())
        .runtime_with_storage(&storage)
        .await
        .unwrap();
    Operations::register(
        &runtime.operations,
        "storage",
        Lanes::Open,
        Arc::new(storage.clone()),
    )
    .unwrap();
    (runtime, storage, data)
}
async fn query_objects(
    runtime: &Runtime<Bank>,
    request: harmos::storage::operations::ObjectSelection,
) -> Vec<harmos::storage::ObjectEntry> {
    let (sender, mut receiver) = harmos::operations::stream::<
        harmos::storage::operations::ObjectResult,
    >(harmos::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    })
    .unwrap();
    let call = runtime
        .operations
        .submit_endpoints::<harmos::storage::operations::ReadObjects, _>(
            harmos::storage::operations::ReadObjectsRequest {
                selection: request,
                fields: None,
            },
            sender,
        )
        .unwrap();
    let mut values = Vec::new();
    while let Some(value) = receiver.recv().await {
        let harmos::storage::operations::ObjectResult::Value(value) = &*value else {
            panic!("expected full object");
        };
        values.push(value.clone());
    }
    call.await.unwrap();
    values
}
