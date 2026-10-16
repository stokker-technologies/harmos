/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![allow(dead_code, unused_imports)]
/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
pub(crate) use std::num::{NonZeroU32, NonZeroUsize};
pub(crate) use std::sync::Arc;
pub(crate) use std::sync::atomic::Ordering;
pub(crate) use std::time::Duration;
// Third Party Imports

// Local Imports
use super::{backend, operations, support};
pub(crate) use backend::{Data, Recovery, history, storage_settings};
pub(crate) use harmos::storage::state::RecoverySnapshot;
pub(crate) use harmos::storage::*;
pub(crate) use harmos::{BackendFailure, Error, Identifier, Runtime};
pub(crate) use support::{Bank, Ledger, catalog};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct Fixture {
    pub(crate) runtime: Runtime<Bank>,
    pub(crate) storage: StorageManager,
    pub(crate) data: Arc<Data>,
    pub(crate) recovery: Arc<Recovery>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Fixture {
    pub(crate) async fn new(config: StorageSettings) -> Self {
        let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
            .register(catalog())
            .register(operations::Lanes::catalog())
            .runtime()
            .await
            .unwrap();
        let data = Arc::new(Data::new(history()));
        let recovery = Arc::new(Recovery::new(history()));
        let storage = StorageManager::new(
            StorageBackends {
                primary: data.clone(),
                state: recovery.clone(),
            },
            config,
        )
        .unwrap();
        storage.start(&runtime).await.unwrap();
        Self {
            runtime,
            storage,
            data,
            recovery,
        }
    }

    pub(crate) async fn finish(self) {
        self.storage.close().await.unwrap();
        assert!(
            self.runtime
                .operations
                .resident(&harmos::operations::resident::Key::root("harmos-storage"))
                .is_none()
        );
        self.runtime.stop().await.unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */
pub(crate) fn config() -> StorageSettings {
    storage_settings()
}

pub(crate) fn schema() -> SchemaKey {
    SchemaKey {
        payload_encoding: harmos::storage::PayloadEncoding::PostcardV1,
        id: Identifier::new("reading").unwrap(),
        version: NonZeroU32::new(1).unwrap(),
    }
}

pub(crate) fn record<'a>(
    schema: &'a SchemaKey,
    item: &'a Identifier,
    payload: &'a [u8],
) -> RecordWrite<'a> {
    RecordWrite {
        schema,
        scope: None,
        item,
        source: None,
        source_time: None,
        payload,
    }
}

/// Saved records of `schema` as (position, item), read through the Query operation contract.
pub(crate) async fn saved_records<C>(
    storage: &StorageManager<C>,
    schema: &SchemaKey,
) -> Vec<(u64, Identifier)> {
    use harmos::storage::query::{Query, QueryOutput, RecordSelection, RowSlice, Source};
    let mut reader = storage
        .query(Query {
            at: storage.cursor_saved(),
            source: Source::Records(RecordSelection {
                schema: schema.clone(),
                scope: None,
                item: None,
                source_start: None,
                source_end: None,
            }),
            transforms: Vec::new(),
            slice: RowSlice::default(),
        })
        .await
        .unwrap();
    let mut records = Vec::new();
    while let Some(output) = reader.next().await.unwrap() {
        let QueryOutput::Batch(batch) = output else {
            continue;
        };
        let batch = batch.as_record_batch();
        let positions = batch
            .column_by_name("position")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow_array::UInt64Array>()
            .unwrap();
        let items = batch
            .column_by_name("item")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow_array::StringArray>()
            .unwrap();
        for row in 0..batch.num_rows() {
            records.push((
                positions.value(row),
                Identifier::new(items.value(row)).unwrap(),
            ));
        }
    }
    records
}

pub(crate) async fn write<C>(storage: &StorageManager<C>, id: &str) -> StorageCursor {
    storage
        .write_records(&[record(&schema(), &Identifier::new(id).unwrap(), &[1, 2, 3])])
        .await
        .unwrap()
}

/* === Tests === */
