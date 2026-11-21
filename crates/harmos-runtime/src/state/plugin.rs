/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Library-Owned Plugin Recovery State

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::fmt::Display;
use std::sync::Arc;

// Third Party Imports
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

// Local Imports
use super::{FactSource, Prepared, Receipt, Recorded, StateManager, SystemFact, writer};
use crate::Application;
use crate::storage::StorageManager;
use crate::supervision::executor::SpanContext;
use harmos_operations::plugins::{Error, Pending};
use harmos_protocol::host::plugins::{StateGetResponse, StatePutRequest};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub(crate) type Values = BTreeMap<String, Value>;

/* === Structs === */

#[derive(Clone, Default, Serialize, Deserialize)]
pub(crate) struct Value {
    pub(crate) revision: u64,
    pub(crate) value: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Mutation {
    pub(crate) key: String,
    pub(crate) value: Option<Value>,
}

/* === Enums === */

/* === Traits === */

pub(crate) trait Persistence: Send + Sync {
    fn get(&self, key: String) -> Pending<'_, StateGetResponse>;
    fn put(&self, key: String, request: StatePutRequest) -> Pending<'_, u64>;
    fn retain(&self, prefix: String, keys: Vec<String>) -> Pending<'_, ()>;
    fn retire(&self, key: String) -> Pending<'_, ()>;
}

/* === Implementations === */

impl Mutation {
    pub(crate) fn fact(&self) -> Result<SystemFact, Error> {
        let payload = serde_json::to_vec(self).map_err(|error| failure("Encode Plugin State", error))?;
        Ok(SystemFact::new(FactSource::Work("plugins".into()), "state".into(), payload))
    }

    pub(crate) fn restore(fact: &SystemFact) -> Result<Option<Self>, serde_json::Error> {
        if fact.source != FactSource::Work("plugins".into()) || fact.name != "state" {
            return Ok(None);
        }
        serde_json::from_slice(&fact.payload).map(Some)
    }

    pub(crate) fn apply(self, values: &mut Values) {
        match self.value {
            Some(value) => {
                values.insert(self.key, value);
            }
            None => {
                values.remove(&self.key);
            }
        }
    }
}

impl<A: Application> Persistence for StateManager<A> {
    fn get(&self, key: String) -> Pending<'_, StateGetResponse> {
        Box::pin(async move {
            let storage = self.plugin_storage()?;
            let state = self.tracked.state.read().await;
            let value = self.tracked.plugins.read().await.get(&key).cloned().unwrap_or_default();
            let position = state.position;
            drop(state);
            storage.flush_recovery().await.map_err(|error| failure("Read Durable Plugin State", error))?;
            if self.stored() < position {
                return Err(Error::new("state_not_durable", "Get Plugin State", "restore recovery storage before retrying"));
            }
            Ok(StateGetResponse {
                revision: value.revision,
                value: value.value,
            })
        })
    }

    fn put(&self, key: String, request: StatePutRequest) -> Pending<'_, u64> {
        Box::pin(async move {
            let storage = self.plugin_storage()?;
            let revision = request
                .expected_revision
                .checked_add(1)
                .ok_or_else(|| Error::new("state_revision_exhausted", "Put Plugin State", "remove and recreate the entry"))?;
            let mutation = Mutation {
                key,
                value: Some(Value { revision, value: request.value }),
            };
            let receipt = self.append_plugin(mutation, Some(request.expected_revision)).await?;
            storage.flush_recovery().await.map_err(|error| failure("Persist Plugin State", error))?;
            if self.stored() < receipt.frontier {
                return Err(Error::new("state_not_durable", "Put Plugin State", "restore recovery storage before retrying"));
            }
            Ok(revision)
        })
    }

    fn retain(&self, prefix: String, keys: Vec<String>) -> Pending<'_, ()> {
        Box::pin(async move {
            let retired = self
                .tracked
                .plugins
                .read()
                .await
                .keys()
                .filter(|key| key.starts_with(&prefix) && !keys.contains(key))
                .cloned()
                .collect::<Vec<_>>();
            for key in retired {
                self.retire(key).await?;
            }
            Ok(())
        })
    }

    fn retire(&self, key: String) -> Pending<'_, ()> {
        Box::pin(async move {
            if self.tracked.plugin_storage.is_none() && !self.tracked.plugins.read().await.contains_key(&key) {
                return Ok(());
            }
            let storage = self.plugin_storage()?;
            self.append_plugin(Mutation { key, value: None }, None).await?;
            storage.flush_recovery().await.map_err(|error| failure("Retire Plugin State", error))?;
            Ok(())
        })
    }
}

impl<A: Application> StateManager<A> {
    fn plugin_storage(&self) -> Result<&StorageManager, Error> {
        self.tracked.plugin_storage.as_ref().ok_or_else(|| {
            Error::new("state_storage_required", "Access Plugin State", "configure managed recovery storage before loading this plugin")
        })
    }

    async fn append_plugin(&self, mutation: Mutation, expected: Option<u64>) -> Result<Receipt, Error> {
        let preparation =
            Arc::clone(&self.tracked.record_preparation).acquire_owned().await.map_err(|error| failure("Prepare Plugin State", error))?;
        let fact = mutation.fact()?;
        let prepared = Prepared::system(&fact).map_err(|error| failure("Prepare Plugin State", error))?;
        let (reply, answer) = oneshot::channel();
        let request = writer::Request::Record {
            value: Box::new(Recorded::System(fact)),
            prepared,
            preparation,
            context: self.context.clone(),
            reply,
            plugin: Some((mutation, expected)),
        };
        let permit = self.writer.reserve().await.map_err(|error| failure("Admit Plugin State", error))?;
        let span = tracing::Span::current();
        permit.send((request, SpanContext::new(span.clone(), span)));
        answer.await.map_err(|error| failure("Write Plugin State", error))?.map_err(|error| match error {
            writer::RecordRefusal::Plugin(error) => error,
            writer::RecordRefusal::Preparation(error) => failure("Write Plugin State", error),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn failure(action: &str, error: impl Display) -> Error {
    Error::new("plugin_state_failure", format!("{action}: {error}"), "correct recovery storage and retry")
}

/* === Tests === */

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::config::ManagedStorageSettings;
    use crate::plugin::manager;
    use crate::state::{Focus, Never, PrincipalId, SnapshotPolicy};
    use crate::storage::{ReadLimits, SchemaKey, StorageBackends, StorageManager};
    use crate::{Identifier, Lifecycle, Runtime, Setup, Startup};
    use harmos_storage::memory::{Backend, RecoveryBackend};
    use harmos_storage::{PayloadEncoding, StoreHistory};
    use std::convert::Infallible;
    use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};
    use std::sync::atomic::Ordering;

    pub(crate) struct Model;
    impl Lifecycle for Model {
        type Config = ();
        type Error = Infallible;
        type Context = Startup<Self>;
        async fn prepare(_: (), _: &mut Setup<Self>) -> Result<Self, Infallible> {
            Ok(Self)
        }
    }
    impl Application for Model {
        type State = ();
        type Transaction = Never;
        type Record = ();
    }

    pub(crate) fn storage(data: &Arc<Backend>, recovery: &Arc<RecoveryBackend>) -> StorageManager {
        let settings = StorageManager::default_in_memory().unwrap().settings().clone();
        StorageManager::with_catalog(
            manager::catalog(),
            StorageBackends {
                primary: data.clone(),
                state: recovery.clone(),
            },
            settings,
        )
        .unwrap()
    }

    pub(crate) fn backends() -> (Arc<Backend>, Arc<RecoveryBackend>) {
        let history = StoreHistory {
            store: Identifier::new("state-store").unwrap(),
            epoch: Identifier::new("state-epoch").unwrap(),
        };
        (Arc::new(Backend::with_catalog(manager::catalog(), history.clone())), Arc::new(RecoveryBackend::new(history)))
    }

    pub(crate) async fn start(storage: &StorageManager, snapshot: bool, manager: Option<manager::Manager>) -> Runtime<Model> {
        Runtime::<Model>::start_with((), (), |setup| {
            if let Some(manager) = manager {
                setup.settings().plugins(manager);
            }
            let limits = ReadLimits {
                rows: NonZeroUsize::new(16).unwrap(),
                bytes: NonZeroUsize::new(64 * 1024).unwrap(),
            };
            setup.settings().storage(storage).managed_storage(ManagedStorageSettings {
                replay: limits,
                capture: limits,
                journal_tail: 1,
                snapshots: snapshot.then(|| SnapshotPolicy {
                    schema: SchemaKey {
                        id: Identifier::new("plugin-state").unwrap(),
                        version: NonZeroU32::new(1).unwrap(),
                        payload_encoding: PayloadEncoding::PostcardV1,
                    },
                    every: NonZeroU64::new(1).unwrap(),
                    max_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
                }),
            });
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn plugin_state_survives_journal_and_snapshot_restart() {
        for snapshot in [false, true] {
            let (data, recovery) = backends();
            let disk = storage(&data, &recovery);
            let runtime = start(&disk, snapshot, None).await;
            assert_eq!(
                runtime
                    .state
                    .put(
                        "entry".into(),
                        StatePutRequest {
                            expected_revision: 0,
                            value: vec![0, 255, 1, 2, 3]
                        }
                    )
                    .await
                    .unwrap(),
                1
            );
            runtime.stop().await.unwrap();
            drop(runtime);
            drop(disk);
            let disk = storage(&data, &recovery);
            let runtime = start(&disk, snapshot, None).await;
            let value = runtime.state.get("entry".into()).await.unwrap();
            assert_eq!(value.value, vec![0, 255, 1, 2, 3]);
            assert_eq!(value.revision, 1);
            runtime.state.retire("entry".into()).await.unwrap();
            runtime.stop().await.unwrap();
            drop(runtime);
            drop(disk);
            let disk = storage(&data, &recovery);
            let runtime = start(&disk, snapshot, None).await;
            assert_eq!(runtime.state.get("entry".into()).await.unwrap(), StateGetResponse::default());
            runtime.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn plugin_state_conflicts_and_clear_preserve_revision() {
        let (data, recovery) = backends();
        let disk = storage(&data, &recovery);
        let runtime = start(&disk, false, None).await;
        let first = StatePutRequest { expected_revision: 0, value: vec![9] };
        runtime.state.put("entry".into(), first.clone()).await.unwrap();
        let error = runtime.state.put("entry".into(), first).await.unwrap_err();
        assert_eq!(error.0.code, "state_revision_conflict");
        assert_eq!(runtime.state.get("entry".into()).await.unwrap().value, vec![9]);
        runtime.state.put("entry".into(), StatePutRequest { expected_revision: 1, value: vec![] }).await.unwrap();
        assert_eq!(runtime.state.get("entry".into()).await.unwrap(), StateGetResponse { value: vec![], revision: 2 });
        runtime.stop().await.unwrap();
    }

    #[tokio::test]
    async fn plugin_state_put_waits_for_durable_flush() {
        let (data, recovery) = backends();
        let disk = storage(&data, &recovery);
        let runtime = start(&disk, false, None).await;
        recovery.control.block_flush.store(true, Ordering::SeqCst);
        let put = runtime.state.put("entry".into(), StatePutRequest { expected_revision: 0, value: vec![7] });
        tokio::pin!(put);
        tokio::select! {
            result = &mut put => panic!("Put returned before durable flush: {result:?}"),
            () = recovery.control.flush_entered.notified() => {}
        }
        assert_eq!(disk.status().recovery_saved.sequence, 0);
        recovery.control.block_flush.store(false, Ordering::SeqCst);
        recovery.control.flush_release.notify_one();
        assert_eq!(put.await.unwrap(), 1);
        assert!(runtime.state.stored() >= runtime.state.applied());

        // Recover Only the Durable Backend Image, Before the Original Host Shuts Down.
        let (crash_data, crash_recovery) = backends();
        *crash_recovery.entries.lock().unwrap() = recovery.entries.lock().unwrap().clone();
        crash_recovery.saved.store(recovery.saved.load(Ordering::SeqCst), Ordering::SeqCst);
        let crash_disk = storage(&crash_data, &crash_recovery);
        let recovered = start(&crash_disk, false, None).await;
        assert_eq!(recovered.state.get("entry".into()).await.unwrap().value, vec![7]);
        recovered.stop().await.unwrap();
        runtime.stop().await.unwrap();
    }

    #[tokio::test]
    async fn plugin_state_requires_managed_recovery() {
        let runtime = Runtime::<Model>::start((), ()).await.unwrap();
        let error = runtime.state.put("entry".into(), StatePutRequest::default()).await.unwrap_err();
        assert_eq!(error.0.code, "state_storage_required");
        runtime.stop().await.unwrap();
    }

    #[tokio::test]
    async fn repeated_snapshots_compact_covered_plugin_state() {
        let (data, recovery) = backends();
        let disk = storage(&data, &recovery);
        let runtime = start(&disk, true, None).await;
        for revision in 0..8 {
            runtime
                .state
                .put(
                    "entry".into(),
                    StatePutRequest {
                        expected_revision: revision,
                        value: vec![revision as u8],
                    },
                )
                .await
                .unwrap();
            // The Next Queued Write Runs Only After the Writer's Complete Snapshot.
            runtime.state.reversals(PrincipalId::new("reader"), Focus::all()).await.unwrap();
            let snapshot = recovery.snapshot.lock().unwrap().clone().unwrap();
            assert_eq!(snapshot.through.sequence, revision + 1);
            assert!(recovery.entries.lock().unwrap().is_empty());
        }
        runtime.stop().await.unwrap();
    }
}
