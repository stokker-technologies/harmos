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

#[path = "../../../harmos-storage/tests/support/fixture.rs"]
mod shared;

/* === Re-Exports === */

pub(crate) use shared::{config, record, schema, state_settings, write};

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
pub(crate) use harmos_runtime::storage::state::RecoverySnapshot;
pub(crate) use harmos_runtime::storage::*;
pub(crate) use harmos_runtime::{BackendFailure, Error, Identifier, Runtime};
pub(crate) use support::{Bank, Ledger, catalog};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Fixture {
    pub(crate) runtime: Runtime<Bank>,
    storage_fixture: shared::Fixture<StorageManager>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Fixture {
    pub(crate) fn from_parts(runtime: Runtime<Bank>, storage: StorageManager, data: Arc<Data>, recovery: Arc<Recovery>) -> Self {
        Self {
            runtime,
            storage_fixture: shared::Fixture { storage, data, recovery },
        }
    }

    pub(crate) async fn new(config: StorageSettings) -> Self {
        let storage_fixture = shared::Fixture::build_with(config, |backends, config| StorageManager::new(backends, config).unwrap());
        let storage = &storage_fixture.storage;
        let runtime = crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
            setup.resource(std::ops::Deref::deref(storage).clone());
            setup.settings().operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(NonZeroUsize::new(16 * 1024 * 1024).unwrap()),
                ..Default::default()
            });
            setup.settings().register(catalog());
        })
        .await
        .unwrap();
        storage.start(&runtime).await.unwrap();
        Self { runtime, storage_fixture }
    }

    pub(crate) async fn finish(self) {
        self.storage.close().await.unwrap();
        crate::support::app::shutdown(&self.runtime).await.unwrap();
    }
}

impl std::ops::Deref for Fixture {
    type Target = shared::Fixture<StorageManager>;
    fn deref(&self) -> &Self::Target {
        &self.storage_fixture
    }
}

/* === Public Functions === */

pub(crate) async fn saved_records(storage: &StorageManager, schema: &SchemaKey) -> Vec<(u64, Identifier)> {
    use harmos_runtime::storage::query::{QueryArrow, RecordSelection, RowSlice, Source};
    let mut reader = storage
        .query(harmos_runtime::storage::Query {
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
        let QueryArrow::Batch(batch) = output else {
            continue;
        };
        let batch = batch.as_record_batch();
        let positions = batch.column_by_name("position").unwrap().as_any().downcast_ref::<arrow_array::UInt64Array>().unwrap();
        let items = batch.column_by_name("item").unwrap().as_any().downcast_ref::<arrow_array::StringArray>().unwrap();
        for row in 0..batch.num_rows() {
            records.push((positions.value(row), Identifier::new(items.value(row)).unwrap()));
        }
    }
    records
}

/* === Tests === */
