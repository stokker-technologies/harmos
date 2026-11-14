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

pub use config::PluginManagerSettings;
pub use harmos_operations::plugins::{Error, Management};
pub use persistence::catalog;

/* === Imports === */

#[cfg(feature = "plugin-guest")]
use harmos_protocol::client::{PluginDescription, PluginEntry, PluginState};

use harmos_protocol::client::{
    PluginsCreateRequest, PluginsCreateResponse, PluginsDeleteRequest, PluginsDeleteResponse, PluginsGetRequest, PluginsGetResponse,
    PluginsListRequest, PluginsListResponse, PluginsReplaceRequest, PluginsReplaceResponse, PluginsRestartRequest, PluginsRestartResponse,
    PluginsSetRequest, PluginsSetResponse, PluginsWatchRequest, PluginsWatchResponse,
};

// Standard Library Imports
use std::sync::{Arc, OnceLock};
// Third Party Imports
use tokio::sync::{mpsc, oneshot};
// Local Imports
use crate::storage::StorageManager;
use crate::{Application, Runtime};
use harmos_operations::plugins::{Access, Pending};
use harmos_protocol::client as wire;

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
    commands: OnceLock<mpsc::Sender<actor::Command>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Manager {
    pub fn new(storage: StorageManager, settings: PluginManagerSettings) -> Result<Self, Error> {
        settings.validate()?;
        Ok(Self {
            inner: Arc::new(Inner {
                storage,
                settings,
                commands: OnceLock::new(),
            }),
        })
    }
    pub async fn start<A: Application>(&self, runtime: &Arc<Runtime<A>>) -> Result<(), Error> {
        let clock = runtime.operations.clock().clone();
        let startup = clock.activity("plugin manager restoration");
        let (sender, receiver) = mpsc::channel(self.inner.settings.command_capacity.get());
        self.inner
            .commands
            .set(sender.clone())
            .map_err(|_| Error::new("already_started", "Start Plugin Manager", "start this manager only once"))?;
        let host = Arc::downgrade(runtime);
        let settings = self.inner.settings.placement.clone();
        let attach: actor::Attach = Arc::new(move |name, placement, values| {
            let activity = clock.activity("plugin manager activation");
            let host = host.clone();
            let settings = settings.clone();
            Box::pin(async move {
                let _activity = activity;
                let runtime = host.upgrade().ok_or(crate::plugin::grpc::Error::Disconnected)?;
                runtime.attach_grpc_entry(name, placement, settings, values).await
            })
        });
        let host = Arc::downgrade(runtime);
        let placement_settings = self.inner.settings.placement.clone();
        let stage: actor::Stage = Arc::new(move |name, placement, values| {
            let host = host.clone();
            let settings = placement_settings.clone();
            Box::pin(async move {
                let runtime = host.upgrade().ok_or(crate::plugin::grpc::Error::Disconnected)?;
                runtime.prepare_grpc_entry(name, placement, settings, values).await
            })
        });
        let host = Arc::downgrade(runtime);
        let placement_settings = self.inner.settings.placement.clone();
        let activate: actor::Activate = Arc::new(move |prepared| {
            let runtime = host.upgrade().ok_or(crate::plugin::grpc::Error::Disconnected)?;
            runtime.activate_grpc_entry(prepared, placement_settings.clone())
        });
        let actor = actor::Actor::open(
            self.inner.storage.engine(),
            self.inner.settings.clone(),
            attach,
            sender.clone(),
            stage,
            activate,
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
            let (updates, mut observations) = tokio::sync::watch::channel(std::collections::BTreeMap::new());
            runtime.plugin.observe_entries(Arc::new(move |entries| {
                updates.send_replace(entries);
            }));
            tokio::spawn(async move {
                let mut previous = std::collections::BTreeSet::new();
                while observations.changed().await.is_ok() {
                    let entries = observations.borrow_and_update().clone();
                    let names = entries.keys().cloned().collect::<std::collections::BTreeSet<_>>();
                    let changed = previous.union(&names).cloned().collect::<Vec<_>>();
                    previous = names;
                    for name in changed {
                        let status = entries.get(&name).cloned();
                        let (state, activation, failure) = match status {
                            Some(status) => {
                                use crate::guest::PluginState as GuestState;
                                let state = match status.state {
                                    GuestState::Ready => PluginState::Ready,
                                    GuestState::Registered | GuestState::Initializing => PluginState::Connecting,
                                    GuestState::Stopping => PluginState::Stopping,
                                    GuestState::Failed => PluginState::Unavailable,
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
    async fn call<T>(&self, command: impl FnOnce(oneshot::Sender<Result<T, Error>>) -> actor::Command) -> Result<T, Error> {
        let sender = self
            .inner
            .commands
            .get()
            .ok_or_else(|| Error::new("not_started", "Manage Plugins", "start the plugin manager with the host"))?;
        let (reply, receiver) = oneshot::channel();
        sender.send(command(reply)).await.map_err(|_| closed())?;
        receiver.await.map_err(|_| closed())?
    }
}

impl<A: Application> crate::runtime::Builder<A> {
    pub fn plugins(&mut self, manager: Manager) -> &mut Self {
        self.resource(manager.clone());
        self.resource(Access(Arc::new(manager)));
        self.catalog(harmos_operations::plugins::catalog(), crate::operations::Lane::default());
        self
    }
}

impl Management for Manager {
    fn replace(&self, request: PluginsReplaceRequest) -> Pending<'_, PluginsReplaceResponse> {
        Box::pin(self.call(move |reply| actor::Command::Replace(request, reply)))
    }
    fn settings_list(&self, request: wire::plugins::SettingsListRequest) -> Pending<'_, wire::plugins::SettingsListResponse> {
        Box::pin(self.call(move |reply| actor::Command::SettingsList(request, reply)))
    }
    fn settings_set(&self, request: wire::plugins::SettingsSetRequest) -> Pending<'_, wire::plugins::SettingsSetResponse> {
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

fn closed() -> Error {
    Error::new("manager_stopped", "Manage Plugins", "start a host with an active plugin manager")
}

/* === Tests === */
