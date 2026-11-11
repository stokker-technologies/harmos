/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Object exchange saves.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn object_exchange_saves() {
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
    runtime.operations.register_catalog(harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default()).await.unwrap();
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
    let request = harmos_protocol::storage::WriteObjects { scope: None };
    let mut writer = runtime
        .operations
        .exchange_protobuf::<WriteObjects>(request, super::super::writes::settings(), super::super::writes::settings())
        .unwrap();
    writer
        .send(
            DataBatch {
                sequence: 1,
                destination: first.schema.id.clone(),
                schema: first.schema,
                data: data.into(),
            }
            .into(),
        )
        .await
        .unwrap();
    writer.finish_input().unwrap();
    let mut accepted = None;
    while let Some(receipt) = writer.next().await.unwrap() {
        match receipt.receipt.as_ref().unwrap() {
            WriteReceipt::Accepted(receipt) => accepted = Some(receipt.cursor.clone()),
            WriteReceipt::Durable(receipt) => {
                assert_eq!(accepted.as_ref(), Some(&receipt.cursor))
            }
        }
    }
    let completion = writer.join().await.unwrap();
    assert_eq!(completion.batches, 1);
    assert_eq!(Some(harmos_protocol::storage::StorageCursor::from(storage.cursor_saved())), completion.cursor);
    let exact = storage.read_object::<Document>(&"a/b".into(), ObjectVersion::Latest).await.unwrap().unwrap();
    assert_eq!(exact.value.text, "first");
    drop(exact);
    runtime.shutdown().await.unwrap();
}
