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
use std::{collections::{BTreeMap, VecDeque}, future::Future, pin::Pin, sync::Arc};
// Third Party Imports
use prost::Message;
use sha2::{Digest, Sha256};
use tokio::{sync::{broadcast, mpsc, oneshot}, task::JoinHandle};
// Local Imports
use super::{ContentSource, Error, PluginManagerSettings, content::Installation, persistence::{Registry, SavedCreate, SavedEntry, Store, problem}};
use crate::plugin::grpc::{Attached, Managed, Placement, Status};
use harmos_protocol::{host as wire, v1};

/* === Constants === */

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
    task: JoinHandle<()>,
}

/* === Enums === */

pub(super) enum Command {
    Get(wire::PluginsGetRequest, Reply<wire::PluginsGetResponse>),
    List(wire::PluginsListRequest, Reply<wire::PluginsListResponse>),
    Watch(wire::PluginsWatchRequest, Reply<mpsc::Receiver<wire::PluginsWatchResponse>>),
    Create(wire::PluginsCreateRequest, Reply<wire::PluginsCreateResponse>),
    Set(wire::PluginsSetRequest, Reply<wire::PluginsSetResponse>),
    Delete(wire::PluginsDeleteRequest, Reply<wire::PluginsDeleteResponse>),
    Restart(wire::PluginsRestartRequest, Reply<wire::PluginsRestartResponse>),
    Health { name: String, activation: u64, status: Box<Result<Status, String>> },
}

/* === Traits === */

/* === Implementations === */

impl Actor {
    pub async fn open(
        storage: harmos_storage::StorageManager, content: Arc<dyn ContentSource>, settings: PluginManagerSettings,
        attach: Attach, commands: mpsc::Sender<Command>, participant: crate::runtime::StopParticipant,
    ) -> Result<Self, Error> {
        let (store, mut registry) = Store::open(storage, &settings).await?;
        if registry.entries.len() > settings.entries.get() || registry.creates.len() > settings.create_receipts.get() {
            return Err(Error::new("registry_capacity", "Restore Plugin Registry", "configure capacity for the persisted entries and replay receipts"));
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
            registry.entries.get_mut(name).expect("restored entry exists").entry = entry.encode_to_vec();
        }
        let mut epoch = [0_u8; 16];
        getrandom::fill(&mut epoch).map_err(|error| problem("Create Plugin Read History", error))?;
        let history = v1::History { store: settings.namespace.clone(), epoch: crate::bundle::digest(&epoch) };
        let changes = broadcast::channel(settings.watch_capacity.get()).0;
        let mut actor = Self { store, registry, settings, content, attach, commands, participant, history, sequence: 0,
            snapshots: VecDeque::new(), changes, workers: BTreeMap::new() };
        actor.store.save(&actor.registry).await?;
        actor.publish(None)?;
        Ok(actor)
    }

    pub async fn run(mut self, mut receiver: mpsc::Receiver<Command>) {
        let names = self.registry.entries.keys().cloned().collect::<Vec<_>>();
        for name in names { self.start(&name); }
        loop {
            let command = tokio::select! {
                biased;
                () = self.participant.stopping() => break,
                command = receiver.recv() => match command { Some(command) => command, None => break },
            };
            match command {
                Command::Get(request, reply) => { let _ = reply.send(self.get(request)); },
                Command::List(request, reply) => { let _ = reply.send(self.list(request)); },
                Command::Watch(request, reply) => { let _ = reply.send(self.watch(request)); },
                Command::Create(request, reply) => { let _ = reply.send(self.create(request).await); },
                Command::Set(request, reply) => { let _ = reply.send(self.set(request).await); },
                Command::Delete(request, reply) => { let _ = reply.send(self.delete(request).await); },
                Command::Restart(request, reply) => { let _ = reply.send(self.restart(request).await); },
                Command::Health { name, activation, status } => {
                    if let Err(error) = self.health(name, activation, *status).await {
                        tracing::error!(%error, "plugin state persistence failed; stopping manager");
                        let _ = self.changes.send(wire::PluginsWatchResponse { resolved_at: Some(self.cut()), event: Some(wire::plugins_watch_response::Event::ResyncRequired(error.to_string())) });
                        break;
                    }
                },
            }
        }
        receiver.close();
        let workers = std::mem::take(&mut self.workers);
        for (_, worker) in workers {
            let _ = worker.stop.send(());
            let _ = worker.task.await;
        }
    }

    fn cut(&self) -> v1::ReadCut {
        v1::ReadCut { positions: vec![v1::Position { history: Some(self.history.clone()), sequence: self.sequence }], revisions: vec![] }
    }

    fn publish(&mut self, event: Option<wire::PluginChange>) -> Result<(), Error> {
        self.sequence = self.sequence.checked_add(1).ok_or_else(|| Error::new("history_exhausted", "Publish Plugin State", "restart the host"))?;
        let cut = self.cut();
        let snapshot = Snapshot { cut: cut.clone(), entries: self.registry.values()?, event: event.clone() };
        self.snapshots.push_back(snapshot);
        while self.snapshots.len() > self.settings.retained_cuts.get() { self.snapshots.pop_front(); }
        if let Some(event) = event {
            let _ = self.changes.send(wire::PluginsWatchResponse { resolved_at: Some(cut), event: Some(wire::plugins_watch_response::Event::Change(event)) });
        }
        Ok(())
    }

    fn snapshot(&self, at: Option<&v1::ReadCut>) -> Result<&Snapshot, Error> {
        match at {
            Some(at) => self.snapshots.iter().find(|snapshot| &snapshot.cut == at).ok_or_else(|| Error::new("position_unavailable", "Read Plugin Entries", "open a new read; the supplied cut is expired or foreign")),
            None => self.snapshots.back().ok_or_else(|| Error::new("not_started", "Read Plugin Entries", "start the manager")),
        }
    }

    fn get(&self, request: wire::PluginsGetRequest) -> Result<wire::PluginsGetResponse, Error> {
        let snapshot = self.snapshot(request.at.as_ref())?;
        let entry = snapshot.entries.get(&request.name).cloned().ok_or_else(missing)?;
        Ok(wire::PluginsGetResponse { entry: Some(entry), resolved_at: Some(snapshot.cut.clone()) })
    }

    fn list(&self, request: wire::PluginsListRequest) -> Result<wire::PluginsListResponse, Error> {
        let page = request.page.ok_or_else(|| Error::new("page_required", "List Plugins", "supply page_size and max_bytes"))?;
        if page.page_size == 0 || page.page_size as usize > self.settings.page_size.get() || page.max_bytes == 0 || page.max_bytes > self.settings.response_bytes.get() as u64 {
            return Err(Error::new("page_capacity", "List Plugins", "request a positive page within the configured row and byte capacities"));
        }
        let snapshot = self.snapshot(request.at.as_ref())?;
        let after = if page.page_token.is_empty() { String::new() } else {
            let token: (v1::ReadCut, u32, u64, String) = decode_token(&page.page_token)?;
            if token.0 != snapshot.cut || token.1 != page.page_size || token.2 != page.max_bytes {
                return Err(Error::new("page_mismatch", "List Plugins", "return the original cut, page_size, max_bytes and token unchanged"));
            }
            token.3
        };
        let mut response = wire::PluginsListResponse { entries: vec![], resolved_at: Some(snapshot.cut.clone()), next_page_token: String::new() };
        let mut selected = snapshot.entries.iter().filter(|(name, _)| *name > &after).peekable();
        while let Some((name, entry)) = selected.next() {
            response.entries.push(entry.clone());
            if selected.peek().is_some() {
                response.next_page_token = encode_token(&(snapshot.cut.clone(), page.page_size, page.max_bytes, name.clone()));
            } else { response.next_page_token.clear(); }
            if response.encoded_len() > page.max_bytes as usize {
                response.entries.pop();
                let Some(last) = response.entries.last() else {
                    return Err(Error::new("entry_capacity", "List Plugins", "request enough bytes for one entry and its continuation token"));
                };
                response.next_page_token = encode_token(&(snapshot.cut.clone(), page.page_size, page.max_bytes, last.name.clone()));
                break;
            }
            if response.entries.len() == page.page_size as usize { break; }
        }
        Ok(response)
    }

    fn watch(&self, request: wire::PluginsWatchRequest) -> Result<mpsc::Receiver<wire::PluginsWatchResponse>, Error> {
        let snapshot = self.snapshot(request.at.as_ref())?;
        let entries = snapshot.entries.values().filter(|entry| request.name.is_empty() || entry.name == request.name).cloned().collect();
        let opening = wire::PluginsWatchResponse { resolved_at: Some(snapshot.cut.clone()), event: Some(wire::plugins_watch_response::Event::Snapshot(wire::PluginSnapshot { entries })) };
        if opening.encoded_len() > self.settings.response_bytes.get() {
            return Err(Error::new("watch_capacity", "Watch Plugins", "select one entry or configure enough response bytes for the opening snapshot"));
        }
        let sequence = snapshot.cut.positions[0].sequence;
        let replay = self.snapshots.iter().filter(|snapshot| snapshot.cut.positions[0].sequence > sequence)
            .filter_map(|snapshot| snapshot.event.clone().map(|event| wire::PluginsWatchResponse { resolved_at: Some(snapshot.cut.clone()), event: Some(wire::plugins_watch_response::Event::Change(event)) })).collect::<Vec<_>>();
        let mut changes = self.changes.subscribe();
        let (sender, receiver) = mpsc::channel(self.settings.watch_capacity.get());
        let name = request.name;
        tokio::spawn(async move {
            if sender.send(opening).await.is_err() { return; }
            for event in replay {
                if selected(&event, &name) && sender.send(event).await.is_err() { return; }
            }
            loop {
                tokio::select! {
                    biased;
                    () = sender.closed() => return,
                    event = changes.recv() => match event {
                        Ok(event) if selected(&event, &name) => if sender.send(event).await.is_err() { return; },
                        Ok(_) => {},
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            let event = wire::PluginsWatchResponse { resolved_at: None, event: Some(wire::plugins_watch_response::Event::ResyncRequired("Watch Plugins: event history expired; open a new snapshot".into())) };
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

    fn receipt(&self, request_id: String) -> v1::Receipt {
        let position = v1::Position { history: Some(self.history.clone()), sequence: self.sequence + 1 };
        v1::Receipt { request_id, accepted_at: Some(position.clone()), durable_at: Some(position), completed: true }
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
            return wire::PluginsCreateResponse::decode(original.response.as_slice()).map_err(|error| problem("Replay Plugin Create", error));
        }
        if self.registry.entries.contains_key(&request.name) {
            return Err(Error::new("entry_exists", "Create Plugin", "choose an unused entry name"));
        }
        if self.registry.entries.len() == self.settings.entries.get() || self.registry.creates.len() == self.settings.create_receipts.get() {
            return Err(Error::new("manager_capacity", "Create Plugin", "configure capacity for entries and retained Create receipts"));
        }
        let placement = request.placement.as_ref().and_then(|placement| placement.kind.as_ref()).ok_or_else(|| Error::new("placement_required", "Create Plugin", "choose remote or managed placement"))?;
        let value = request.value.clone().ok_or_else(|| Error::new("value_required", "Create Plugin", "supply the complete writable value"))?;
        let installation = match placement {
            wire::plugin_placement::Kind::Remote(remote) => {
                let endpoint = tonic::transport::Endpoint::from_shared(remote.url.clone()).map_err(|error| problem("Create Remote Plugin", error))?;
                let uri = endpoint.uri();
                if uri.scheme_str() != Some("http") || uri.host().is_none() || uri.authority().is_none_or(|authority| authority.as_str().contains('@')) {
                    return Err(Error::new("invalid_endpoint", "Create Remote Plugin", "supply a plain gRPC URL without credentials"));
                }
                None
            },
            wire::plugin_placement::Kind::Managed(managed) => Some(Installation::prepare(self.content.as_ref(), managed, &self.settings).await?),
        };
        let mut registry = self.registry.clone();
        let entry = wire::PluginEntry {
            name: request.name.clone(), placement: request.placement.clone(), value: Some(value.clone()), revision: registry.revision()?,
            state: if value.enabled { wire::PluginState::Connecting } else { wire::PluginState::Stopped } as i32,
            failure: None, description: None, catalog_digest: vec![], process: None, activation: u64::from(value.enabled),
        };
        let response = wire::PluginsCreateResponse { entry: Some(entry.clone()), receipt: Some(self.receipt(request.request_id.clone())) };
        registry.entries.insert(entry.name.clone(), SavedEntry { entry: entry.encode_to_vec(), executable: installation.as_ref().map(|installation| installation.executable().to_owned()), executable_digest: installation.as_ref().map(Installation::digest) });
        registry.creates.insert(request.request_id, SavedCreate { request: encoded, response: response.encode_to_vec() });
        self.commit(registry, changed(entry.clone())).await?;
        if let Some(installation) = installation { installation.commit(); }
        self.start(&entry.name);
        Ok(response)
    }

    fn entry(&self, name: &str, revision: &[u8]) -> Result<wire::PluginEntry, Error> {
        let entry = self.snapshots.back().and_then(|snapshot| snapshot.entries.get(name)).cloned().ok_or_else(missing)?;
        if entry.revision != revision {
            return Err(Error::new("revision_conflict", "Change Plugin Entry", "get again and retry with the current revision"));
        }
        Ok(entry)
    }

    async fn set(&mut self, request: wire::PluginsSetRequest) -> Result<wire::PluginsSetResponse, Error> {
        let mut entry = self.entry(&request.name, &request.expected_revision)?;
        let value = request.value.ok_or_else(|| Error::new("value_required", "Set Plugin", "supply the complete writable value"))?;
        let mut registry = self.registry.clone();
        entry.value = Some(value.clone());
        entry.revision = registry.revision()?;
        entry.process = None;
        entry.failure = None;
        entry.activation = next_activation(entry.activation)?;
        entry.state = if value.enabled { wire::PluginState::Connecting } else { wire::PluginState::Stopped } as i32;
        registry.entries.get_mut(&entry.name).expect("entry exists").entry = entry.encode_to_vec();
        let response = wire::PluginsSetResponse { entry: Some(entry.clone()), receipt: Some(self.receipt(String::new())) };
        self.commit(registry, changed(entry.clone())).await?;
        self.stop(&entry.name).await;
        self.start(&entry.name);
        Ok(response)
    }

    async fn restart(&mut self, request: wire::PluginsRestartRequest) -> Result<wire::PluginsRestartResponse, Error> {
        let mut entry = self.entry(&request.name, &request.expected_revision)?;
        if !entry.value.as_ref().is_some_and(|value| value.enabled) {
            return Err(Error::new("plugin_disabled", "Restart Plugin", "enable the entry with Set before restarting it"));
        }
        entry.activation = next_activation(entry.activation)?;
        entry.state = wire::PluginState::Connecting as i32;
        entry.failure = None;
        entry.process = None;
        let mut registry = self.registry.clone();
        registry.entries.get_mut(&entry.name).expect("entry exists").entry = entry.encode_to_vec();
        self.commit(registry, changed(entry.clone())).await?;
        self.stop(&entry.name).await;
        self.start(&entry.name);
        Ok(wire::PluginsRestartResponse { entry: Some(entry) })
    }

    async fn delete(&mut self, request: wire::PluginsDeleteRequest) -> Result<wire::PluginsDeleteResponse, Error> {
        let entry = self.entry(&request.name, &request.expected_revision)?;
        let mut registry = self.registry.clone();
        let saved = registry.entries.remove(&entry.name).expect("entry exists");
        let response = wire::PluginsDeleteResponse { receipt: Some(self.receipt(String::new())) };
        self.commit(registry, wire::PluginChange { kind: Some(wire::plugin_change::Kind::Deleted(entry.name.clone())) }).await?;
        self.stop(&entry.name).await;
        if let Some(executable) = saved.executable {
            crate::bundle::validate_path(&executable).map_err(|error| problem("Delete Plugin Installation", error))?;
            let parent = executable.split('/').next().expect("installation path has a parent");
            std::fs::remove_dir_all(self.settings.directory.join(parent)).map_err(|error| problem("Delete Plugin Installation", error))?;
        }
        Ok(response)
    }

    fn start(&mut self, name: &str) {
        let entry = self.snapshots.back().and_then(|snapshot| snapshot.entries.get(name)).expect("persisted entry exists");
        if !entry.value.as_ref().is_some_and(|value| value.enabled) { return; }
        let saved = self.registry.entries.get(name).expect("persisted entry exists");
        let placement = match entry.placement.as_ref().and_then(|placement| placement.kind.as_ref()) {
            Some(wire::plugin_placement::Kind::Remote(remote)) => Ok(Placement::Remote(remote.url.clone())),
            Some(wire::plugin_placement::Kind::Managed(_)) => managed(saved, &self.settings),
            None => Err(Error::new("placement_required", "Start Plugin", "restore a valid placement")),
        };
        let attach = self.attach.clone();
        let commands = self.commands.clone();
        let name = name.to_owned();
        let activation = entry.activation;
        let (stop, mut stopping) = oneshot::channel();
        let worker_name = name.clone();
        let task = tokio::spawn(async move {
            let attached = match placement {
                Ok(placement) => tokio::select! {
                    biased;
                    _ = &mut stopping => return,
                    result = attach(placement) => result.map_err(|error| error.to_string()),
                },
                Err(error) => Err(error.to_string()),
            };
            let attached = match attached {
                Ok(attached) => attached,
                Err(error) => {
                    let command = Command::Health { name: worker_name, activation, status: Box::new(Err(error)) };
                    tokio::select! { _ = &mut stopping => {}, _ = commands.send(command) => {} }
                    return;
                },
            };
            let mut status = attached.status();
            loop {
                let current = status.borrow_and_update().clone();
                let command = Command::Health { name: worker_name.clone(), activation, status: Box::new(Ok(current)) };
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
            if let Err(error) = attached.stop("Plugin Entry Activation Replaced").await {
                tracing::warn!(plugin = %worker_name, %error, "plugin activation stop reported a failure");
            }
        });
        self.workers.insert(name, Worker { stop, task });
    }

    async fn stop(&mut self, name: &str) {
        if let Some(worker) = self.workers.remove(name) {
            let _ = worker.stop.send(());
            let _ = worker.task.await;
        }
    }

    async fn health(&mut self, name: String, activation: u64, status: Result<Status, String>) -> Result<(), Error> {
        let Some(mut entry) = self.snapshots.back().and_then(|snapshot| snapshot.entries.get(&name)).cloned() else { return Ok(()); };
        if entry.activation != activation || !entry.value.as_ref().is_some_and(|value| value.enabled) { return Ok(()); }
        match status {
            Ok(status) => {
                if let Some(current) = status.activation {
                    entry.state = wire::PluginState::Ready as i32;
                    entry.failure = None;
                    entry.catalog_digest = current.description.catalog.as_ref().expect("admitted catalog exists").digest.clone();
                    entry.process = current.process_id.map(|id| wire::PluginProcess { id, executable_digest: current.executable_digest.map(|digest| digest.to_vec()).unwrap_or_default(), started_at: current.description.started_at });
                    entry.description = Some(current.description);
                } else {
                    entry.state = wire::PluginState::Unavailable as i32;
                    entry.process = None;
                    entry.failure = Some(unavailable(&name, status.failure.map(|failure| failure.to_string()).unwrap_or_else(|| "plugin activation stopped".into())));
                }
            },
            Err(error) => {
                entry.state = wire::PluginState::Unavailable as i32;
                entry.process = None;
                entry.failure = Some(unavailable(&name, error));
            },
        }
        let mut registry = self.registry.clone();
        registry.entries.get_mut(&name).expect("entry exists").entry = entry.encode_to_vec();
        self.commit(registry, changed(entry)).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn missing() -> Error { Error::new("entry_missing", "Get Plugin Entry", "list plugins and select an existing entry") }
fn next_activation(value: u64) -> Result<u64, Error> {
    value.checked_add(1).ok_or_else(|| Error::new("activation_exhausted", "Restart Plugin", "create a new entry"))
}
fn changed(entry: wire::PluginEntry) -> wire::PluginChange { wire::PluginChange { kind: Some(wire::plugin_change::Kind::Entry(entry)) } }
fn unavailable(name: &str, cause: String) -> v1::Failure {
    v1::Failure { classification: v1::Classification::Transient as i32, code: "plugin_unavailable".into(), message: format!("Activate Plugin: {cause}"), action: "correct the reported cause and Restart the plugin".into(), plugin: name.into(), ..Default::default() }
}
fn selected(event: &wire::PluginsWatchResponse, name: &str) -> bool {
    if name.is_empty() { return true; }
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
    let executable = saved.executable.as_ref().ok_or_else(|| Error::new("executable_missing", "Start Managed Plugin", "recreate the entry from a verified bundle"))?;
    crate::bundle::validate_path(executable).map_err(|error| problem("Verify Installed Executable Path", error))?;
    let executable = settings.directory.join(executable);
    let bytes = std::fs::read(&executable).map_err(|error| problem("Read Installed Executable", error))?;
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    if saved.executable_digest != Some(digest) {
        return Err(Error::new("executable_digest", "Start Managed Plugin: Installed Executable Was Changed", "recreate the entry from the original verified bundle"));
    }
    Ok(Placement::Managed(Managed { executable, arguments: vec![], environment: Default::default() }))
}
fn encode_token(value: &(v1::ReadCut, u32, u64, String)) -> String {
    let bytes = value.0.encode_to_vec();
    let bytes = serde_json::to_vec(&(bytes, value.1, value.2, &value.3)).expect("page values serialize");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn decode_token(token: &str) -> Result<(v1::ReadCut, u32, u64, String), Error> {
    let invalid = || Error::new("page_invalid", "List Plugins", "return the issued page token unchanged");
    if !token.len().is_multiple_of(2) { return Err(invalid()); }
    let bytes = token.as_bytes().chunks_exact(2).map(|pair| std::str::from_utf8(pair).ok().and_then(|pair| u8::from_str_radix(pair, 16).ok()).ok_or_else(&invalid)).collect::<Result<Vec<_>, _>>()?;
    let (cut, rows, bytes, after): (Vec<u8>, u32, u64, String) = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    let cut = v1::ReadCut::decode(cut.as_slice()).map_err(|_| invalid())?;
    Ok((cut, rows, bytes, after))
}

/* === Tests === */
