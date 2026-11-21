/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Durable Plugin Manager

/* === Modules === */

mod actor;
mod config;
mod content;
#[cfg(feature = "plugin-guest")]
mod guest;
mod persistence;
mod replacement;
pub(crate) mod settings;

/* === Re-Exports === */

pub use config::{PluginManagerSettings, PluginStateSettings};
pub use harmos_operations::plugins::{Error, Management};
pub use persistence::catalog;

/* === Imports === */

#[cfg(feature = "plugin-guest")]
use harmos_protocol::host::{PluginEntry, PluginState};

#[cfg(feature = "plugin-guest")]
use harmos_protocol::host::PluginDescription;

use harmos_protocol::host::PluginsCreateRequest;
use harmos_protocol::host::PluginsCreateResponse;
use harmos_protocol::host::PluginsDeleteRequest;
use harmos_protocol::host::PluginsDeleteResponse;
use harmos_protocol::host::PluginsGetRequest;
use harmos_protocol::host::PluginsGetResponse;
use harmos_protocol::host::PluginsListRequest;
use harmos_protocol::host::PluginsListResponse;
use harmos_protocol::host::PluginsReplaceRequest;
use harmos_protocol::host::PluginsReplaceResponse;
use harmos_protocol::host::PluginsRestartRequest;
use harmos_protocol::host::PluginsRestartResponse;
use harmos_protocol::host::PluginsSetRequest;
use harmos_protocol::host::PluginsSetResponse;
use harmos_protocol::host::PluginsWatchRequest;
use harmos_protocol::host::PluginsWatchResponse;

// Standard Library Imports
#[cfg(feature = "plugin-guest")]
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};
// Third Party Imports
#[cfg(feature = "plugin-guest")]
use tokio::sync::watch;
use tokio::sync::{mpsc, oneshot};
// Local Imports
use crate::operations::Lane;
use crate::plugin::grpc;
use crate::runtime::Builder;
use crate::state::plugin::Persistence;
use crate::storage::StorageManager;
use crate::{Application, Runtime};
use harmos_operations::plugins::{Access, Pending};
use harmos_protocol::host;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct Manager {
    inner: Arc<Inner>,
}

struct Inner {
    storage: StorageManager,
    settings: PluginManagerSettings,
    commands: OnceLock<Commands>,
    states: OnceLock<Arc<dyn Persistence>>,
}

struct Commands {
    mutations: mpsc::Sender<actor::Command>,
    queries: mpsc::Sender<actor::Command>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Manager {
    /// Returns the currently admitted gRPC activation of a ready managed entry.
    pub async fn activation(&self, name: impl Into<String>) -> Result<grpc::Activation, Error> {
        self.call(move |reply| actor::Command::Activation(name.into(), reply)).await
    }

    pub fn new(storage: StorageManager, settings: PluginManagerSettings) -> Result<Self, Error> {
        settings.validate()?;
        Ok(Self {
            inner: Arc::new(Inner {
                storage,
                settings,
                commands: OnceLock::new(),
                states: OnceLock::new(),
            }),
        })
    }
    pub async fn start<A: Application>(&self, runtime: &Arc<Runtime<A>>) -> Result<(), Error> {
        let clock = runtime.operations.clock().clone();
        let startup = clock.activity("plugin manager restoration");
        let (sender, receiver) = mpsc::channel(self.inner.settings.command_capacity.get());
        let (queries, reading) = mpsc::channel(self.inner.settings.command_capacity.get());
        self.inner
            .commands
            .set(Commands { mutations: sender.clone(), queries })
            .map_err(|_| Error::new("already_started", "Start Plugin Manager", "start this manager only once"))?;
        let host = Arc::downgrade(runtime);
        let settings = self.inner.settings.placement.clone();
        let manager = self.clone();
        let attach: actor::Attach = Arc::new(move |name, placement, values, cancelled| {
            let activity = clock.activity("plugin manager activation");
            let host = host.clone();
            let settings = settings.clone();
            let manager = manager.clone();
            Box::pin(async move {
                let _activity = activity;
                let runtime = host.upgrade().ok_or(grpc::Error::Disconnected)?;
                runtime.attach_grpc_entry(name, placement, settings, values, cancelled, manager).await
            })
        });
        let host = Arc::downgrade(runtime);
        let placement_settings = self.inner.settings.placement.clone();
        let stage: actor::Stage = Arc::new(move |name, placement, values| {
            let host = host.clone();
            let settings = placement_settings.clone();
            Box::pin(async move {
                let runtime = host.upgrade().ok_or(grpc::Error::Disconnected)?;
                runtime.prepare_grpc_entry(name, placement, settings, values).await
            })
        });
        let host = Arc::downgrade(runtime);
        let placement_settings = self.inner.settings.placement.clone();
        let activation_clock = runtime.operations.clock().clone();
        let activate: actor::Activate = Arc::new(move |mut prepared| {
            let host = host.clone();
            let settings = placement_settings.clone();
            let clock = activation_clock.clone();
            Box::pin(async move {
                let Some(runtime) = host.upgrade() else {
                    prepared.stop(&clock, &settings).await?;
                    return Err(grpc::Error::Disconnected);
                };
                runtime.activate_grpc_entry(prepared, settings).await
            })
        });
        let states: Arc<dyn Persistence> = Arc::new(runtime.state.clone());
        self.inner
            .states
            .set(states.clone())
            .map_err(|_| Error::new("already_started", "Start Plugin State", "start the manager only once"))?;
        let actor = actor::Actor::open(
            self.inner.storage.engine(),
            self.inner.settings.clone(),
            states,
            actor::Lifecycle { attach, stage, activate },
            (sender.clone(), reading),
            runtime.stop_participant(),
        )
        .await?;
        #[cfg(feature = "plugin-guest")]
        let actor = {
            let mut actor = actor;
            let plugin = runtime.plugin.clone();
            actor.guest_control = Some(Arc::new(move |name, action| {
                let plugin = plugin.clone();
                Box::pin(async move {
                    if plugin.status(&name).is_none() {
                        return Ok(());
                    }
                    match action {
                        actor::GuestAction::Restart => plugin.reload(&name).await,
                        actor::GuestAction::Stop => plugin.stop(&name).await,
                        actor::GuestAction::Remove => plugin.remove(&name).await,
                    }
                    .map_err(|error| {
                        Error::new("guest_lifecycle", format!("Manage Guest Plugin: {error}"), "correct the reported lifecycle failure")
                    })
                })
            }));
            let (updates, mut observations) = watch::channel(BTreeMap::new());
            runtime.plugin.observe_entries(Arc::new(move |entries| {
                updates.send_replace(entries);
            }));
            tokio::spawn(async move {
                let mut previous = BTreeSet::new();
                while observations.changed().await.is_ok() {
                    let entries = observations.borrow_and_update().clone();
                    let names = entries.keys().cloned().collect::<BTreeSet<_>>();
                    let changed = previous.union(&names).cloned().collect::<Vec<_>>();
                    previous = names;
                    for name in changed {
                        let status = entries.get(&name).cloned();
                        let (state, activation, failure) = match status {
                            Some(status) => {
                                let state = match status.state {
                                    crate::guest::PluginState::Ready => PluginState::Ready,
                                    crate::guest::PluginState::Registered | crate::guest::PluginState::Initializing => {
                                        PluginState::Connecting
                                    }
                                    crate::guest::PluginState::Stopping => PluginState::Stopping,
                                    crate::guest::PluginState::Failed => PluginState::Unavailable,
                                };
                                (state, status.generation.map_or(0, |generation| generation.get()), status.last_failure)
                            }
                            None => (PluginState::Stopped, 0, None),
                        };
                        if sender.send(actor::Command::GuestHealth { name, state, activation, failure }).await.is_err() {
                            return;
                        }
                    }
                }
            });
            actor
        };
        tokio::spawn(actor.run(receiver, startup));
        Ok(())
    }
    #[cfg(feature = "plugin-guest")]
    pub(crate) async fn register_guest(&self, description: PluginDescription) -> Result<PluginEntry, Error> {
        self.call(move |reply| actor::Command::Guest(Box::new(description), reply)).await
    }
    pub(crate) async fn admitted(&self, activation: grpc::Activation) -> Result<(), Error> {
        self.call(move |reply| actor::Command::Admitted(Box::new(activation), reply)).await
    }

    async fn call<T>(&self, command: impl FnOnce(oneshot::Sender<Result<T, Error>>) -> actor::Command) -> Result<T, Error> {
        let sender = self
            .inner
            .commands
            .get()
            .ok_or_else(|| Error::new("not_started", "Manage Plugins", "start the plugin manager with the host"))?;
        let (reply, receiver) = oneshot::channel();
        let command = command(reply);
        let sender = if command.is_query() { &sender.queries } else { &sender.mutations };
        sender.send(command).await.map_err(|_| closed())?;
        receiver.await.map_err(|_| closed())?
    }
}

impl<A: Application> Builder<A> {
    pub fn plugins(&mut self, manager: Manager) -> &mut Self {
        self.resource(manager.clone());
        self.resource(Access(Arc::new(manager)));
        self.catalog(harmos_operations::plugins::catalog(), Lane::default());
        self
    }
}

impl Management for Manager {
    fn state_get(&self, name: String) -> Pending<'_, host::plugins::StateGetResponse> {
        Box::pin(async move {
            let key = self.call(move |reply| actor::Command::StateKey(name, reply)).await?;
            self.inner.states.get().ok_or_else(closed)?.get(key).await
        })
    }
    fn state_put(&self, name: String, request: host::plugins::StatePutRequest) -> Pending<'_, host::plugins::StatePutResponse> {
        Box::pin(async move {
            if request.value.len() > self.inner.settings.state.max_bytes.get() {
                return Err(Error::new(
                    "state_capacity",
                    "Put Plugin State",
                    "reduce the value or configure PluginStateSettings.max_bytes",
                ));
            }
            let key = self.call(move |reply| actor::Command::StateKey(name, reply)).await?;
            let revision = self.inner.states.get().ok_or_else(closed)?.put(key, request).await?;
            Ok(host::plugins::StatePutResponse { revision })
        })
    }

    fn replace(&self, request: PluginsReplaceRequest) -> Pending<'_, PluginsReplaceResponse> {
        Box::pin(self.call(move |reply| actor::Command::Replace(request, reply)))
    }
    fn settings_list(&self, request: host::plugins::SettingsListRequest) -> Pending<'_, host::plugins::SettingsListResponse> {
        Box::pin(self.call(move |reply| actor::Command::SettingsList(request, reply)))
    }
    fn settings_set(&self, request: host::plugins::SettingsSetRequest) -> Pending<'_, host::plugins::SettingsSetResponse> {
        Box::pin(self.call(move |reply| actor::Command::SettingsSet(request, reply)))
    }

    fn get(&self, request: PluginsGetRequest) -> Pending<'_, PluginsGetResponse> {
        Box::pin(self.call(move |reply| actor::Command::Get(request, reply)))
    }
    fn list(&self, request: PluginsListRequest) -> Pending<'_, PluginsListResponse> {
        Box::pin(self.call(move |reply| actor::Command::List(request, reply)))
    }
    fn create(&self, request: PluginsCreateRequest) -> Pending<'_, PluginsCreateResponse> {
        Box::pin(self.call(move |reply| actor::Command::Create(Box::new(request), reply)))
    }
    fn set(&self, request: PluginsSetRequest) -> Pending<'_, PluginsSetResponse> {
        Box::pin(self.call(move |reply| actor::Command::Set(request, reply)))
    }
    fn delete(&self, request: PluginsDeleteRequest) -> Pending<'_, PluginsDeleteResponse> {
        Box::pin(self.call(move |reply| actor::Command::Delete(request, reply)))
    }
    fn restart(&self, request: PluginsRestartRequest) -> Pending<'_, PluginsRestartResponse> {
        Box::pin(self.call(move |reply| actor::Command::Restart(request, reply)))
    }
    fn watch(&self, request: PluginsWatchRequest) -> Pending<'_, mpsc::Receiver<PluginsWatchResponse>> {
        Box::pin(self.call(move |reply| actor::Command::Watch(request, reply)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn state_prefix(namespace: &str) -> String {
    format!("{}:{namespace}", namespace.len())
}

fn closed() -> Error {
    Error::new("manager_stopped", "Manage Plugins", "start a host with an active plugin manager")
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::plugin::tests::{backends, start, storage};
    extern crate harmos;

    use self::harmos::plugin::{Context, HostOperation, Operation, Plugin};
    use prost_types::Timestamp;
    use std::env;
    use std::num::NonZeroUsize;
    use std::time::Duration;

    fn settings() -> PluginManagerSettings {
        let capacity = NonZeroUsize::new(32).unwrap();
        PluginManagerSettings {
            namespace: "plugin-manager-story".into(),
            directory: env::current_dir().unwrap().join("target/plugin-state-test"),
            target: "x86_64-pc-windows-msvc".into(),
            state: PluginStateSettings { max_bytes: NonZeroUsize::new(8).unwrap() },
            placement: grpc::PlacementSettings {
                reconnect_delay: Duration::from_millis(100),
                stop_grace: Duration::from_secs(1),
                stream_window: capacity,
                interval_window: Duration::from_secs(1),
                interval_lag: Default::default(),
            },
            bundle: Default::default(),
            entries: capacity,
            create_receipts: capacity,
            registry_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
            command_capacity: capacity,
            retained_cuts: capacity,
            watch_capacity: capacity,
            response_bytes: NonZeroUsize::new(65536).unwrap(),
        }
    }

    fn entry(identity: &str) -> host::PluginsCreateRequest {
        host::PluginsCreateRequest {
            request_id: identity.into(),
            name: "example".into(),
            origin: None,
            value: Some(host::PluginValue { enabled: false }),
            placement: Some(host::PluginPlacement {
                kind: Some(host::plugin_placement::Kind::Remote(host::PluginRemote { url: "http://127.0.0.1:1".into() })),
            }),
        }
    }

    #[tokio::test]
    async fn initialize_reads_state_after_plugin_and_host_restart() {
        let (observed, mut observations) = mpsc::unbounded_channel();
        let initialize = Operation::new("example.initialize", async move |ctx: &Context, _: ()| -> harmos::Result<()> {
            let state = ctx.state();
            let previous = state.get::<Timestamp>().await?;
            state.put(&Timestamp { seconds: previous.seconds + 1, nanos: 0 }).await?;
            observed
                .send(previous.seconds)
                .map_err(|_| harmos::Error::Invalid("Initialize Plugin State: retain the observation receiver".into()))?;
            Ok(())
        })
        .visibility(harmos::Visibility::Protected);
        let mut listener = Plugin::new("example", "1.0.0")
            .remote()
            .register(|ctx| {
                ctx.register(HostOperation::PluginsStateGet)?;
                ctx.register(HostOperation::PluginsStatePut)?;
                ctx.register(initialize)?;
                Ok(())
            })
            .unwrap()
            .start("127.0.0.1:0")
            .await
            .unwrap();
        let (data, recovery) = backends();
        let disk = storage(&data, &recovery);
        let manager = Manager::new(disk.clone(), settings()).unwrap();
        let runtime = Arc::new(start(&disk, true, Some(manager.clone())).await);
        manager.start(&runtime).await.unwrap();
        let mut request = entry("initial");
        request.value = Some(host::PluginValue { enabled: true });
        request.placement = Some(host::PluginPlacement {
            kind: Some(host::plugin_placement::Kind::Remote(host::PluginRemote {
                url: format!("http://{}", listener.address()),
            })),
        });
        manager.create(request).await.unwrap();
        assert_eq!(observations.recv().await.unwrap(), 0);
        let current = manager.get(host::PluginsGetRequest { name: "example".into(), at: None }).await.unwrap().entry.unwrap();
        manager
            .restart(host::PluginsRestartRequest {
                name: current.name,
                expected_revision: current.revision,
            })
            .await
            .unwrap();
        assert_eq!(observations.recv().await.unwrap(), 1);
        runtime.stop().await.unwrap();
        drop(runtime);
        drop(manager);
        drop(disk);
        let disk = storage(&data, &recovery);
        let manager = Manager::new(disk.clone(), settings()).unwrap();
        let runtime = Arc::new(start(&disk, true, Some(manager.clone())).await);
        manager.start(&runtime).await.unwrap();
        assert_eq!(observations.recv().await.unwrap(), 2);
        runtime.stop().await.unwrap();
        listener.close().await.unwrap();
    }

    #[tokio::test]
    async fn plugin_entry_state_survives_restart_and_is_removed_with_entry() {
        let (data, recovery) = backends();
        let disk = storage(&data, &recovery);
        let manager = Manager::new(disk.clone(), settings()).unwrap();
        let runtime = Arc::new(start(&disk, true, Some(manager.clone())).await);
        manager.start(&runtime).await.unwrap();
        manager.create(entry("first")).await.unwrap();
        manager.state_put("example".into(), host::plugins::StatePutRequest { expected_revision: 0, value: vec![7, 8] }).await.unwrap();
        let error = manager
            .state_put("example".into(), host::plugins::StatePutRequest { expected_revision: 1, value: vec![0; 9] })
            .await
            .unwrap_err();
        assert_eq!(error.0.code, "state_capacity");
        assert!(manager.state_get("missing".into()).await.is_err());
        runtime.stop().await.unwrap();
        drop(runtime);
        drop(manager);
        drop(disk);
        let disk = storage(&data, &recovery);
        let manager = Manager::new(disk.clone(), settings()).unwrap();
        let runtime = Arc::new(start(&disk, true, Some(manager.clone())).await);
        manager.start(&runtime).await.unwrap();
        let saved = manager.state_get("example".into()).await.unwrap();
        assert_eq!(saved.value, vec![7, 8]);
        assert_eq!(saved.revision, 1);
        let current = manager.get(host::PluginsGetRequest { name: "example".into(), at: None }).await.unwrap().entry.unwrap();
        let key = manager.call(|reply| actor::Command::StateKey("example".into(), reply)).await.unwrap();
        manager
            .delete(host::PluginsDeleteRequest {
                name: current.name,
                expected_revision: current.revision,
            })
            .await
            .unwrap();
        assert_eq!(runtime.state.get(key).await.unwrap(), host::plugins::StateGetResponse::default());
        manager.create(entry("second")).await.unwrap();
        assert_eq!(manager.state_get("example".into()).await.unwrap(), host::plugins::StateGetResponse::default());
        runtime.stop().await.unwrap();
    }
}
