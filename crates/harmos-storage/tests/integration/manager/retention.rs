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
use super::waveforms::{Experiment, Objects};
use harmos_domain::operations::storage::Query;
use harmos_storage::Identifier;
use harmos_storage::query::transform::{
    Transform,
    project::{Project, Projection},
};
use harmos_storage::query::{ObjectSelection, ObjectVersions, QueryArrow, RowSlice, Source};
use harmos_storage::*;
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
    storage: StorageManager<Objects>,
    first: Experiment,
    first_receipt: ObjectReceipt,
    second_receipt: ObjectReceipt,
    earlier: StorageCursor,
    durable: StorageCursor,
    schema: SchemaKey,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Revisions {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let data = JsonStorage::new(
            JsonStorageSettings { root: directory.path().to_path_buf(), max_entry_bytes: NonZeroUsize::new(16 * 1024 * 1024).unwrap() },
            history(),
        );
        let storage = StorageManager::<Objects>::new(data.into_backends(super::fixture::state_settings()), storage_settings()).unwrap();
        super::fixture::start(&storage).await;

        // Two Revisions Of One Object Are Durable At Distinct Cursors.
        let first = Experiment { id: "experiment".into(), label: "first".into() };
        let first_receipt = storage.write_object(&first).await.unwrap();
        let earlier = storage.flush().await.unwrap();
        let second_receipt = storage.write_object(&Experiment { id: first.id.clone(), label: "second".into() }).await.unwrap();
        let durable = storage.flush().await.unwrap();
        let encoded = Objects::encode(&first, NonZeroUsize::new(16 * 1024).unwrap()).unwrap();
        Self { _directory: directory, storage, first, first_receipt, second_receipt, earlier, durable, schema: encoded.schema }
    }

    async fn finish(self) {
        self.storage.close().await.unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn labels<C>(
    storage: &StorageManager<C>, at: &StorageCursor, schema: &SchemaKey, versions: ObjectVersions,
) -> harmos_storage::Result<Vec<String>> {
    let mut reader = storage
        .query(Query {
            at: at.clone(),
            source: Source::Objects(ObjectSelection { schema: Some(schema.clone()), key: None, versions }),
            transforms: vec![Transform::Project(Project {
                fields: vec![Projection { field: "payload.label".into(), name: "label".into() }],
            })],
            slice: RowSlice::default(),
        })
        .await?;
    let mut labels = Vec::new();
    while let Some(output) = reader.next().await? {
        let QueryArrow::Batch(batch) = output else {
            continue;
        };
        let values = batch.as_record_batch().column_by_name("label").unwrap().as_any().downcast_ref::<arrow_array::StringArray>().unwrap();
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
    assert_eq!(labels(&r.storage, &r.durable, &r.schema, ObjectVersions::Revisions).await.unwrap(), ["first", "second"]);
    let exact =
        r.storage.read_object::<Experiment>(&r.first.id, ObjectVersion::Exact(r.first_receipt.version.clone())).await.unwrap().unwrap();
    assert_eq!(exact.value, r.first);
    let current = r.storage.read_object::<Experiment>(&r.first.id, ObjectVersion::Latest).await.unwrap().unwrap();
    assert_eq!(current.version, r.second_receipt.version);

    // A Cursor From Another History Is Refused Before Any Output.
    let mut foreign = r.durable.clone();
    foreign.history.epoch = Identifier::new("foreign").unwrap();
    assert!(latest(&foreign).await.is_err());
    r.finish().await;
}

#[tokio::test]
async fn release_reclaims_history() {
    // A Controlled Result Gate Separates Reclamation From Its Acknowledgement.
    let data = Arc::new(Data::<Objects>::with_catalog(history()));
    let backends = StorageBackends { primary: data.clone(), state: Arc::new(Recovery::new(history())) };
    let storage = StorageManager::<Objects>::new(backends, storage_settings()).unwrap();
    super::fixture::start(&storage).await;
    let first = Experiment { id: "experiment".into(), label: "first".into() };
    storage.write_object(&first).await.unwrap();
    let earlier = storage.flush().await.unwrap();
    storage.write_object(&Experiment { id: first.id.clone(), label: "second".into() }).await.unwrap();
    let durable = storage.flush().await.unwrap();
    let encoded = Objects::encode(&first, NonZeroUsize::new(16 * 1024).unwrap()).unwrap();

    assert_eq!(data.objects.lock().unwrap().len(), 2);
    data.control.block_prune_result.store(true, Ordering::SeqCst);
    storage.release_object_history(&encoded.schema, &encoded.item, &durable).await.unwrap();
    data.control.prune_result_entered.notified().await;
    assert_eq!(data.objects.lock().unwrap().len(), 1);
    data.control.block_prune_result.store(false, Ordering::SeqCst);
    data.control.prune_result_release.notify_one();

    // Reclaimed History Fails Explicitly While The Current Image Remains.
    assert!(labels(&storage, &earlier, &encoded.schema, ObjectVersions::Latest).await.is_err());
    assert_eq!(labels(&storage, &durable, &encoded.schema, ObjectVersions::Latest).await.unwrap(), ["second"]);
    storage.close().await.unwrap();
}
