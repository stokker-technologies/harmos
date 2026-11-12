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
mod persistence;

/* === Re-Exports === */

pub use config::PluginManagerSettings;
pub use content::{ContentRead, ContentSource, PublishedContent};
pub use harmos_operations::plugins::{Error, Management};
pub use persistence::catalog;

/* === Imports === */

// Standard Library Imports
use std::sync::{Arc, OnceLock};
// Third Party Imports
use tokio::sync::{mpsc, oneshot};
// Local Imports
use crate::storage::StorageManager;
use crate::{Application, Runtime};
use harmos_operations::plugins::{Access, Pending};
use harmos_protocol::host as wire;

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
    content: Arc<dyn ContentSource>,
    settings: PluginManagerSettings,
    commands: OnceLock<mpsc::Sender<actor::Command>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Manager {
    pub fn new(storage: StorageManager, content: Arc<dyn ContentSource>, settings: PluginManagerSettings) -> Result<Self, Error> {
        settings.validate()?;
        Ok(Self {
            inner: Arc::new(Inner {
                storage,
                content,
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
        let attach: actor::Attach = Arc::new(move |placement| {
            let activity = clock.activity("plugin manager activation");
            let host = host.clone();
            let settings = settings.clone();
            Box::pin(async move {
                let _activity = activity;
                let runtime = host.upgrade().ok_or(crate::plugin::grpc::Error::Disconnected)?;
                runtime.attach_grpc(placement, settings).await
            })
        });
        let actor = actor::Actor::open(
            self.inner.storage.engine(),
            self.inner.content.clone(),
            self.inner.settings.clone(),
            attach,
            sender,
            runtime.stop_participant(),
        )
        .await?;
        tokio::spawn(actor.run(receiver, startup));
        Ok(())
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
        self.resource(Access(Arc::new(manager)));
        self.catalog(harmos_operations::plugins::catalog(), crate::operations::Lane::default());
        self
    }
}

impl Management for Manager {
    fn get(&self, request: wire::PluginsGetRequest) -> Pending<'_, wire::PluginsGetResponse> {
        Box::pin(self.call(move |reply| actor::Command::Get(request, reply)))
    }
    fn list(&self, request: wire::PluginsListRequest) -> Pending<'_, wire::PluginsListResponse> {
        Box::pin(self.call(move |reply| actor::Command::List(request, reply)))
    }
    fn create(&self, request: wire::PluginsCreateRequest) -> Pending<'_, wire::PluginsCreateResponse> {
        Box::pin(self.call(move |reply| actor::Command::Create(request, reply)))
    }
    fn set(&self, request: wire::PluginsSetRequest) -> Pending<'_, wire::PluginsSetResponse> {
        Box::pin(self.call(move |reply| actor::Command::Set(request, reply)))
    }
    fn delete(&self, request: wire::PluginsDeleteRequest) -> Pending<'_, wire::PluginsDeleteResponse> {
        Box::pin(self.call(move |reply| actor::Command::Delete(request, reply)))
    }
    fn restart(&self, request: wire::PluginsRestartRequest) -> Pending<'_, wire::PluginsRestartResponse> {
        Box::pin(self.call(move |reply| actor::Command::Restart(request, reply)))
    }
    fn watch(&self, request: wire::PluginsWatchRequest) -> Pending<'_, mpsc::Receiver<wire::PluginsWatchResponse>> {
        Box::pin(self.call(move |reply| actor::Command::Watch(request, reply)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn closed() -> Error {
    Error::new("manager_stopped", "Manage Plugins", "start a host with an active plugin manager")
}

/* === Tests === */
