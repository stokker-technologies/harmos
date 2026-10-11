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
pub(crate) use backend::{Data, Recovery, history};
pub(crate) use harmos::storage::recovery::RecoverySnapshot;
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
    pub(crate) execution: QueryLimits,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Fixture {
    pub(crate) async fn new(settings: StorageSettings) -> Self {
        Self::with_replicas(settings, Vec::new()).await
    }

    pub(crate) async fn with_replicas(
        config: StorageSettings,
        replicas: Vec<ReplicaTarget>,
    ) -> Self {
        let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
            .register(catalog())
            .register(operations::Lanes::catalog())
            .runtime()
            .await
            .unwrap();
        let data = Arc::new(Data::new(history()));
        let recovery = Arc::new(Recovery::new(history()));
        let execution = config.query_execution;
        let storage = StorageManager::new(
            StorageBackends {
                primary: data.clone(),
                recovery: recovery.clone(),
                replicas,
            },
            config,
        )
        .unwrap();
        storage.start(&runtime.services).await.unwrap();
        Self {
            runtime,
            storage,
            data,
            recovery,
            execution,
        }
    }

    pub(crate) async fn finish(self) {
        self.storage.close().await.unwrap();
        assert!(
            self.runtime
                .services
                .status(&harmos::services::ServiceKey::root("harmos-storage"))
                .is_none()
        );
        self.runtime.stop().await.unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */
pub(crate) fn config() -> StorageSettings {
    StorageSettings {
        flush_interval: Duration::from_secs(60),
        max_retries: 0,
        ..StorageSettings::default()
    }
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

pub(crate) fn decode_bytes(bytes: &[u8], allowance: NonZeroUsize) -> harmos::Result<Vec<u8>> {
    let required = bytes
        .len()
        .checked_add(std::mem::size_of::<Vec<u8>>())
        .ok_or(Error::ResourceExhausted {
            resource: "query decoded memory",
        })?;
    if required > allowance.get() {
        return Err(Error::ResourceExhausted {
            resource: "query decoded memory",
        });
    }
    Ok(Box::<[u8]>::from(bytes).into_vec())
}

pub(crate) fn record_query(page: ReadLimits) -> RecordQuery<Vec<u8>> {
    let mut query = RecordQuery::new(schema(), decode_bytes);
    query.options.page = Some(page);
    query.options.decoded_bytes = Some(page.bytes);
    query
}

pub(crate) fn refusing_record_query(page: ReadLimits) -> RecordQuery<Vec<u8>> {
    let first = std::sync::atomic::AtomicBool::new(true);
    let mut query = RecordQuery::new(schema(), move |bytes, allowance| {
        if first.swap(false, Ordering::SeqCst) {
            return Err(Error::ResourceExhausted {
                resource: "query decoded memory",
            });
        }
        decode_bytes(bytes, allowance)
    });
    query.options.page = Some(page);
    query.options.decoded_bytes = Some(page.bytes);
    query
}

pub(crate) fn stream_query(page: ReadLimits) -> StreamQuery<Vec<u8>> {
    let mut query = StreamQuery::new(schema(), decode_bytes);
    query.options.page = Some(page);
    query.options.decoded_bytes = Some(page.bytes);
    query
}

pub(crate) fn schema_query(page: ReadLimits) -> SchemaQuery {
    let mut query = SchemaQuery::new();
    query.options.page = Some(page);
    query.options.decoded_bytes = Some(page.bytes);
    query
}

pub(crate) fn file_query(page: ReadLimits) -> FileQuery {
    let mut query = FileQuery::new();
    query.options.page = Some(page);
    query.options.decoded_bytes = Some(page.bytes);
    query
}

pub(crate) async fn write<C>(storage: &StorageManager<C>, id: &str) -> StorageCursor {
    storage
        .write_records(&[record(&schema(), &Identifier::new(id).unwrap(), &[1, 2, 3])])
        .await
        .unwrap()
}

/* === Tests === */
