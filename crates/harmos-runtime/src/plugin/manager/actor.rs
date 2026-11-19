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

use harmos_protocol::host::PluginDescription;

use harmos_protocol::host::PluginChange;
use harmos_protocol::host::PluginEntry;
use harmos_protocol::host::PluginProcess;
use harmos_protocol::host::PluginSnapshot;
use harmos_protocol::host::PluginState;
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
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    future::Future,
    mem,
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
use tonic::transport::Endpoint;
// Local Imports
use super::{
    Error, PluginManagerSettings,
    content::Installation,
    persistence::{Registry, SavedEntry, SavedMutation, Store, problem},
};
use crate::bundle::{self, Manifest};
use crate::clock::Activity;
use crate::plugin::grpc::{self, Activation, Attached, Managed, Placement, PreparedPlugin, Status};
use crate::runtime::StopParticipant;
use harmos_protocol::host;
use harmos_protocol::model;
use harmos_storage::StorageManager;

/* === Constants === */

pub(super) const CAPACITY_ACTION: &str = "Watch Plugins: event exceeds response capacity; open a bounded selection";
pub(super) const GAP_ACTION: &str = "Watch Plugins: event history expired; open a new snapshot";

/* === Macros === */

/* === Type Aliases === */

pub(super) type GuestControl = Arc<dyn Fn(String, GuestAction) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send>> + Send + Sync>;

type Reply<T> = oneshot::Sender<Result<T, Error>>;
type Attachment = Pin<Box<dyn Future<Output = Result<Attached, grpc::Error>> + Send>>;
pub(super) type Attach = Arc<dyn Fn(String, Placement, Option<model::SettingValues>, oneshot::Receiver<()>) -> Attachment + Send + Sync>;
pub(super) type Stage = Arc<
    dyn Fn(String, Placement, Option<model::SettingValues>) -> Pin<Box<dyn Future<Output = Result<PreparedPlugin, grpc::Error>> + Send>>
        + Send
        + Sync,
>;
pub(super) type Activate = Arc<dyn Fn(PreparedPlugin) -> Attachment + Send + Sync>;

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
    participant: StopParticipant,
    queries: mpsc::Receiver<Command>,
    history: model::History,
    sequence: u64,
    snapshots: VecDeque<Snapshot>,
    pub(super) issued: BTreeSet<u64>,
    changes: broadcast::Sender<PluginsWatchResponse>,
    workers: BTreeMap<String, Worker>,
}

#[derive(Clone)]
pub(super) struct Snapshot {
    pub(super) cut: model::ReadCut,
    entries: BTreeMap<String, PluginEntry>,
    event: Option<PluginChange>,
    pub(super) settings: BTreeMap<String, super::settings::Snapshot>,
}

struct Worker {
    activation: Option<Activation>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), Error>>>,
    failure: Option<String>,
}

/* === Enums === */

pub(super) enum GuestAction {
    Restart,
    Stop,
    Remove,
}

pub(super) enum Command {
    Activation(String, Reply<Activation>),
    Admitted(Box<Activation>, Reply<()>),
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
    SettingsList(host::plugins::SettingsListRequest, Reply<host::plugins::SettingsListResponse>),
    SettingsSet(host::plugins::SettingsSetRequest, Reply<host::plugins::SettingsSetResponse>),
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
        status: Box<Result<Status, Arc<grpc::Error>>>,
    },
}

/* === Traits === */

/* === Implementations === */

impl Command {
    pub(super) fn is_query(&self) -> bool {
        matches!(self, Self::Activation(..) | Self::SettingsList(..) | Self::Get(..) | Self::List(..) | Self::Watch(..))
    }
}

impl Actor {
    pub async fn open(
        storage: StorageManager, settings: PluginManagerSettings, attach: Attach,
        mailboxes: (mpsc::Sender<Command>, mpsc::Receiver<Command>), stage: Stage, activate: Activate, participant: StopParticipant,
    ) -> Result<Self, Error> {
        let (commands, queries) = mailboxes;
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
        let history = model::History {
            store: settings.namespace.clone(),
            epoch: bundle::digest(&epoch),
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
            queries,
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

    pub async fn run(mut self, mut receiver: mpsc::Receiver<Command>, startup: Activity) {
        let names = self.registry.entries.keys().cloned().collect::<Vec<_>>();
        for name in names {
            self.start(&name);
        }
        drop(startup);
        loop {
            let command = tokio::select! {
                biased;
                () = self.participant.stopping() => break,
                command = self.queries.recv() => match command { Some(command) => command, None => break },
                command = receiver.recv() => match command { Some(command) => command, None => break },
            };
            let Some(command) = self.query(command) else {
                continue;
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
                Command::Admitted(activation, reply) => {
                    if !reply.is_closed() {
                        let _ = reply.send(self.admitted(*activation).await);
                    }
                }
                Command::Replace(request, reply) => {
                    let _ = reply.send(self.replace(request).await);
                }
                Command::SettingsSet(request, reply) => {
                    let _ = reply.send(self.settings_set(request).await);
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
                Command::Activation(..) | Command::SettingsList(..) | Command::Get(..) | Command::List(..) | Command::Watch(..) => {
                    unreachable!("queries are handled before mutations");
                }
                Command::Health { name, activation, status } => {
                    if let Err(error) = self.health(name, activation, *status).await {
                        tracing::error!(%error, "plugin state persistence failed; stopping manager");
                        let _ = self.changes.send(PluginsWatchResponse {
                            resolved_at: Some(self.cut()),
                            event: Some(host::plugins_watch_response::Event::ResyncRequired(error.to_string())),
                        });
                        break;
                    }
                }
            }
        }
        receiver.close();
        let workers = mem::take(&mut self.workers);
        for (_, mut worker) in workers {
            if let Some(stop) = worker.stop.take() {
                let _ = stop.send(());
            }
            let Some(task) = worker.task else {
                continue;
            };
            match self.wait(task).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => tracing::warn!(%error, "plugin manager shutdown reported a failure"),
                Err(error) => tracing::error!(%error, "plugin manager settlement task failed"),
            }
        }
    }

    fn query(&mut self, command: Command) -> Option<Command> {
        match command {
            Command::Activation(name, reply) => {
                let activation = self.workers.get(&name).and_then(|worker| worker.activation.clone());
                let result = activation
                    .filter(Activation::is_connected)
                    .ok_or_else(|| Error::new("activation_unavailable", "Get Plugin Activation", "select a ready gRPC plugin entry"));
                let _ = reply.send(result);
            }
            Command::SettingsList(request, reply) => {
                let _ = reply.send(self.settings_list(request));
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
            command => return Some(command),
        }
        None
    }

    pub(super) async fn wait<T>(&mut self, mut task: JoinHandle<T>) -> Result<T, Error> {
        loop {
            tokio::select! {
                result = &mut task => return result.map_err(|error| problem("Join Plugin Transition", error)),
                Some(command) = self.queries.recv() => {
                    assert!(self.query(command).is_none(), "the read mailbox only carries queries");
                }
            }
        }
    }

    pub(super) fn cut(&self) -> model::ReadCut {
        model::ReadCut {
            positions: vec![model::Position {
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
                event: Some(host::plugins_watch_response::Event::Change(event)),
            });
        }
        Ok(())
    }

    pub(super) fn snapshot(&self, at: Option<&model::ReadCut>) -> Result<&Snapshot, Error> {
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
            event: Some(host::plugins_watch_response::Event::Snapshot(PluginSnapshot { entries })),
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
                    event: Some(host::plugins_watch_response::Event::Change(event)),
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
                            let event = PluginsWatchResponse { resolved_at, event: Some(host::plugins_watch_response::Event::ResyncRequired(GAP_ACTION.into())) };
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

    pub(super) fn receipt(&mut self, request_id: String) -> model::Receipt {
        let position = model::Position {
            history: Some(self.history.clone()),
            sequence: self.sequence + 1,
        };
        self.issued.insert(position.sequence);
        model::Receipt {
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
            host::plugin_placement::Kind::Remote(remote) => {
                let endpoint = Endpoint::from_shared(remote.url.clone()).map_err(|error| problem("Create Remote Plugin", error))?;
                let uri = endpoint.uri();
                if uri.scheme_str() != Some("http")
                    || uri.host().is_none()
                    || uri.authority().is_none_or(|authority| authority.as_str().contains('@'))
                {
                    return Err(Error::new("invalid_endpoint", "Create Remote Plugin", "supply a plain gRPC URL without credentials"));
                }
                None
            }
            host::plugin_placement::Kind::Guest(_) => {
                return Err(Error::new(
                    "host_registration_required",
                    "Create Guest Plugin",
                    "register the loaded module through PluginHost.register_guest",
                ));
            }
            host::plugin_placement::Kind::Managed(managed) => {
                Some(Installation::prepare(&self.store.storage, managed, &self.settings).await?)
            }
        };
        let mut registry = self.registry.clone();
        let entry = PluginEntry {
            name: request.name.clone(),
            origin: request.origin.unwrap_or_default(),
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
            let position = model::Position {
                history: Some(self.history.clone()),
                sequence: self.sequence,
            };
            self.issued.insert(position.sequence);
            return Ok(PluginsSetResponse {
                entry: Some(entry),
                receipt: Some(model::Receipt {
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
                Some(control) => {
                    let task = tokio::spawn(control(entry.name.clone(), GuestAction::Stop));
                    self.wait(task).await?
                }
                None => Ok(()),
            }
        } else {
            self.stop(&entry.name).await
        };
        if let Err(error) = stopped {
            let failure = problem("Set Plugin State", &error);
            self.cleanup_failure(&entry.name, error).await?;
            return Err(failure);
        }
        if !value.enabled {
            entry.state = PluginState::Stopped as i32;
            entry.process = None;
            entry.failure = None;
            let mut registry = self.registry.clone();
            registry.entries.get_mut(&entry.name).expect("entry exists").entry = entry.encode_to_vec();
            self.commit(registry, changed(entry.clone())).await?;
            response.entry = Some(entry.clone());
            response.receipt.as_mut().expect("mutation has a receipt").durable_at = self.cut().positions.into_iter().next();
            self.issued.insert(self.sequence);
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
            let failure = problem("Restart Plugin", &error);
            self.cleanup_failure(&entry.name, error).await?;
            return Err(failure);
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
        if is_guest(&entry) {
            if let Some(control) = &self.guest_control {
                let task = tokio::spawn(control(entry.name.clone(), GuestAction::Remove));
                self.wait(task).await??;
            }
        } else {
            self.stop(&entry.name).await?;
        }
        self.commit(
            registry,
            PluginChange {
                kind: Some(host::plugin_change::Kind::Deleted(entry.name.clone())),
            },
        )
        .await?;
        if let Some(executable) = saved.executable {
            super::content::remove(&executable, &self.settings)?;
        }
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
            Some(host::plugin_placement::Kind::Remote(remote)) => Ok(Placement::Remote(remote.url.clone())),
            Some(host::plugin_placement::Kind::Managed(_)) => managed(saved, &self.settings),
            Some(host::plugin_placement::Kind::Guest(_)) => unreachable!("guest handled above"),
            None => Err(Error::new("placement_required", "Start Plugin", "restore a valid placement")),
        };
        let values = (!saved.settings.is_empty())
            .then(|| model::SettingValues::decode(saved.settings.as_slice()))
            .transpose()
            .map_err(|error| problem("Restore Plugin Settings", error));
        let (cancel, cancelled) = oneshot::channel();
        let attachment = placement.and_then(|placement| values.map(|values| (self.attach)(name.to_owned(), placement, values, cancelled)));
        self.monitor(name, saved.generation, attachment, Some(cancel));
    }
    pub(super) fn monitor(
        &mut self, name: &str, activation: u64, attachment: Result<Attachment, Error>, cancel: Option<oneshot::Sender<()>>,
    ) {
        let commands = self.commands.clone();
        let name = name.to_owned();
        let (stop, mut stopping) = oneshot::channel();
        let worker_name = name.clone();
        let task = tokio::spawn(async move {
            let attached = match attachment {
                Ok(mut attachment) => tokio::select! {
                    biased;
                    _ = &mut stopping => {
                        if let Some(cancel) = cancel { let _ = cancel.send(()); }
                        match attachment.await {
                            Ok(attached) => attached.stop("Plugin Entry Stopped During Startup").await.map_err(|error| problem("Stop Starting Plugin", error))?,
                            Err(error @ grpc::Error::Reap(_)) => return Err(problem("Stop Starting Plugin", error)),
                            Err(_) => {},
                        }
                        return Ok(());
                    },
                    result = &mut attachment => result,

                },
                Err(error) => Err(grpc::Error::Invalid(error.to_string())),
            };
            let attached = match attached {
                Ok(attached) => attached,
                Err(error) => {
                    let error = Arc::new(error);
                    let command = Command::Health {
                        name: worker_name,
                        activation,
                        status: Box::new(Err(error.clone())),
                    };
                    tokio::select! { _ = &mut stopping => {}, _ = commands.send(command) => {} }
                    return match error.as_ref() {
                        grpc::Error::Reap(_) => Err(problem("Reap Starting Plugin", error)),
                        _ => Ok(()),
                    };
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
                Err(error) if terminal_failure && !matches!(error.as_ref(), grpc::Error::Reap(_)) => Ok(()),
                Err(error) => Err(problem("Stop Plugin Activation", error)),
            }
        });
        self.workers.insert(
            name,
            Worker {
                activation: None,
                stop: Some(stop),
                task: Some(task),
                failure: None,
            },
        );
    }

    pub(super) async fn stop(&mut self, name: &str) -> Result<(), Error> {
        let Some(mut worker) = self.workers.remove(name) else {
            return Ok(());
        };
        if let Some(stop) = worker.stop.take() {
            let _ = stop.send(());
        }
        if let Some(failure) = &worker.failure {
            let error = problem("Stop Plugin Activation", failure);
            self.workers.insert(name.to_owned(), worker);
            return Err(error);
        }
        let Some(task) = worker.task.take() else {
            return Ok(());
        };
        let result = self.wait(task).await.and_then(|result| result);
        if let Err(error) = result {
            worker.failure = Some(error.to_string());
            self.workers.insert(name.to_owned(), worker);
            return Err(error);
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

    fn describe(&self, entry: &mut PluginEntry, current: Activation) -> Result<(), Error> {
        entry.declarations_digest = current.description.declarations.as_ref().expect("admitted catalog exists").digest.clone();
        entry.process = current.process_id.map(|id| PluginProcess {
            id,
            executable_digest: current.executable_digest.map(|digest| digest.to_vec()).unwrap_or_default(),
            started_at: current.description.started_at,
        });
        let version = match entry.placement.as_ref().and_then(|placement| placement.kind.as_ref()) {
            Some(host::plugin_placement::Kind::Managed(_)) => {
                let saved = self.registry.entries.get(&entry.name).expect("selected entry exists");
                super::content::version(saved, &self.settings)?
            }
            _ => current.description.version,
        };
        entry.description = Some(PluginDescription {
            id: current.description.id,
            version,
            started_at: current.description.started_at,
            declarations: current.description.declarations,
            revision: current.description.revision,
            sdk: current.description.sdk,
            sdk_version: current.description.sdk_version,
        });
        Ok(())
    }

    async fn admitted(&mut self, current: Activation) -> Result<(), Error> {
        let name = current.entry_name.as_ref().expect("entry activation names its owner");
        let mut entry = self.snapshots.back().and_then(|snapshot| snapshot.entries.get(name)).cloned().ok_or_else(missing)?;
        self.describe(&mut entry, current)?;
        let mut registry = self.registry.clone();
        registry.entries.get_mut(&entry.name).expect("admitted entry exists").entry = entry.encode_to_vec();
        self.commit(registry, changed(entry)).await
    }

    async fn health(&mut self, name: String, activation: u64, status: Result<Status, Arc<grpc::Error>>) -> Result<(), Error> {
        let Some(mut entry) = self.snapshots.back().and_then(|snapshot| snapshot.entries.get(&name)).cloned() else {
            return Ok(());
        };
        if self.registry.entries.get(&name).is_none_or(|saved| saved.generation != activation)
            || !entry.value.as_ref().is_some_and(|value| value.enabled)
        {
            return Ok(());
        }
        if let Some(worker) = self.workers.get_mut(&name) {
            worker.activation = status.as_ref().ok().and_then(|status| status.activation.clone());
        }
        match status {
            Ok(status) => {
                if let Some(current) = status.activation {
                    entry.activation = activation
                        .checked_add(current.number - 1)
                        .ok_or_else(|| Error::new("activation_exhausted", "Reconnect Plugin", "create a new entry"))?;
                    entry.state = PluginState::Ready as i32;
                    // A Ready Activation Reports a Stream Whose Demand Failed; Its Other Streams Keep Publishing.
                    entry.failure = status.failure.map(|failure| {
                        failure.lifecycle_failure(&name).unwrap_or_else(|| model::Failure {
                            classification: model::Classification::Permanent as i32,
                            code: "plugin_stream_failed".into(),
                            message: failure.to_string(),
                            action: "follow the reported action, then Restart the plugin".into(),
                            plugin: name.clone(),
                            ..Default::default()
                        })
                    });
                    self.describe(&mut entry, current)?;
                } else {
                    entry.state = PluginState::Unavailable as i32;
                    entry.process = None;
                    entry.failure = Some(match status.failure {
                        Some(failure) => failure.lifecycle_failure(&name).unwrap_or_else(|| unavailable(&name, failure.to_string())),
                        None => unavailable(&name, "plugin activation stopped".into()),
                    });
                }
            }
            Err(error) => {
                entry.state = PluginState::Unavailable as i32;
                entry.process = None;
                entry.failure = Some(error.lifecycle_failure(&name).unwrap_or_else(|| unavailable(&name, error.to_string())));
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
    matches!(entry.placement.as_ref().and_then(|placement| placement.kind.as_ref()), Some(host::plugin_placement::Kind::Guest(_)))
}

fn missing() -> Error {
    Error::new("entry_missing", "Get Plugin Entry", "list plugins and select an existing entry")
}
pub(super) fn next_activation(value: u64) -> Result<u64, Error> {
    value.checked_add(1).ok_or_else(|| Error::new("activation_exhausted", "Restart Plugin", "create a new entry"))
}
pub(super) fn changed(entry: PluginEntry) -> PluginChange {
    PluginChange {
        kind: Some(host::plugin_change::Kind::Entry(entry)),
    }
}
pub(super) fn unavailable(name: &str, cause: String) -> model::Failure {
    model::Failure {
        classification: model::Classification::Transient as i32,
        code: "plugin_unavailable".into(),
        message: format!("Activate Plugin: {cause}"),
        action: "correct the reported cause and Restart the plugin".into(),
        plugin: name.into(),
        ..Default::default()
    }
}
async fn watch_event(
    sender: &mpsc::Sender<PluginsWatchResponse>, event: PluginsWatchResponse, max_bytes: usize, resolved_at: &mut Option<model::ReadCut>,
) -> bool {
    if event.encoded_len() > max_bytes {
        let event = PluginsWatchResponse {
            resolved_at: resolved_at.clone(),
            event: Some(host::plugins_watch_response::Event::ResyncRequired(CAPACITY_ACTION.into())),
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
        Some(host::plugins_watch_response::Event::Change(change)) => match &change.kind {
            Some(host::plugin_change::Kind::Entry(entry)) => entry.name == name,
            Some(host::plugin_change::Kind::Deleted(deleted)) => deleted == name,
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
    bundle::validate_path(executable).map_err(|error| problem("Verify Installed Executable Path", error))?;
    let executable = settings.directory.join(executable);
    let bytes = fs::read(&executable).map_err(|error| problem("Read Installed Executable", error))?;
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    if saved.executable_digest != Some(digest) {
        return Err(Error::new(
            "executable_digest",
            "Start Managed Plugin: Installed Executable Was Changed",
            "recreate the entry from the original verified bundle",
        ));
    }
    if executable.file_name().is_some_and(|name| name == "manifest.json") {
        let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|error| problem("Read Installed Manifest", error))?;
        let launch = manifest
            .launch
            .as_ref()
            .ok_or_else(|| Error::new("launch_missing", "Start Managed Plugin", "reinstall the original command bundle"))?;
        let root = executable.parent().expect("installed manifest has a parent");
        let placement = Managed::launch(launch, root).map_err(|error| {
            Error::new("launch_program", error.to_string(), &format!("install {} at its declared location and retry", launch[0]))
        })?;
        let canonical_root = root.canonicalize().map_err(|error| problem("Resolve Installed Bundle", error))?;
        for file in &manifest.files {
            bundle::validate_path(&file.path).map_err(|error| problem("Verify Installed File Path", error))?;
            let path = root.join(&file.path).canonicalize().map_err(|error| problem("Read Installed File", error))?;
            if !path.starts_with(&canonical_root) {
                return Err(Error::new(
                    "installation_path",
                    "Start Managed Plugin",
                    "reinstall the original bundle inside the manager directory",
                ));
            }
            let bytes = fs::read(path).map_err(|error| problem("Read Installed File", error))?;
            if file.verify(&bytes).is_err() {
                return Err(Error::new(
                    "file_digest",
                    "Start Managed Plugin: Installed File Was Changed",
                    "reinstall the original verified bundle",
                ));
            }
        }
        return Ok(Placement::Managed(placement));
    }
    Ok(Placement::Managed(Managed {
        executable,
        directory: None,
        arguments: vec![],
        environment: [("HARMOS_ADDRESS".into(), "127.0.0.1:0".into())].into(),
    }))
}
/* === Tests === */
