/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Lifecycle and Durable Read Boundaries

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos_protocol::client::PluginDescription;

use harmos_protocol::client::{
    PluginChange, PluginEntry, PluginProcess, PluginSnapshot, PluginState, PluginsCreateRequest, PluginsCreateResponse,
    PluginsDeleteRequest, PluginsDeleteResponse, PluginsGetRequest, PluginsGetResponse, PluginsListRequest, PluginsListResponse,
    PluginsReplaceRequest, PluginsReplaceResponse, PluginsRestartRequest, PluginsRestartResponse, PluginsSetRequest, PluginsSetResponse,
    PluginsWatchRequest, PluginsWatchResponse,
};

// Standard Library Imports
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    future::Future,
    pin::Pin,
    sync::Arc,
};
// Third Party Imports
use prost::Message;
use sha2::{Digest, Sha256};
use tokio::{
    sync::{broadcast, mpsc, oneshot},
    task::JoinHandle,
};
// Local Imports
use super::{
    Error, PluginManagerSettings,
    content::Installation,
    persistence::{Registry, SavedEntry, SavedMutation, Store, problem},
};
use crate::plugin::grpc::{Attached, Managed, Placement, Status};
use harmos_protocol::{client as wire, v1};

/* === Constants === */

pub(super) const CAPACITY_ACTION: &str = "Watch Plugins: event exceeds response capacity; open a bounded selection";
pub(super) const GAP_ACTION: &str = "Watch Plugins: event history expired; open a new snapshot";

/* === Macros === */

/* === Type Aliases === */

pub(super) type GuestControl = Arc<dyn Fn(String, GuestAction) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send>> + Send + Sync>;

type Reply<T> = oneshot::Sender<Result<T, Error>>;
type Attachment = Pin<Box<dyn Future<Output = Result<Attached, crate::plugin::grpc::Error>> + Send>>;
pub(super) type Attach = Arc<dyn Fn(String, Placement, Option<v1::SettingValues>) -> Attachment + Send + Sync>;
pub(super) type Stage = Arc<
    dyn Fn(
            String,
            Placement,
            Option<v1::SettingValues>,
        ) -> Pin<Box<dyn Future<Output = Result<crate::plugin::grpc::PreparedPlugin, crate::plugin::grpc::Error>> + Send>>
        + Send
        + Sync,
>;
pub(super) type Activate = Arc<dyn Fn(crate::plugin::grpc::PreparedPlugin) -> Result<Attached, crate::plugin::grpc::Error> + Send + Sync>;

/* === Structs === */

pub(super) struct Actor {
    pub(super) store: Store,
    pub(super) guest_control: Option<GuestControl>,
    pub(super) registry: Registry,
    pub(super) settings: PluginManagerSettings,
    attach: Attach,
    pub(super) stage: Stage,
    pub(super) activate: Activate,
    commands: mpsc::Sender<Command>,
    participant: crate::runtime::StopParticipant,
    history: v1::History,
    sequence: u64,
    snapshots: VecDeque<Snapshot>,
    pub(super) issued: BTreeSet<u64>,
    changes: broadcast::Sender<PluginsWatchResponse>,
    workers: BTreeMap<String, Worker>,
}

#[derive(Clone)]
pub(super) struct Snapshot {
    pub(super) cut: v1::ReadCut,
    entries: BTreeMap<String, PluginEntry>,
    event: Option<PluginChange>,
    pub(super) settings: BTreeMap<String, super::settings::Snapshot>,
}

struct Worker {
    stop: oneshot::Sender<()>,
    task: JoinHandle<Result<(), Error>>,
}

/* === Enums === */

pub(super) enum GuestAction {
    Restart,
    Stop,
    Remove,
}

pub(super) enum Command {
    #[cfg(feature = "plugin-guest")]
    Guest(Box<PluginDescription>, Reply<PluginEntry>),
    #[cfg(feature = "plugin-guest")]
    GuestHealth {
        name: String,
        state: PluginState,
        activation: u64,
        failure: Option<String>,
    },
    Replace(PluginsReplaceRequest, Reply<PluginsReplaceResponse>),
    SettingsList(wire::plugins::SettingsListRequest, Reply<wire::plugins::SettingsListResponse>),
    SettingsSet(wire::plugins::SettingsSetRequest, Reply<wire::plugins::SettingsSetResponse>),
    Get(PluginsGetRequest, Reply<PluginsGetResponse>),
    List(PluginsListRequest, Reply<PluginsListResponse>),
    Watch(PluginsWatchRequest, Reply<mpsc::Receiver<PluginsWatchResponse>>),
    Create(Box<PluginsCreateRequest>, Reply<PluginsCreateResponse>),
    Set(PluginsSetRequest, Reply<PluginsSetResponse>),
    Delete(PluginsDeleteRequest, Reply<PluginsDeleteResponse>),
    Restart(PluginsRestartRequest, Reply<PluginsRestartResponse>),
    Health {
        name: String,
        activation: u64,
        status: Box<Result<Status, String>>,
    },
}

/* === Traits === */

/* === Implementations === */

impl Actor {
    pub async fn open(
        storage: harmos_storage::StorageManager, settings: PluginManagerSettings, attach: Attach, commands: mpsc::Sender<Command>,
        stage: Stage, activate: Activate, participant: crate::runtime::StopParticipant,
    ) -> Result<Self, Error> {
        let (store, mut registry) = Store::open(storage, &settings).await?;
        if registry.entries.len() > settings.entries.get()
            || (registry.creates.len()
                + registry.entries.values().map(|entry| entry.settings_receipts.len() + entry.replacements.len()).sum::<usize>())
                > settings.create_receipts.get()
        {
            return Err(Error::new(
                "registry_capacity",
                "Restore Plugin Registry",
                "configure capacity for the persisted entries and replay receipts",
            ));
        }
        let mut entries = registry.values()?;
        for (name, entry) in &mut entries {
            entry.process = None;
            entry.failure = None;
            if entry.value.as_ref().is_some_and(|value| value.enabled) {
                entry.state = PluginState::Connecting as i32;
                entry.activation = next_activation(entry.activation)?;
            } else {
                entry.state = PluginState::Stopped as i32;
            }
            let saved = registry.entries.get_mut(name).expect("restored entry exists");
            saved.generation = entry.activation;
            saved.entry = entry.encode_to_vec();
        }
        let mut epoch = [0_u8; 16];
        getrandom::fill(&mut epoch).map_err(|error| problem("Create Plugin Read History", error))?;
        let history = v1::History {
            store: settings.namespace.clone(),
            epoch: crate::bundle::digest(&epoch),
        };
        let changes = broadcast::channel(settings.watch_capacity.get()).0;
        let mut actor = Self {
            store,
            guest_control: None,
            registry,
            settings,
            attach,
            stage,
            activate,
            commands,
            participant,
            history,
            sequence: 0,
            snapshots: VecDeque::new(),
            issued: BTreeSet::new(),
            changes,
            workers: BTreeMap::new(),
        };
        actor.store.save(&actor.registry).await?;
        actor.publish(None)?;
        Ok(actor)
    }

    pub async fn run(mut self, mut receiver: mpsc::Receiver<Command>, startup: crate::clock::Activity) {
        let names = self.registry.entries.keys().cloned().collect::<Vec<_>>();
        for name in names {
            self.start(&name);
        }
        drop(startup);
        loop {
            let command = tokio::select! {
                biased;
                () = self.participant.stopping() => break,
                command = receiver.recv() => match command { Some(command) => command, None => break },
            };
            match command {
                #[cfg(feature = "plugin-guest")]
                Command::Guest(description, reply) => {
                    let _ = reply.send(self.guest(*description).await);
                }
                #[cfg(feature = "plugin-guest")]
                Command::GuestHealth { name, state, activation, failure } => {
                    if let Err(error) = self.guest_health(name, state, activation, failure).await {
                        tracing::error!(%error, "guest entry state persistence failed");
                    }
                }
                Command::Replace(request, reply) => {
                    let _ = reply.send(self.replace(request).await);
                }
                Command::SettingsList(request, reply) => {
                    let _ = reply.send(self.settings_list(request));
                }
                Command::SettingsSet(request, reply) => {
                    let _ = reply.send(self.settings_set(request).await);
                }
                Command::Get(request, reply) => {
                    let _ = reply.send(self.get(request));
                }
                Command::List(request, reply) => {
                    let _ = reply.send(self.list(request));
                }
                Command::Watch(request, reply) => {
                    let _ = reply.send(self.watch(request));
                }
                Command::Create(request, reply) => {
                    let _ = reply.send(self.create(*request).await);
                }
                Command::Set(request, reply) => {
                    let _ = reply.send(self.set(request).await);
                }
                Command::Delete(request, reply) => {
                    let _ = reply.send(self.delete(request).await);
                }
                Command::Restart(request, reply) => {
                    let _ = reply.send(self.restart(request).await);
                }
                Command::Health { name, activation, status } => {
                    if let Err(error) = self.health(name, activation, *status).await {
                        tracing::error!(%error, "plugin state persistence failed; stopping manager");
                        let _ = self.changes.send(PluginsWatchResponse {
                            resolved_at: Some(self.cut()),
                            event: Some(wire::plugins_watch_response::Event::ResyncRequired(error.to_string())),
                        });
                        break;
                    }
                }
            }
        }
        receiver.close();
        let workers = std::mem::take(&mut self.workers);
        for (_, worker) in workers {
            let _ = worker.stop.send(());
            match worker.task.await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => tracing::warn!(%error, "plugin manager shutdown reported a failure"),
                Err(error) => tracing::error!(%error, "plugin manager activation task failed"),
            }
        }
    }

    pub(super) fn cut(&self) -> v1::ReadCut {
        v1::ReadCut {
            positions: vec![v1::Position {
                history: Some(self.history.clone()),
                sequence: self.sequence,
            }],
            revisions: vec![],
        }
    }

    pub(super) fn publish(&mut self, event: Option<PluginChange>) -> Result<(), Error> {
        self.sequence =
            self.sequence.checked_add(1).ok_or_else(|| Error::new("history_exhausted", "Publish Plugin State", "restart the host"))?;
        let cut = self.cut();
        let snapshot = Snapshot {
            cut: cut.clone(),
            entries: self.registry.values()?,
            event: event.clone(),
            settings: super::settings::snapshots(&self.registry)?,
        };
        for retained in &mut self.snapshots {
            retained.settings.retain(|name, _| self.registry.entries.contains_key(name));
        }
        self.snapshots.push_back(snapshot);
        while self.snapshots.len() > self.settings.retained_cuts.get() {
            self.snapshots.pop_front();
        }
        let first = self.snapshots.front().expect("current snapshot exists").cut.positions[0].sequence;
        self.issued.retain(|sequence| *sequence >= first);
        if let Some(event) = event {
            if self.changes.receiver_count() > 0 {
                self.issued.insert(self.sequence);
            }
            let _ = self.changes.send(PluginsWatchResponse {
                resolved_at: Some(cut),
                event: Some(wire::plugins_watch_response::Event::Change(event)),
            });
        }
        Ok(())
    }

    pub(super) fn snapshot(&self, at: Option<&v1::ReadCut>) -> Result<&Snapshot, Error> {
        match at {
            Some(at) => self
                .snapshots
                .iter()
                .find(|snapshot| &snapshot.cut == at && self.issued.contains(&snapshot.cut.positions[0].sequence))
                .ok_or_else(|| {
                    Error::new(
                        "position_unavailable",
                        "Read Plugin Entries",
                        "open a new read; the supplied cut is unissued, expired or foreign",
                    )
                }),
            None => self.snapshots.back().ok_or_else(|| Error::new("not_started", "Read Plugin Entries", "start the manager")),
        }
    }

    fn get(&mut self, request: PluginsGetRequest) -> Result<PluginsGetResponse, Error> {
        let snapshot = self.snapshot(request.at.as_ref())?;
        let entry = snapshot.entries.get(&request.name).cloned().ok_or_else(missing)?;
        let cut = snapshot.cut.clone();
        let response = PluginsGetResponse {
            entry: Some(entry),
            resolved_at: Some(cut.clone()),
        };
        if response.encoded_len() > self.settings.response_bytes.get() {
            return Err(Error::new("entry_capacity", "Get Plugin Entry", "configure read response capacity for the complete entry"));
        }
        self.issued.insert(cut.positions[0].sequence);
        Ok(response)
    }

    fn list(&mut self, request: PluginsListRequest) -> Result<PluginsListResponse, Error> {
        let snapshot = self.snapshot(request.at.as_ref())?.clone();
        let response = PluginsListResponse {
            entries: snapshot.entries.into_values().collect(),
            resolved_at: Some(snapshot.cut.clone()),
        };
        self.issued.insert(snapshot.cut.positions[0].sequence);
        Ok(response)
    }

    fn watch(&mut self, request: PluginsWatchRequest) -> Result<mpsc::Receiver<PluginsWatchResponse>, Error> {
        let snapshot = self.snapshot(request.at.as_ref())?.clone();
        let name = request.name.unwrap_or_default();
        let entries = snapshot.entries.values().filter(|entry| name.is_empty() || entry.name == name).cloned().collect();
        let opening = PluginsWatchResponse {
            resolved_at: Some(snapshot.cut.clone()),
            event: Some(wire::plugins_watch_response::Event::Snapshot(PluginSnapshot { entries })),
        };
        if opening.encoded_len() > self.settings.response_bytes.get() {
            return Err(Error::new(
                "watch_capacity",
                "Watch Plugins",
                "select one entry or configure enough response bytes for the opening snapshot",
            ));
        }
        let sequence = snapshot.cut.positions[0].sequence;
        self.issued.extend(
            self.snapshots
                .iter()
                .filter(|snapshot| snapshot.cut.positions[0].sequence >= sequence)
                .map(|snapshot| snapshot.cut.positions[0].sequence),
        );
        let replay = self
            .snapshots
            .iter()
            .filter(|snapshot| snapshot.cut.positions[0].sequence > sequence)
            .filter_map(|snapshot| {
                snapshot.event.clone().map(|event| PluginsWatchResponse {
                    resolved_at: Some(snapshot.cut.clone()),
                    event: Some(wire::plugins_watch_response::Event::Change(event)),
                })
            })
            .collect::<Vec<_>>();
        let mut changes = self.changes.subscribe();
        let (sender, receiver) = mpsc::channel(self.settings.watch_capacity.get());
        let response_bytes = self.settings.response_bytes.get();
        tokio::spawn(async move {
            let mut resolved_at = opening.resolved_at.clone();
            if sender.send(opening).await.is_err() {
                return;
            }
            for event in replay {
                if selected(&event, &name) && !watch_event(&sender, event, response_bytes, &mut resolved_at).await {
                    return;
                }
            }
            loop {
                tokio::select! {
                    biased;
                    () = sender.closed() => return,
                    event = changes.recv() => match event {
                        Ok(event) if selected(&event, &name) => if !watch_event(&sender, event, response_bytes, &mut resolved_at).await { return; },
                        Ok(_) => {},
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            let event = PluginsWatchResponse { resolved_at, event: Some(wire::plugins_watch_response::Event::ResyncRequired(GAP_ACTION.into())) };
                            let _ = sender.send(event).await;
                            return;
                        },
                        Err(broadcast::error::RecvError::Closed) => return,
                    }
                }
            }
        });
        Ok(receiver)
    }

    pub(super) async fn commit(&mut self, mut registry: Registry, change: PluginChange) -> Result<(), Error> {
        super::settings::materialize(&mut registry)?;
        self.store.save(&registry).await?;
        self.registry = registry;
        self.publish(Some(change))
    }

    pub(super) fn receipt(&mut self, request_id: String) -> v1::Receipt {
        let position = v1::Position {
            history: Some(self.history.clone()),
            sequence: self.sequence + 1,
        };
        self.issued.insert(position.sequence);
        v1::Receipt {
            request_id,
            accepted_at: Some(position.clone()),
            durable_at: Some(position),
            completed: true,
        }
    }

    async fn create(&mut self, request: PluginsCreateRequest) -> Result<PluginsCreateResponse, Error> {
        if request.request_id.is_empty() || request.name.is_empty() {
            return Err(Error::new("identity_required", "Create Plugin", "supply a nonempty request identifier and entry name"));
        }
        let encoded = request.encode_to_vec();
        if let Some(original) = self.registry.creates.get(&request.request_id) {
            if original.request != encoded {
                return Err(Error::new("request_conflict", "Create Plugin", "resend the original payload or use a new request identifier"));
            }
            return PluginsCreateResponse::decode(original.response.as_slice()).map_err(|error| problem("Replay Plugin Create", error));
        }
        if self.registry.entries.contains_key(&request.name) {
            return Err(Error::new("entry_exists", "Create Plugin", "choose an unused entry name"));
        }
        let receipts = self.registry.creates.len()
            + self.registry.entries.values().map(|entry| entry.settings_receipts.len() + entry.replacements.len()).sum::<usize>();
        if self.registry.entries.len() >= self.settings.entries.get() || receipts >= self.settings.create_receipts.get() {
            return Err(Error::new("manager_capacity", "Create Plugin", "configure capacity for entries and retained mutation receipts"));
        }
        let placement = request
            .placement
            .as_ref()
            .and_then(|placement| placement.kind.as_ref())
            .ok_or_else(|| Error::new("placement_required", "Create Plugin", "choose remote or managed placement"))?;
        let value = request.value.ok_or_else(|| Error::new("value_required", "Create Plugin", "supply the complete writable value"))?;
        let installation = match placement {
            wire::plugin_placement::Kind::Remote(remote) => {
                let endpoint =
                    tonic::transport::Endpoint::from_shared(remote.url.clone()).map_err(|error| problem("Create Remote Plugin", error))?;
                let uri = endpoint.uri();
                if uri.scheme_str() != Some("http")
                    || uri.host().is_none()
                    || uri.authority().is_none_or(|authority| authority.as_str().contains('@'))
                {
                    return Err(Error::new("invalid_endpoint", "Create Remote Plugin", "supply a plain gRPC URL without credentials"));
                }
                None
            }
            wire::plugin_placement::Kind::Guest(_) => {
                return Err(Error::new(
                    "host_registration_required",
                    "Create Guest Plugin",
                    "register the loaded module through PluginHost.register_guest",
                ));
            }
            wire::plugin_placement::Kind::Managed(managed) => {
                Some(Installation::prepare(&self.store.storage, managed, &self.settings).await?)
            }
        };
        let mut registry = self.registry.clone();
        let entry = PluginEntry {
            name: request.name.clone(),
            placement: request.placement.clone(),
            value: Some(value),
            revision: registry.revision()?,
            state: if value.enabled { PluginState::Connecting } else { PluginState::Stopped } as i32,
            failure: None,
            description: None,
            declarations_digest: vec![],
            process: None,
            activation: u64::from(value.enabled),
        };
        let response = PluginsCreateResponse {
            entry: Some(entry.clone()),
            receipt: Some(self.receipt(request.request_id.clone())),
        };
        registry.entries.insert(
            entry.name.clone(),
            SavedEntry {
                entry: entry.encode_to_vec(),
                executable: installation.as_ref().map(|installation| installation.executable().to_owned()),
                executable_digest: installation.as_ref().map(Installation::digest),
                generation: entry.activation,
                settings: Vec::new(),
                settings_revision: Vec::new(),
                settings_receipts: BTreeMap::new(),
                replacements: BTreeMap::new(),
            },
        );
        registry.creates.insert(
            request.request_id,
            SavedMutation {
                request: encoded,
                response: response.encode_to_vec(),
            },
        );
        self.commit(registry, changed(entry.clone())).await?;
        if let Some(installation) = installation {
            installation.commit();
        }
        self.start(&entry.name);
        Ok(response)
    }

    pub(super) fn entry(&self, name: &str, revision: &[u8], action: &str) -> Result<PluginEntry, Error> {
        let entry = self.snapshots.back().and_then(|snapshot| snapshot.entries.get(name)).cloned().ok_or_else(|| {
            Error::new("entry_missing", format!("{action}: Entry Does Not Exist"), "list plugins and select an existing entry")
        })?;
        if entry.revision != revision {
            return Err(Error::new(
                "revision_conflict",
                format!("{action}: Revision Conflict"),
                "get again and retry with the current revision",
            ));
        }
        Ok(entry)
    }

    async fn set(&mut self, request: PluginsSetRequest) -> Result<PluginsSetResponse, Error> {
        let mut entry = self.entry(&request.name, &request.expected_revision, "Set Plugin")?;
        let value = request.value.ok_or_else(|| Error::new("value_required", "Set Plugin", "supply the complete writable value"))?;
        if entry.value.as_ref() == Some(&value) && entry.state != PluginState::Stopping as i32 {
            let position = v1::Position {
                history: Some(self.history.clone()),
                sequence: self.sequence,
            };
            self.issued.insert(position.sequence);
            return Ok(PluginsSetResponse {
                entry: Some(entry),
                receipt: Some(v1::Receipt {
                    request_id: String::new(),
                    accepted_at: Some(position.clone()),
                    durable_at: Some(position),
                    completed: true,
                }),
            });
        }
        let mut registry = self.registry.clone();
        entry.value = Some(value);
        entry.revision = registry.revision()?;
        if value.enabled {
            entry.process = None;
        }
        entry.failure = None;
        entry.activation = next_activation(entry.activation)?;
        entry.state = if value.enabled { PluginState::Connecting } else { PluginState::Stopping } as i32;
        registry.entries.get_mut(&entry.name).expect("entry exists").generation = entry.activation;
        registry.entries.get_mut(&entry.name).expect("entry exists").entry = entry.encode_to_vec();
        let mut response = PluginsSetResponse {
            entry: Some(entry.clone()),
            receipt: Some(self.receipt(String::new())),
        };
        self.commit(registry, changed(entry.clone())).await?;
        let stopped = if is_guest(&entry) && !value.enabled {
            match &self.guest_control {
                Some(control) => control(entry.name.clone(), GuestAction::Stop).await,
                None => Ok(()),
            }
        } else {
            self.stop(&entry.name).await
        };
        if !value.enabled {
            entry.state = PluginState::Stopped as i32;
            entry.process = None;
            entry.failure = stopped.err().map(|error| *error.0);
            let mut registry = self.registry.clone();
            registry.entries.get_mut(&entry.name).expect("entry exists").entry = entry.encode_to_vec();
            self.commit(registry, changed(entry.clone())).await?;
            response.entry = Some(entry.clone());
            response.receipt.as_mut().expect("mutation has a receipt").durable_at = self.cut().positions.into_iter().next();
            self.issued.insert(self.sequence);
        } else if let Err(error) = stopped {
            response.entry = Some(self.cleanup_failure(&entry.name, error).await?);
        }
        self.start(&entry.name);
        Ok(response)
    }

    async fn restart(&mut self, request: PluginsRestartRequest) -> Result<PluginsRestartResponse, Error> {
        let mut entry = self.entry(&request.name, &request.expected_revision, "Restart Plugin")?;
        if !entry.value.as_ref().is_some_and(|value| value.enabled) {
            return Err(Error::new("plugin_disabled", "Restart Plugin", "enable the entry with Set before restarting it"));
        }
        entry.activation = next_activation(entry.activation)?;
        entry.state = PluginState::Connecting as i32;
        entry.failure = None;
        entry.process = None;
        let mut registry = self.registry.clone();
        registry.entries.get_mut(&entry.name).expect("entry exists").generation = entry.activation;
        registry.entries.get_mut(&entry.name).expect("entry exists").entry = entry.encode_to_vec();
        self.commit(registry, changed(entry.clone())).await?;
        if let Err(error) = self.stop(&entry.name).await {
            entry = self.cleanup_failure(&entry.name, error).await?;
        }
        self.start(&entry.name);
        Ok(PluginsRestartResponse { entry: Some(entry) })
    }

    async fn delete(&mut self, request: PluginsDeleteRequest) -> Result<PluginsDeleteResponse, Error> {
        let entry = self.entry(&request.name, &request.expected_revision, "Delete Plugin")?;
        let mut registry = self.registry.clone();
        let saved = registry.entries.remove(&entry.name).expect("entry exists");
        let response = PluginsDeleteResponse {
            receipt: Some(self.receipt(String::new())),
        };
        self.commit(
            registry,
            PluginChange {
                kind: Some(wire::plugin_change::Kind::Deleted(entry.name.clone())),
            },
        )
        .await?;
        let stopped = if is_guest(&entry) {
            match &self.guest_control {
                Some(control) => control(entry.name.clone(), GuestAction::Remove).await,
                None => Ok(()),
            }
        } else {
            self.stop(&entry.name).await
        };
        if let Some(executable) = saved.executable {
            super::content::remove(&executable, &self.settings)?;
        }
        stopped?;
        Ok(response)
    }

    fn start(&mut self, name: &str) {
        let entry = self.snapshots.back().and_then(|snapshot| snapshot.entries.get(name)).expect("persisted entry exists");
        if !entry.value.as_ref().is_some_and(|value| value.enabled) {
            return;
        }
        let saved = self.registry.entries.get(name).expect("persisted entry exists");
        if is_guest(entry) {
            if let Some(control) = &self.guest_control {
                let future = control(name.to_owned(), GuestAction::Restart);
                tokio::spawn(async move {
                    if let Err(error) = future.await {
                        tracing::error!(%error, "guest entry activation failed");
                    }
                });
            }
            return;
        }
        let placement = match entry.placement.as_ref().and_then(|placement| placement.kind.as_ref()) {
            Some(wire::plugin_placement::Kind::Remote(remote)) => Ok(Placement::Remote(remote.url.clone())),
            Some(wire::plugin_placement::Kind::Managed(_)) => managed(saved, &self.settings),
            Some(wire::plugin_placement::Kind::Guest(_)) => unreachable!("guest handled above"),
            None => Err(Error::new("placement_required", "Start Plugin", "restore a valid placement")),
        };
        let values = (!saved.settings.is_empty())
            .then(|| v1::SettingValues::decode(saved.settings.as_slice()))
            .transpose()
            .map_err(|error| problem("Restore Plugin Settings", error));
        let attachment = placement.and_then(|placement| values.map(|values| (self.attach)(name.to_owned(), placement, values)));
        self.monitor(name, saved.generation, attachment);
    }
    pub(super) fn monitor(&mut self, name: &str, activation: u64, attachment: Result<Attachment, Error>) {
        let commands = self.commands.clone();
        let name = name.to_owned();
        let (stop, mut stopping) = oneshot::channel();
        let worker_name = name.clone();
        let task = tokio::spawn(async move {
            let attached = match attachment {
                Ok(mut attachment) => tokio::select! {
                    biased;
                    _ = &mut stopping => {
                        if let Ok(attached) = attachment.await {
                            attached.stop("Plugin Entry Stopped During Startup").await.map_err(|error| problem("Stop Starting Plugin", error))?;
                        }
                        return Ok(());
                    },
                    result = &mut attachment => result.map_err(|error| error.to_string()),
                },
                Err(error) => Err(error.to_string()),
            };
            let attached = match attached {
                Ok(attached) => attached,
                Err(error) => {
                    let command = Command::Health {
                        name: worker_name,
                        activation,
                        status: Box::new(Err(error)),
                    };
                    tokio::select! { _ = &mut stopping => {}, _ = commands.send(command) => {} }
                    return Ok(());
                }
            };
            let mut status = attached.status();
            let mut terminal_failure;
            loop {
                let current = status.borrow_and_update().clone();
                terminal_failure = current.activation.is_none() && current.failure.is_some();
                let command = Command::Health {
                    name: worker_name.clone(),
                    activation,
                    status: Box::new(Ok(current)),
                };
                tokio::select! {
                    biased;
                    _ = &mut stopping => break,
                    result = commands.send(command) => if result.is_err() { break; },
                }
                tokio::select! {
                    biased;
                    _ = &mut stopping => break,
                    result = status.changed() => if result.is_err() { break; },
                }
            }
            match attached.stop("Plugin Entry Activation Replaced").await {
                Ok(()) => Ok(()),
                Err(_) if terminal_failure => Ok(()),
                Err(error) => Err(problem("Stop Plugin Activation", error)),
            }
        });
        self.workers.insert(name, Worker { stop, task });
    }

    pub(super) async fn stop(&mut self, name: &str) -> Result<(), Error> {
        if let Some(worker) = self.workers.remove(name) {
            let _ = worker.stop.send(());
            worker.task.await.map_err(|error| problem("Join Plugin Activation", error))??;
        }
        Ok(())
    }

    pub(super) async fn cleanup_failure(&mut self, name: &str, error: Error) -> Result<PluginEntry, Error> {
        let mut entry = self.snapshots.back().and_then(|snapshot| snapshot.entries.get(name)).cloned().ok_or_else(missing)?;
        entry.failure = Some(*error.0);
        let mut registry = self.registry.clone();
        registry.entries.get_mut(name).expect("entry exists").entry = entry.encode_to_vec();
        self.commit(registry, changed(entry.clone())).await?;
        Ok(entry)
    }

    async fn health(&mut self, name: String, activation: u64, status: Result<Status, String>) -> Result<(), Error> {
        let Some(mut entry) = self.snapshots.back().and_then(|snapshot| snapshot.entries.get(&name)).cloned() else {
            return Ok(());
        };
        if self.registry.entries.get(&name).is_none_or(|saved| saved.generation != activation)
            || !entry.value.as_ref().is_some_and(|value| value.enabled)
        {
            return Ok(());
        }
        match status {
            Ok(status) => {
                if let Some(current) = status.activation {
                    entry.activation = activation
                        .checked_add(current.number - 1)
                        .ok_or_else(|| Error::new("activation_exhausted", "Reconnect Plugin", "create a new entry"))?;
                    entry.state = PluginState::Ready as i32;
                    entry.failure = None;
                    entry.declarations_digest = current.description.declarations.as_ref().expect("admitted catalog exists").digest.clone();
                    entry.process = current.process_id.map(|id| PluginProcess {
                        id,
                        executable_digest: current.executable_digest.map(|digest| digest.to_vec()).unwrap_or_default(),
                        started_at: current.description.started_at,
                    });
                    entry.description = Some(PluginDescription {
                        id: current.description.id,
                        version: current.description.version,
                        started_at: current.description.started_at,
                        declarations: current.description.declarations,
                        revision: current.description.revision,
                        sdk: current.description.sdk,
                        sdk_version: current.description.sdk_version,
                    });
                } else {
                    entry.state = PluginState::Unavailable as i32;
                    entry.process = None;
                    entry.failure = Some(unavailable(
                        &name,
                        status.failure.map(|failure| failure.to_string()).unwrap_or_else(|| "plugin activation stopped".into()),
                    ));
                }
            }
            Err(error) => {
                entry.state = PluginState::Unavailable as i32;
                entry.process = None;
                entry.failure = Some(unavailable(&name, error));
            }
        }
        let mut registry = self.registry.clone();
        registry.entries.get_mut(&name).expect("entry exists").entry = entry.encode_to_vec();
        self.commit(registry, changed(entry)).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn is_guest(entry: &PluginEntry) -> bool {
    matches!(entry.placement.as_ref().and_then(|placement| placement.kind.as_ref()), Some(wire::plugin_placement::Kind::Guest(_)))
}

fn missing() -> Error {
    Error::new("entry_missing", "Get Plugin Entry", "list plugins and select an existing entry")
}
pub(super) fn next_activation(value: u64) -> Result<u64, Error> {
    value.checked_add(1).ok_or_else(|| Error::new("activation_exhausted", "Restart Plugin", "create a new entry"))
}
pub(super) fn changed(entry: PluginEntry) -> PluginChange {
    PluginChange {
        kind: Some(wire::plugin_change::Kind::Entry(entry)),
    }
}
pub(super) fn unavailable(name: &str, cause: String) -> v1::Failure {
    v1::Failure {
        classification: v1::Classification::Transient as i32,
        code: "plugin_unavailable".into(),
        message: format!("Activate Plugin: {cause}"),
        action: "correct the reported cause and Restart the plugin".into(),
        plugin: name.into(),
        ..Default::default()
    }
}
async fn watch_event(
    sender: &mpsc::Sender<PluginsWatchResponse>, event: PluginsWatchResponse, max_bytes: usize, resolved_at: &mut Option<v1::ReadCut>,
) -> bool {
    if event.encoded_len() > max_bytes {
        let event = PluginsWatchResponse {
            resolved_at: resolved_at.clone(),
            event: Some(wire::plugins_watch_response::Event::ResyncRequired(CAPACITY_ACTION.into())),
        };
        let _ = sender.send(event).await;
        return false;
    }
    let cut = event.resolved_at.clone();
    if sender.send(event).await.is_err() {
        return false;
    }
    *resolved_at = cut;
    true
}

fn selected(event: &PluginsWatchResponse, name: &str) -> bool {
    if name.is_empty() {
        return true;
    }
    match &event.event {
        Some(wire::plugins_watch_response::Event::Change(change)) => match &change.kind {
            Some(wire::plugin_change::Kind::Entry(entry)) => entry.name == name,
            Some(wire::plugin_change::Kind::Deleted(deleted)) => deleted == name,
            None => false,
        },
        _ => true,
    }
}
pub(super) fn managed(saved: &SavedEntry, settings: &PluginManagerSettings) -> Result<Placement, Error> {
    let executable = saved
        .executable
        .as_ref()
        .ok_or_else(|| Error::new("executable_missing", "Start Managed Plugin", "recreate the entry from a verified bundle"))?;
    crate::bundle::validate_path(executable).map_err(|error| problem("Verify Installed Executable Path", error))?;
    let executable = settings.directory.join(executable);
    let bytes = std::fs::read(&executable).map_err(|error| problem("Read Installed Executable", error))?;
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    if saved.executable_digest != Some(digest) {
        return Err(Error::new(
            "executable_digest",
            "Start Managed Plugin: Installed Executable Was Changed",
            "recreate the entry from the original verified bundle",
        ));
    }
    Ok(Placement::Managed(Managed {
        executable,
        arguments: vec![],
        environment: [("HARMOS_ADDRESS".into(), "127.0.0.1:0".into())].into(),
    }))
}
/* === Tests === */
