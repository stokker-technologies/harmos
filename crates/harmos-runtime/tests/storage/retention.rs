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
use super::backend::{Data, Recovery, history, storage_settings};
use super::support::{Bank, Ledger, catalog};
use super::waveforms::{Experiment, Objects};
use harmos_runtime::storage::operations::Query;
use harmos_runtime::storage::query::transform::{
    Transform,
    project::{Project, Projection},
};
use harmos_runtime::storage::query::{ObjectSelection, ObjectVersions, QueryArrow, RowSlice, Source};
use harmos_runtime::storage::*;
use harmos_runtime::{Identifier, Runtime};
use std::num::NonZeroUsize;
use std::sync::Arc;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
async fn labels<C>(
    storage: &StorageManager<C>,
    at: &StorageCursor,
    schema: &SchemaKey,
    versions: ObjectVersions,
) -> harmos_runtime::Result<Vec<String>> {
    let mut reader = storage
        .query(Query {
            at: at.clone(),
            source: Source::Objects(ObjectSelection {
                schema: Some(schema.clone()),
                key: None,
                versions,
            }),
            transforms: vec![Transform::Project(Project {
                fields: vec![Projection {
                    field: "payload.label".into(),
                    name: "label".into(),
                }],
            })],
            slice: RowSlice::default(),
        })
        .await?;
    let mut labels = Vec::new();
    while let Some(output) = reader.next().await? {
        let QueryArrow::Batch(batch) = output else {
            continue;
        };
        let values = batch
            .as_record_batch()
            .column_by_name("label")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow_array::StringArray>()
            .unwrap();
        labels.extend(values.iter().map(|value| value.unwrap().to_owned()));
    }
    Ok(labels)
}

/* === Tests === */
#[tokio::test]
async fn retention() {
    // Configure A Real Temporary Journal With An Explicitly Released Object Lifecycle.
    let directory = tempfile::tempdir().unwrap();
    let data = Arc::new(Data::<Objects>::journaled(directory.path(), history()).unwrap());
    let storage = StorageManager::<Objects>::new(
        StorageBackends {
            primary: data.clone(),
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

    // Two Revisions Of One Object Are Durable At Distinct Cursors.
    let first = Experiment {
        id: "experiment".into(),
        label: "first".into(),
    };
    let first_receipt = storage.write_object(&first).await.unwrap();
    let earlier = storage.flush().await.unwrap();
    let second_receipt = storage
        .write_object(&Experiment {
            id: first.id.clone(),
            label: "second".into(),
        })
        .await
        .unwrap();
    let durable = storage.flush().await.unwrap();
    let encoded = Objects::encode(&first, NonZeroUsize::new(16 * 1024).unwrap()).unwrap();

    // Queries Read Exactly The Requested Cursor, Never Newer Data.
    assert_eq!(
        labels(&storage, &earlier, &encoded.schema, ObjectVersions::Latest)
            .await
            .unwrap(),
        ["first"]
    );
    assert_eq!(
        labels(&storage, &durable, &encoded.schema, ObjectVersions::Latest)
            .await
            .unwrap(),
        ["second"]
    );
    assert_eq!(
        labels(
            &storage,
            &durable,
            &encoded.schema,
            ObjectVersions::Revisions
        )
        .await
        .unwrap(),
        ["first", "second"]
    );
    let exact = storage
        .read_object::<Experiment>(&first.id, ObjectVersion::Exact(first_receipt.version))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exact.value, first);
    let latest = storage
        .read_object::<Experiment>(&first.id, ObjectVersion::Latest)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest.version, second_receipt.version);

    // A Cursor From Another History Is Refused Before Any Output.
    let mut foreign = durable.clone();
    foreign.history.epoch = Identifier::new("foreign").unwrap();
    assert!(
        labels(&storage, &foreign, &encoded.schema, ObjectVersions::Latest)
            .await
            .is_err()
    );

    // Unreleased Lifecycle History Survives; Release Lets Pruning Reclaim It.
    assert_eq!(data.objects.lock().unwrap().len(), 2);
    storage
        .release_object_history(&encoded.schema, &encoded.item, &durable)
        .await
        .unwrap();
    while data.objects.lock().unwrap().len() != 1 {
        tokio::task::yield_now().await;
    }

    // Pruned History Is Unavailable Explicitly; The Current Image Remains.
    assert!(
        labels(&storage, &earlier, &encoded.schema, ObjectVersions::Latest)
            .await
            .is_err()
    );
    assert_eq!(
        labels(&storage, &durable, &encoded.schema, ObjectVersions::Latest)
            .await
            .unwrap(),
        ["second"]
    );
    storage.close().await.unwrap();
    runtime.stop().await.unwrap();
}
