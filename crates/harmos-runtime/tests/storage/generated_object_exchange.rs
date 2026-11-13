/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Append Immutable Object Revisions And Retain Their Original Receipts

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Tests === */

#[tokio::test]
async fn object_append_saves() {
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
    crate::support::app::register(&runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
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
    let data = super::super::writes::envelope(payload, &[first.item.as_str(), second.item.as_str()], &[0, 1]);
    let mutation = MutationId {
        window: storage.mutation_window().unwrap().id,
        key: Identifier::new("object-append").unwrap(),
    };
    let request = harmos_protocol::v1::StorageAppendObjectRevisionsRequest {
        mutation: Some(mutation.clone().into()),
        scope: None,
        batch: Some(
            DataBatch {
                sequence: 1,
                destination: first.schema.id.clone(),
                schema: first.schema,
                data: data.into(),
            }
            .into(),
        ),
    };
    let accepted = runtime.operations.call_protobuf::<AppendObjectRevisions>(request.clone()).await.unwrap().receipt.unwrap();
    let position = storage.cursor_accepted();
    storage.wait_mutation_durable(&mutation, &position).await.unwrap();
    let completed = runtime.operations.call_protobuf::<AppendObjectRevisions>(request.clone()).await.unwrap().receipt.unwrap();
    assert_eq!(completed.accepted_at, accepted.accepted_at);
    assert_eq!(completed.durable_at, completed.accepted_at);
    assert!(completed.completed);
    assert_eq!(runtime.operations.call_protobuf::<AppendObjectRevisions>(request).await.unwrap().receipt, Some(completed));
    let exact = storage.get_object::<Document>(&"a/b".into(), ObjectVersion::Latest).await.unwrap().unwrap();
    assert_eq!(exact.value.text, "first");
    drop(exact);
    crate::support::app::shutdown(&runtime).await.unwrap();
}
