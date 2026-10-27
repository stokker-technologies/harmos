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
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::super::{Bank, Data, Ledger, Recovery, catalog, config, history};
use super::Lanes;
use arrow_array::{StringArray, StructArray};
use arrow_schema::{DataType, Field, Fields};
use harmos_operations::storage::WriteObjects;
use harmos_runtime::storage::writes::{DataBatch, WriteReceipt};
use harmos_runtime::storage::{
    MutationId, ObjectCatalog, ObjectRetention, ObjectValue, ObjectVersion, StorageBackends, StorageManager, StoredObject,
};
use harmos_runtime::{DefinitionId, Identified, Identifier, Runtime};

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

#[harmos_runtime::message]
struct DocumentV2 {
    id: String,
    text: String,
}

/* === Enums === */

#[harmos_runtime::catalog]
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
    Objects::encode(&Document { id: key.into(), text: text.into() }, NonZeroUsize::new(4096).unwrap()).unwrap()
}

/* === Tests === */

#[tokio::test]
async fn object_exchange_saves() {
    let storage = StorageManager::<Objects>::new(
        StorageBackends { primary: Arc::new(Data::<Objects>::with_catalog(history())), state: Arc::new(Recovery::new(history())) },
        config(),
    )
    .unwrap();
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(Lanes::catalog())
        .runtime_with_storage(&storage)
        .await
        .unwrap();
    runtime
        .operations
        .register(harmos_runtime::operations::RegistrationExt::lane(
            harmos_runtime::builtin::storage_registration(Arc::new(storage.clone())).unwrap(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    let first = value("a/b", "first");
    let second = value("second", "two");
    // Register Authored Schema Before Sending Its Typed Payload.
    let document = Document { id: "bootstrap".into(), text: "schema".into() };
    storage.write_object(&document).await.unwrap();
    storage.flush().await.unwrap();
    let fields: Fields = vec![Field::new("id", DataType::Utf8, false), Field::new("text", DataType::Utf8, false)].into();
    let payload = StructArray::new(
        fields,
        vec![Arc::new(StringArray::from(vec!["a/b", "second"])), Arc::new(StringArray::from(vec!["first", "two"]))],
        None,
    );
    let data = super::writes::envelope(payload, &[first.item.as_str(), second.item.as_str()], &[0, 1]);
    let request = harmos_operations::storage::WriteObjectsRequest { scope: None };
    let mut writer = runtime.operations.exchange::<WriteObjects>(request, super::writes::settings(), super::writes::settings()).unwrap();
    writer.send(DataBatch { sequence: 1, destination: first.schema.id.clone(), schema: first.schema, data: data.into() }).await.unwrap();
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
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn object_batch_atomic() {
    let storage = StorageManager::<Objects>::new(
        StorageBackends { primary: Arc::new(Data::<Objects>::with_catalog(history())), state: Arc::new(Recovery::new(history())) },
        config(),
    )
    .unwrap();
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(Lanes::catalog())
        .runtime_with_storage(&storage)
        .await
        .unwrap();
    let mutation = MutationId { window: storage.mutation_window().unwrap().id, key: Identifier::new("atomic").unwrap() };
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
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn object_follow_resumes() {
    use harmos_operations::storage::FollowObjects;
    use harmos_runtime::storage::{ObjectEntry, query::ObjectVersions};
    let (runtime, storage, _) = query_fixture().await;
    let first = storage.write_object(&Document { id: "key".into(), text: "saved".into() }).await.unwrap();
    storage.flush().await.unwrap();
    let mut request = harmos_operations::storage::FollowObjectsRequest {
        after: None,
        selection: harmos_runtime::storage::query::ObjectSelection { schema: None, key: None, versions: ObjectVersions::Revisions },
    };
    let limits = harmos_runtime::operations::StreamSettings { items: NonZeroUsize::MIN, bytes: NonZeroUsize::new(4096).unwrap() };
    let (sender, mut receiver) = harmos_runtime::operations::stream::<ObjectEntry>(limits).unwrap();
    let call = runtime.operations.submit_endpoints::<FollowObjects, _>(request.clone(), sender).unwrap();
    let received = receiver.recv().await.unwrap();
    assert_eq!(received.version, first.version);
    drop(received);
    let second = storage.write_object(&Document { id: "key".into(), text: "live".into() }).await.unwrap();
    let received = receiver.recv().await.unwrap();
    assert_eq!(received.version, second.version);
    drop(received);
    storage.flush().await.unwrap();
    call.cancel();
    assert!(call.await.is_err());
    drop(receiver);
    request.after = Some(second.position);
    let (sender, mut receiver) = harmos_runtime::operations::stream::<ObjectEntry>(limits).unwrap();
    let call = runtime.operations.submit_endpoints::<FollowObjects, _>(request, sender).unwrap();
    let third = storage.write_object(&Document { id: "key".into(), text: "reconnected".into() }).await.unwrap();
    let received = receiver.recv().await.unwrap();
    assert_eq!(received.version, third.version);
    drop(received);
    call.cancel();
    assert!(call.await.is_err());
    drop(receiver);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn follow_reports_pruned() {
    use harmos_operations::storage::FollowObjects;
    use harmos_runtime::storage::{ObjectEntry, query::ObjectVersions};
    let (runtime, storage, data) = query_fixture().await;
    for index in 0..9 {
        storage.write_object(&Document { id: "key".into(), text: index.to_string() }).await.unwrap();
    }
    storage.flush().await.unwrap();
    while data.objects.lock().unwrap().len() != 8 {
        data.control.pruned.notified().await;
    }
    let request = harmos_operations::storage::FollowObjectsRequest {
        after: None,
        selection: harmos_runtime::storage::query::ObjectSelection { schema: None, key: None, versions: ObjectVersions::Revisions },
    };
    let (sender, mut receiver) = harmos_runtime::operations::stream::<ObjectEntry>(harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    })
    .unwrap();
    let call = runtime.operations.submit_endpoints::<FollowObjects, _>(request, sender).unwrap();
    let error = call.await.err().unwrap();
    let error = std::iter::successors(Some(&error as &(dyn std::error::Error + 'static)), |error| error.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ");
    assert!(error.contains("retained data begins"), "{error}");
    assert!(receiver.recv().await.is_none());
    runtime.stop().await.unwrap();
}

async fn query_fixture() -> (Runtime<Bank>, StorageManager<Objects>, Arc<Data<Objects>>) {
    let data = Arc::new(Data::<Objects>::with_catalog(history()));
    let storage =
        StorageManager::new(StorageBackends { primary: data.clone(), state: Arc::new(Recovery::new(history())) }, config()).unwrap();
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(Lanes::catalog())
        .runtime_with_storage(&storage)
        .await
        .unwrap();
    runtime
        .operations
        .register(harmos_runtime::operations::RegistrationExt::lane(
            harmos_runtime::builtin::storage_registration(Arc::new(storage.clone())).unwrap(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    (runtime, storage, data)
}
