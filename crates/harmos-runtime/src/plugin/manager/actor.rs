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
    ContentSource, Error, PluginManagerSettings,
    content::Installation,
    persistence::{Registry, SavedCreate, SavedEntry, Store, problem},
};
use crate::plugin::grpc::{Attached, Managed, Placement, Status};
use harmos_protocol::{v1 as wire, v1};

/* === Constants === */

pub(super) const CAPACITY_ACTION: &str = "Watch Plugins: event exceeds response capacity; open a bounded selection";
pub(super) const GAP_ACTION: &str = "Watch Plugins: event history expired; open a new snapshot";

/* === Macros === */

/* === Type Aliases === */

type Reply<T> = oneshot::Sender<Result<T, Error>>;
type Attachment = Pin<Box<dyn Future<Output = Result<Attached, crate::plugin::grpc::Error>> + Send>>;
pub(super) type Attach = Arc<dyn Fn(Placement) -> Attachment + Send + Sync>;

/* === Structs === */

pub(super) struct Actor {
    store: Store,
    registry: Registry,
    settings: PluginManagerSettings,
    content: Arc<dyn ContentSource>,
    attach: Attach,
    commands: mpsc::Sender<Command>,
    participant: crate::runtime::StopParticipant,
    history: v1::History,
    sequence: u64,
    snapshots: VecDeque<Snapshot>,
    issued: BTreeSet<u64>,
    changes: broadcast::Sender<wire::PluginsWatchResponse>,
    workers: BTreeMap<String, Worker>,
}

#[derive(Clone)]
struct Snapshot {
    cut: v1::ReadCut,
    entries: BTreeMap<String, wire::PluginEntry>,
    event: Option<wire::PluginChange>,
}

struct Worker {
    stop: oneshot::Sender<()>,
    task: JoinHandle<Result<(), Error>>,
}

/* === Enums === */

pub(super) enum Command {
    Get(wire::PluginsGetRequest, Reply<wire::PluginsGetResponse>),
    List(wire::PluginsListRequest, Reply<wire::PluginsListResponse>),
    Watch(wire::PluginsWatchRequest, Reply<mpsc::Receiver<wire::PluginsWatchResponse>>),
    Create(Box<wire::PluginsCreateRequest>, Reply<wire::PluginsCreateResponse>),
    Set(wire::PluginsSetRequest, Reply<wire::PluginsSetResponse>),
    Delete(wire::PluginsDeleteRequest, Reply<wire::PluginsDeleteResponse>),
    Restart(wire::PluginsRestartRequest, Reply<wire::PluginsRestartResponse>),
    Health { name: String, activation: u64, status: Box<Result<Status, String>> },
}

/* === Traits === */

/* === Implementations === */

impl Actor {
    pub async fn open(
        storage: harmos_storage::StorageManager, content: Arc<dyn ContentSource>, settings: PluginManagerSettings, attach: Attach,
        commands: mpsc::Sender<Command>, participant: crate::runtime::StopParticipant,
    ) -> Result<Self, Error> {
        let (store, mut registry) = Store::open(storage, &settings).await?;
        if registry.entries.len() > settings.entries.get() || registry.creates.len() > settings.create_receipts.get() {
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
                entry.state = wire::PluginState::Connecting as i32;
                entry.activation = next_activation(entry.activation)?;
            } else {
                entry.state = wire::PluginState::Stopped as i32;
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
            registry,
            settings,
            content,
            attach,
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
                        let _ = self.changes.send(wire::PluginsWatchResponse {
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

    fn cut(&self) -> v1::ReadCut {
        v1::ReadCut {
            positions: vec![v1::Position {
                history: Some(self.history.clone()),
                sequence: self.sequence,
            }],
            revisions: vec![],
        }
    }

    fn publish(&mut self, event: Option<wire::PluginChange>) -> Result<(), Error> {
        self.sequence =
            self.sequence.checked_add(1).ok_or_else(|| Error::new("history_exhausted", "Publish Plugin State", "restart the host"))?;
        let cut = self.cut();
        let snapshot = Snapshot {
            cut: cut.clone(),
            entries: self.registry.values()?,
            event: event.clone(),
        };
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
            let _ = self.changes.send(wire::PluginsWatchResponse {
                resolved_at: Some(cut),
                event: Some(wire::plugins_watch_response::Event::Change(event)),
            });
        }
        Ok(())
    }

    fn snapshot(&self, at: Option<&v1::ReadCut>) -> Result<&Snapshot, Error> {
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

    fn get(&mut self, request: wire::PluginsGetRequest) -> Result<wire::PluginsGetResponse, Error> {
        let snapshot = self.snapshot(request.at.as_ref())?;
        let entry = snapshot.entries.get(&request.name).cloned().ok_or_else(missing)?;
        let cut = snapshot.cut.clone();
        let response = wire::PluginsGetResponse {
            entry: Some(entry),
            resolved_at: Some(cut.clone()),
        };
        if response.encoded_len() > self.settings.response_bytes.get() {
            return Err(Error::new("entry_capacity", "Get Plugin Entry", "configure read response capacity for the complete entry"));
        }
        self.issued.insert(cut.positions[0].sequence);
        Ok(response)
    }

    fn list(&mut self, request: wire::PluginsListRequest) -> Result<wire::PluginsListResponse, Error> {
        let snapshot = self.snapshot(request.at.as_ref())?.clone();
        let response = wire::PluginsListResponse {
            entries: snapshot.entries.into_values().collect(),
            resolved_at: Some(snapshot.cut.clone()),
        };
        self.issued.insert(snapshot.cut.positions[0].sequence);
        Ok(response)
    }

    fn watch(&mut self, request: wire::PluginsWatchRequest) -> Result<mpsc::Receiver<wire::PluginsWatchResponse>, Error> {
        let snapshot = self.snapshot(request.at.as_ref())?.clone();
        let entries = snapshot.entries.values().filter(|entry| request.name.is_empty() || entry.name == request.name).cloned().collect();
        let opening = wire::PluginsWatchResponse {
            resolved_at: Some(snapshot.cut.clone()),
            event: Some(wire::plugins_watch_response::Event::Snapshot(wire::PluginSnapshot { entries })),
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
                snapshot.event.clone().map(|event| wire::PluginsWatchResponse {
                    resolved_at: Some(snapshot.cut.clone()),
                    event: Some(wire::plugins_watch_response::Event::Change(event)),
                })
            })
            .collect::<Vec<_>>();
        let mut changes = self.changes.subscribe();
        let (sender, receiver) = mpsc::channel(self.settings.watch_capacity.get());
        let name = request.name;
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
                            let event = wire::PluginsWatchResponse { resolved_at, event: Some(wire::plugins_watch_response::Event::ResyncRequired(GAP_ACTION.into())) };
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

    async fn commit(&mut self, registry: Registry, change: wire::PluginChange) -> Result<(), Error> {
        self.store.save(&registry).await?;
        self.registry = registry;
        self.publish(Some(change))
    }

    fn receipt(&mut self, request_id: String) -> v1::Receipt {
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

    async fn create(&mut self, request: wire::PluginsCreateRequest) -> Result<wire::PluginsCreateResponse, Error> {
        if request.request_id.is_empty() || request.name.is_empty() {
            return Err(Error::new("identity_required", "Create Plugin", "supply a nonempty request identifier and entry name"));
        }
        let encoded = request.encode_to_vec();
        if let Some(original) = self.registry.creates.get(&request.request_id) {
            if original.request != encoded {
                return Err(Error::new("request_conflict", "Create Plugin", "resend the original payload or use a new request identifier"));
            }
            return wire::PluginsCreateResponse::decode(original.response.as_slice())
                .map_err(|error| problem("Replay Plugin Create", error));
        }
        if self.registry.entries.contains_key(&request.name) {
            return Err(Error::new("entry_exists", "Create Plugin", "choose an unused entry name"));
        }
        if self.registry.entries.len() == self.settings.entries.get() || self.registry.creates.len() == self.settings.create_receipts.get()
        {
            return Err(Error::new("manager_capacity", "Create Plugin", "configure capacity for entries and retained Create receipts"));
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
            wire::plugin_placement::Kind::Managed(managed) => {
                Some(Installation::prepare(self.content.as_ref(), managed, &self.settings).await?)
            }
        };
        let mut registry = self.registry.clone();
        let entry = wire::PluginEntry {
            name: request.name.clone(),
            placement: request.placement.clone(),
            value: Some(value),
            revision: registry.revision()?,
            state: if value.enabled { wire::PluginState::Connecting } else { wire::PluginState::Stopped } as i32,
            failure: None,
            description: None,
            catalog_digest: vec![],
            process: None,
            activation: u64::from(value.enabled),
        };
        let response = wire::PluginsCreateResponse {
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
            },
        );
        registry.creates.insert(
            request.request_id,
            SavedCreate {
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

    fn entry(&self, name: &str, revision: &[u8], action: &str) -> Result<wire::PluginEntry, Error> {
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

    async fn set(&mut self, request: wire::PluginsSetRequest) -> Result<wire::PluginsSetResponse, Error> {
        let mut entry = self.entry(&request.name, &request.expected_revision, "Set Plugin")?;
        let value = request.value.ok_or_else(|| Error::new("value_required", "Set Plugin", "supply the complete writable value"))?;
        if entry.value.as_ref() == Some(&value) && entry.state != wire::PluginState::Stopping as i32 {
            let position = v1::Position {
                history: Some(self.history.clone()),
                sequence: self.sequence,
            };
            self.issued.insert(position.sequence);
            return Ok(wire::PluginsSetResponse {
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
        entry.state = if value.enabled { wire::PluginState::Connecting } else { wire::PluginState::Stopping } as i32;
        registry.entries.get_mut(&entry.name).expect("entry exists").generation = entry.activation;
        registry.entries.get_mut(&entry.name).expect("entry exists").entry = entry.encode_to_vec();
        let mut response = wire::PluginsSetResponse {
            entry: Some(entry.clone()),
            receipt: Some(self.receipt(String::new())),
        };
        self.commit(registry, changed(entry.clone())).await?;
        let stopped = self.stop(&entry.name).await;
        if !value.enabled {
            entry.state = wire::PluginState::Stopped as i32;
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

    async fn restart(&mut self, request: wire::PluginsRestartRequest) -> Result<wire::PluginsRestartResponse, Error> {
        let mut entry = self.entry(&request.name, &request.expected_revision, "Restart Plugin")?;
        if !entry.value.as_ref().is_some_and(|value| value.enabled) {
            return Err(Error::new("plugin_disabled", "Restart Plugin", "enable the entry with Set before restarting it"));
        }
        entry.activation = next_activation(entry.activation)?;
        entry.state = wire::PluginState::Connecting as i32;
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
        Ok(wire::PluginsRestartResponse { entry: Some(entry) })
    }

    async fn delete(&mut self, request: wire::PluginsDeleteRequest) -> Result<wire::PluginsDeleteResponse, Error> {
        let entry = self.entry(&request.name, &request.expected_revision, "Delete Plugin")?;
        let mut registry = self.registry.clone();
        let saved = registry.entries.remove(&entry.name).expect("entry exists");
        let response = wire::PluginsDeleteResponse {
            receipt: Some(self.receipt(String::new())),
        };
        self.commit(
            registry,
            wire::PluginChange {
                kind: Some(wire::plugin_change::Kind::Deleted(entry.name.clone())),
            },
        )
        .await?;
        let stopped = self.stop(&entry.name).await;
        if let Some(executable) = saved.executable {
            crate::bundle::validate_path(&executable).map_err(|error| problem("Delete Plugin Installation", error))?;
            let parent = executable.split('/').next().expect("installation path has a parent");
            let root = self.settings.directory.canonicalize().map_err(|error| problem("Resolve Plugin Directory", error))?;
            let directory = root.join(parent).canonicalize().map_err(|error| problem("Resolve Plugin Installation", error))?;
            if directory.parent() != Some(root.as_path()) {
                return Err(Error::new(
                    "installation_path",
                    "Delete Plugin Installation",
                    "restore an installation directly beneath the manager directory",
                ));
            }
            std::fs::remove_dir_all(directory).map_err(|error| problem("Delete Plugin Installation", error))?;
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
        let placement = match entry.placement.as_ref().and_then(|placement| placement.kind.as_ref()) {
            Some(wire::plugin_placement::Kind::Remote(remote)) => Ok(Placement::Remote(remote.url.clone())),
            Some(wire::plugin_placement::Kind::Managed(_)) => managed(saved, &self.settings),
            None => Err(Error::new("placement_required", "Start Plugin", "restore a valid placement")),
        };
        let attachment = placement.map(|placement| (self.attach)(placement));
        let commands = self.commands.clone();
        let name = name.to_owned();
        let activation = saved.generation;
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

    async fn stop(&mut self, name: &str) -> Result<(), Error> {
        if let Some(worker) = self.workers.remove(name) {
            let _ = worker.stop.send(());
            worker.task.await.map_err(|error| problem("Join Plugin Activation", error))??;
        }
        Ok(())
    }

    async fn cleanup_failure(&mut self, name: &str, error: Error) -> Result<wire::PluginEntry, Error> {
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
                    entry.state = wire::PluginState::Ready as i32;
                    entry.failure = None;
                    entry.catalog_digest = current.description.catalog.as_ref().expect("admitted catalog exists").digest.clone();
                    entry.process = current.process_id.map(|id| wire::PluginProcess {
                        id,
                        executable_digest: current.executable_digest.map(|digest| digest.to_vec()).unwrap_or_default(),
                        started_at: current.description.started_at,
                    });
                    entry.description = Some(wire::PluginDescription {
                        id: current.description.id,
                        version: current.description.version,
                        started_at: current.description.started_at,
                        catalog: current.description.catalog,
                        revision: current.description.revision,
                        sdk: current.description.sdk,
                        sdk_version: current.description.sdk_version,
                    });
                } else {
                    entry.state = wire::PluginState::Unavailable as i32;
                    entry.process = None;
                    entry.failure = Some(unavailable(
                        &name,
                        status.failure.map(|failure| failure.to_string()).unwrap_or_else(|| "plugin activation stopped".into()),
                    ));
                }
            }
            Err(error) => {
                entry.state = wire::PluginState::Unavailable as i32;
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

fn missing() -> Error {
    Error::new("entry_missing", "Get Plugin Entry", "list plugins and select an existing entry")
}
fn next_activation(value: u64) -> Result<u64, Error> {
    value.checked_add(1).ok_or_else(|| Error::new("activation_exhausted", "Restart Plugin", "create a new entry"))
}
fn changed(entry: wire::PluginEntry) -> wire::PluginChange {
    wire::PluginChange {
        kind: Some(wire::plugin_change::Kind::Entry(entry)),
    }
}
fn unavailable(name: &str, cause: String) -> v1::Failure {
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
    sender: &mpsc::Sender<wire::PluginsWatchResponse>, event: wire::PluginsWatchResponse, max_bytes: usize,
    resolved_at: &mut Option<v1::ReadCut>,
) -> bool {
    if event.encoded_len() > max_bytes {
        let event = wire::PluginsWatchResponse {
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

fn selected(event: &wire::PluginsWatchResponse, name: &str) -> bool {
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
fn managed(saved: &SavedEntry, settings: &PluginManagerSettings) -> Result<Placement, Error> {
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
