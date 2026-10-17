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
use harmos::storage::query::transform::{
    Transform,
    project::{Project, Projection},
};
use harmos::storage::query::{
    ObjectSelection, ObjectVersions, Query, QueryArrow, RowSlice, Source,
};
use harmos::storage::*;
use harmos::{Identifier, Runtime};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::Ordering;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// Journaled Storage Holding Two Durable Revisions Of One Object.
struct Revisions {
    _directory: tempfile::TempDir,
    data: Arc<Data<Objects>>,
    storage: StorageManager<Objects>,
    runtime: Runtime<Bank>,
    first: Experiment,
    first_receipt: ObjectReceipt,
    second_receipt: ObjectReceipt,
    earlier: StorageCursor,
    durable: StorageCursor,
    schema: SchemaKey,
    item: ObjectKey,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Revisions {
    async fn new() -> Self {
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
        Self {
            _directory: directory,
            data,
            storage,
            runtime,
            first,
            first_receipt,
            second_receipt,
            earlier,
            durable,
            schema: encoded.schema,
            item: encoded.item,
        }
    }

    async fn finish(self) {
        self.storage.close().await.unwrap();
        self.runtime.stop().await.unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn labels<C>(
    storage: &StorageManager<C>,
    at: &StorageCursor,
    schema: &SchemaKey,
    versions: ObjectVersions,
) -> harmos::Result<Vec<String>> {
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
async fn reads_pin_cursor() {
    let r = Revisions::new().await;

    // Queries Read Exactly The Requested Cursor, Never Newer Data.
    let latest = |at| labels(&r.storage, at, &r.schema, ObjectVersions::Latest);
    assert_eq!(latest(&r.earlier).await.unwrap(), ["first"]);
    assert_eq!(latest(&r.durable).await.unwrap(), ["second"]);
    assert_eq!(
        labels(&r.storage, &r.durable, &r.schema, ObjectVersions::Revisions)
            .await
            .unwrap(),
        ["first", "second"]
    );
    let exact = r
        .storage
        .read_object::<Experiment>(&r.first.id, ObjectVersion::Exact(r.first_receipt.version.clone()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exact.value, r.first);
    let current = r
        .storage
        .read_object::<Experiment>(&r.first.id, ObjectVersion::Latest)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.version, r.second_receipt.version);

    // A Cursor From Another History Is Refused Before Any Output.
    let mut foreign = r.durable.clone();
    foreign.history.epoch = Identifier::new("foreign").unwrap();
    assert!(latest(&foreign).await.is_err());
    r.finish().await;
}

#[tokio::test]
async fn release_reclaims_history() {
    let r = Revisions::new().await;

    // Unreleased Lifecycle History Survives; Release Lets Pruning Reclaim It.
    assert_eq!(r.data.objects.lock().unwrap().len(), 2);
    r.data
        .control
        .block_prune_result
        .store(true, Ordering::SeqCst);
    r.storage
        .release_object_history(&r.schema, &r.item, &r.durable)
        .await
        .unwrap();
    r.data.control.prune_result_entered.notified().await;
    assert_eq!(r.data.objects.lock().unwrap().len(), 1);
    r.data
        .control
        .block_prune_result
        .store(false, Ordering::SeqCst);
    r.data.control.prune_result_release.notify_one();

    // Pruned History Is Unavailable Explicitly; The Current Image Remains.
    let latest = |at| labels(&r.storage, at, &r.schema, ObjectVersions::Latest);
    assert!(latest(&r.earlier).await.is_err());
    assert_eq!(latest(&r.durable).await.unwrap(), ["second"]);
    r.finish().await;
}
