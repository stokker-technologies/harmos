/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::any::TypeId;
use std::fmt;
use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Poll;

use tokio::sync::watch;

use super::window::{ListenerCandidate, Row};
use super::{Holding, ManagedPublication, Stream, StreamManager, Table, Wired};
use crate::operations::{EncodedInvocation, Manager, PluginCallback};
use crate::plugin::PluginScope;
use crate::schema::{Described, graph::Schema};
use crate::supervision::{Listener, ListenerDelivery};
use crate::{Application, Error, Message, MessageError, Result};
use harmos_domain::listeners::{Context, Source};

const PAGE_ROWS: usize = 512;
const PAGE_BYTES: usize = 8 * 1024 * 1024;
const SOURCES: usize = 64;
const REGISTRATIONS: usize = 64;

type Submit = fn(&Row, Context, &PluginCallback) -> Result<EncodedInvocation>;

pub struct PluginListenerBindings<'a, A: Application> {
    streams: &'a StreamManager<A>,
    scope: &'a PluginScope,
    operations: &'a Manager,
    sources: Vec<Captured>,
    entries: Vec<Binding>,
}

struct Binding {
    ordinal: usize,
    source: usize,
    callback: PluginCallback,
    submit: Submit,
}

struct Captured {
    wired: Arc<Wired>,
    frontier: Arc<AtomicU64>,
    bytes: fn(&Row) -> Result<usize>,
}

#[must_use = "retain the listener registration until shutdown positively settles"]
pub struct PluginListenerRegistration {
    table: Arc<Table>,
    listening: Option<Arc<Listening>>,
}

pub(super) struct Listening {
    delivery: Listener,
    frontiers: Vec<(Arc<str>, u32, Arc<AtomicU64>)>,
    held: Arc<Mutex<Vec<Holding>>>,
}

struct Chunk {
    rows: Vec<(ListenerCandidate, Row)>,
    through: u64,
}

// Borrowed, Already Validated Protobuf Bytes; Never A Schema Or Authority Facade
struct Payload<'a>(&'a [u8]);

impl<A: Application> StreamManager<A> {
    pub fn stage_listeners(
        &self, scope: &PluginScope, operations: &Manager, configure: impl FnOnce(&mut PluginListenerBindings<'_, A>) -> Result<()>,
    ) -> Result<PluginListenerRegistration> {
        if scope.is_closed() || scope.has_committed() || self.closed.load(Ordering::Acquire) {
            return Err(Error::Closing { domain: "stream listener staging" });
        }
        let mut bindings = PluginListenerBindings {
            streams: self,
            scope,
            operations,
            sources: Vec::new(),
            entries: Vec::new(),
        };
        configure(&mut bindings)?;
        let expected = scope.listeners().iter().filter(|listener| matches!(listener.source(), Source::Stream { .. })).count();
        if bindings.entries.len() != expected {
            return Err(Error::InvalidInput {
                field: "stream listeners",
                reason: "the complete frozen source inventory must be bound",
            });
        }
        if expected == 0 {
            return Ok(PluginListenerRegistration {
                table: Arc::clone(&self.windows),
                listening: None,
            });
        }
        bindings.entries.sort_by_key(|entry| entry.ordinal);
        let mut registry = self.windows.listeners.lock().expect("stream listener registry poisoned");
        registry.retain(|_, owner| !owner.delivery.cleaned());
        if registry.contains_key(scope.owner()) {
            return Err(Error::AlreadyRegistered {
                domain: "stream listeners",
                name: scope.owner().to_owned(),
            });
        }
        if registry.len() == REGISTRATIONS {
            return Err(Error::ResourceExhausted {
                resource: "stream listener registrations",
            });
        }
        let held = Arc::new(Mutex::new(Vec::new()));
        let frontiers = bindings
            .sources
            .iter()
            .map(|source| (Arc::<str>::from(source.wired.definition.id()), source.wired.definition.version(), Arc::clone(&source.frontier)))
            .collect();
        let delivery = Listener::spawn(scope, {
            let streams = self.clone();
            let scope = scope.clone();
            let held = Arc::clone(&held);
            move |delivery, stopping| async move {
                let providers: Vec<_> = bindings
                    .sources
                    .iter()
                    .filter_map(|source| source.wired.staged.as_ref())
                    .filter(|source| !source.scope.same_activation(&scope))
                    .map(|source| source.scope.clone())
                    .collect();
                tokio::select! {
                    biased;
                    _ = closed_provider(&providers) => Err(Error::Closing { domain: "listener stream provider" }),
                    answer = deliver(&streams, &scope, bindings.sources, bindings.entries, &delivery, &held, stopping) => answer,
                }
            }
        });
        let listening = Arc::new(Listening { delivery, frontiers, held });
        registry.insert(scope.owner().to_owned(), Arc::clone(&listening));
        drop(registry);
        let registration = PluginListenerRegistration {
            table: Arc::clone(&self.windows),
            listening: Some(listening),
        };
        if let Err(error) = scope.bind_stream_listeners() {
            registration.stop();
            return Err(error);
        }
        Ok(registration)
    }

    pub(crate) fn stop_listeners(&self) {
        let owners: Vec<_> = self.windows.listeners.lock().expect("stream listener registry poisoned").values().cloned().collect();
        for owner in owners {
            owner.delivery.stop();
        }
    }

    pub async fn shutdown_listeners(&self) -> Result<()> {
        let owners: Vec<_> = self.windows.listeners.lock().expect("stream listener registry poisoned").values().cloned().collect();
        for owner in &owners {
            owner.delivery.stop();
        }
        for owner in owners {
            owner.shutdown().await?;
        }
        self.windows.listeners.lock().expect("stream listener registry poisoned").retain(|_, owner| !owner.delivery.cleaned());
        Ok(())
    }

    pub async fn shutdown_listeners_for(&self, scope: &PluginScope) -> Result<()> {
        let owner = self.windows.listeners.lock().expect("stream listener registry poisoned").get(scope.owner()).cloned();
        let Some(owner) = owner else {
            return Ok(());
        };
        if !owner.delivery.scope().same_activation(scope) {
            return Err(Error::InvalidInput {
                field: "stream listener scope",
                reason: "scope belongs to a different registered activation",
            });
        }
        owner.shutdown().await?;
        self.windows.remove_listening(&owner);
        Ok(())
    }
}

impl<A: Application> PluginListenerBindings<'_, A> {
    pub fn stream<S: Stream<A> + Message + Described>(&mut self, after: u64) -> Result<()> {
        let wired = self.source(S::ID, S::VERSION)?;
        if wired.definition.row() != TypeId::of::<S>() || wired.definition.owner().is_some() {
            return Err(Error::InvalidInput {
                field: "native listener stream",
                reason: "source differs from the compiled native catalog member",
            });
        }
        let graph = Schema::from_declaration(S::DEFINITION, &self.streams.schema_settings)
            .map_err(|source| Error::RecordSchema { source: source.into() })?;
        self.bind(wired, &graph, after, submit_native::<S>, native_bytes::<S>)
    }

    pub fn publication(&mut self, id: &str, schema_ordinal: u32, after: u64) -> Result<()> {
        let wired = self.source(id, schema_ordinal)?;
        if wired.validation.is_none() || wired.staged.is_none() {
            return Err(Error::InvalidInput {
                field: "managed listener stream",
                reason: "source has no original validated generation owner",
            });
        }
        let graph = wired.definition.schema().ok_or(Error::InvalidInput {
            field: "managed listener stream",
            reason: "source has no admitted row schema",
        })?;
        self.bind(Arc::clone(&wired), graph, after, submit_publication, publication_bytes)
    }

    fn source(&self, id: &str, version: u32) -> Result<Arc<Wired>> {
        let _visibility = self.streams.windows.visibility.read().expect("stream visibility poisoned");
        let windows = self.streams.windows.windows.read().expect("stream registry poisoned");
        let wired = windows.get(id).and_then(|versions| versions.get(&version)).cloned().ok_or_else(|| Error::NotRegistered {
            domain: "listener stream",
            name: id.to_owned(),
        })?;
        if let Some(owner) = wired.definition.owner() {
            if owner != self.scope.owner() && !self.scope.allows(owner) {
                return Err(Error::InvalidInput {
                    field: "listener stream owner",
                    reason: "source is neither owned nor a direct dependency",
                });
            }
            if let Some(staged) = &wired.staged
                && !staged.scope.same_activation(self.scope)
                && !staged.scope.is_ready()
            {
                return Err(Error::Closing { domain: "listener stream source" });
            }
        }
        Ok(wired)
    }

    fn bind(&mut self, wired: Arc<Wired>, graph: &Schema, after: u64, submit: Submit, bytes: fn(&Row) -> Result<usize>) -> Result<()> {
        if self.sources.len() == SOURCES {
            return Err(Error::ResourceExhausted {
                resource: "stream listener source windows",
            });
        }
        if after > wired.window.logical_frontier() {
            return Err(Error::FrontierAhead);
        }
        if self.sources.iter().any(|source| Arc::ptr_eq(&source.wired.window, &wired.window)) {
            return Err(Error::AlreadyRegistered {
                domain: "listener stream source",
                name: wired.definition.id().into(),
            });
        }
        let index = self.sources.len();
        let mut selected = false;
        for (ordinal, listener) in self.scope.listeners().iter().enumerate() {
            let Source::Stream { id, schema_ordinal, .. } = listener.source() else {
                continue;
            };
            if id.as_ref() != wired.definition.id() || *schema_ordinal != wired.definition.version() {
                continue;
            }
            let source_graph = listener.source().schema();
            if !graph
                .equivalent(graph.root, source_graph, source_graph.root, &self.streams.schema_settings)
                .map_err(|source| Error::RecordSchema { source: source.into() })?
            {
                return Err(Error::InvalidInput {
                    field: "listener stream source",
                    reason: "row schema differs from the frozen declaration",
                });
            }
            self.entries.push(Binding {
                ordinal,
                source: index,
                callback: self.operations.capture_callback(self.scope, listener.callback())?,
                submit,
            });
            selected = true;
        }
        if !selected {
            return Err(Error::NotRegistered {
                domain: "listener stream source",
                name: wired.definition.id().into(),
            });
        }
        self.sources.push(Captured {
            wired,
            frontier: Arc::new(AtomicU64::new(after)),
            bytes,
        });
        Ok(())
    }
}

impl PluginListenerRegistration {
    pub fn frontier(&self, id: &str, schema_ordinal: u32) -> Option<u64> {
        self.listening
            .as_ref()?
            .frontiers
            .iter()
            .find(|(source, version, _)| source.as_ref() == id && *version == schema_ordinal)
            .map(|(_, _, frontier)| frontier.load(Ordering::Acquire))
    }

    pub fn failure(&self) -> Option<Arc<Error>> {
        self.listening.as_ref().and_then(|owner| owner.delivery.failure())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(owner) = &self.listening {
            owner.shutdown().await?;
            self.table.remove_listening(owner);
        }
        Ok(())
    }

    fn stop(&self) {
        if let Some(owner) = &self.listening {
            owner.delivery.stop();
        }
    }
}

impl Drop for PluginListenerRegistration {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Listening {
    async fn shutdown(&self) -> Result<()> {
        self.delivery.shutdown().await?;
        let held = std::mem::take(&mut *self.held.lock().expect("listener demand poisoned"));
        drop(held);
        Ok(())
    }
}

impl fmt::Debug for Listening {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Listening")
            .field("owner", &self.delivery.scope().owner())
            .field("cleaned", &self.delivery.cleaned())
            .finish_non_exhaustive()
    }
}

impl Table {
    fn remove_listening(&self, owner: &Arc<Listening>) {
        let mut registry = self.listeners.lock().expect("stream listener registry poisoned");
        if registry.get(owner.delivery.scope().owner()).is_some_and(|current| Arc::ptr_eq(current, owner)) {
            registry.remove(owner.delivery.scope().owner());
        }
    }
}

impl Message for Payload<'_> {
    const CODEC: &'static str = "harmos.raw.v1";

    fn decoding_memory(_: &[u8]) -> std::result::Result<usize, MessageError> {
        Err(MessageError::borrowed("borrowed source payload is never decoded"))
    }

    fn encode(&self) -> Vec<u8> {
        self.0.to_vec()
    }

    fn encode_into(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(self.0);
    }

    fn encoded_size(&self) -> usize {
        self.0.len()
    }

    fn decode(_: &[u8]) -> std::result::Result<Self, MessageError> {
        Err(MessageError::new("borrowed source payload is never decoded"))
    }
}

fn submit_native<S: Message + 'static>(row: &Row, context: Context, callback: &PluginCallback) -> Result<EncodedInvocation> {
    let row = row.downcast_ref::<S>().ok_or(Error::InvalidInput {
        field: "listener native row",
        reason: "original window row type differs from its captured source",
    })?;
    submit(row, context, callback)
}

fn submit_publication(row: &Row, context: Context, callback: &PluginCallback) -> Result<EncodedInvocation> {
    let row = row.downcast_ref::<ManagedPublication>().ok_or(Error::InvalidInput {
        field: "listener managed row",
        reason: "original charged publication owner is absent",
    })?;
    submit(&Payload(row.payload()), context, callback)
}

fn submit<R: Message>(row: &R, context: Context, callback: &PluginCallback) -> Result<EncodedInvocation> {
    let encoding_error = |source| Error::OperationFailed {
        operation: callback.contract().operation().id.to_string(),
        source: Arc::new(source),
    };
    let bytes = harmos_domain::listeners::encoding_memory_bytes(context, row).map_err(encoding_error)?;
    let bytes = NonZeroUsize::new(bytes).ok_or(Error::ResourceExhausted { resource: "stream callback encoding" })?;
    callback.submit_with(bytes, |budget| harmos_domain::listeners::encode_borrowed(context, row, budget).map_err(encoding_error))
}

async fn closed_provider(providers: &[PluginScope]) {
    let mut closed: Vec<_> = providers.iter().map(|provider| Box::pin(provider.closed())).collect();
    std::future::poll_fn(|context| {
        for provider in &mut closed {
            if provider.as_mut().poll(context).is_ready() {
                return Poll::Ready(());
            }
        }
        Poll::Pending
    })
    .await
}

fn native_bytes<S: Message + 'static>(row: &Row) -> Result<usize> {
    row.downcast_ref::<S>().map(Message::encoded_size).ok_or(Error::InvalidInput {
        field: "listener row",
        reason: "original native row type differs",
    })
}

fn publication_bytes(row: &Row) -> Result<usize> {
    row.downcast_ref::<ManagedPublication>().map(|row| row.payload().len()).ok_or(Error::InvalidInput {
        field: "listener row",
        reason: "original publication owner is absent",
    })
}

fn page(table: &Table, sources: &[Captured], after: u64) -> Result<Chunk> {
    let _visibility = table.visibility.read().expect("stream visibility poisoned");
    let cut = sources.iter().map(|source| source.wired.window.latest_locked()).max().unwrap_or(after).max(after);
    let mut selected = Vec::with_capacity(PAGE_ROWS);
    for (index, source) in sources.iter().enumerate() {
        source.wired.window.listener_candidates(after, cut, index, PAGE_ROWS, &mut selected);
    }
    let mut through = if selected.len() == PAGE_ROWS { selected.last().unwrap().cursor } else { cut };
    let mut rows = Vec::new();
    let mut bytes = 0usize;
    let mut accepted = after;
    for candidate in selected {
        let source = &sources[candidate.source];
        if let Some(row) = source.wired.window.listener_row(candidate.cursor) {
            let size = (source.bytes)(&row)?;
            // A Single Admitted Row Must Make Progress Even Above The Page Target.
            if !rows.is_empty() && size > PAGE_BYTES.saturating_sub(bytes) {
                through = accepted;
                break;
            }
            bytes = bytes.saturating_add(size);
            accepted = candidate.cursor;
            rows.push((candidate, row));
        } else {
            accepted = candidate.cursor;
        }
    }
    Ok(Chunk { rows, through })
}

async fn deliver<A: Application>(
    streams: &StreamManager<A>, scope: &PluginScope, sources: Vec<Captured>, entries: Vec<Binding>, delivery: &ListenerDelivery,
    held: &Mutex<Vec<Holding>>, mut stopping: watch::Receiver<bool>,
) -> Result<()> {
    tokio::select! { biased; _ = stopping.changed() => return Ok(()), ready = scope.ready() => ready?, }
    let demand = sources.iter().map(|source| Holding::engage(Arc::clone(&source.wired.window))).collect();
    *held.lock().expect("listener demand poisoned") = demand;
    let mut changed = streams.delivery();
    let mut after = 0;
    loop {
        changed.borrow_and_update();
        if *stopping.borrow() || scope.is_closed() || streams.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        let page = page(&streams.windows, &sources, after)?;
        for (candidate, row) in page.rows {
            let source = &sources[candidate.source];
            let previous = source.frontier.load(Ordering::Acquire);
            if candidate.sequence <= previous {
                continue;
            }
            let context = Context::Stream {
                sequence: candidate.sequence,
                missed: candidate.sequence - previous - 1,
            };
            for binding in entries.iter().filter(|binding| binding.source == candidate.source) {
                if let Some(staged) = &source.wired.staged
                    && !staged.scope.is_ready()
                {
                    return Err(Error::Closing { domain: "listener stream source" });
                }
                let invocation = (binding.submit)(&row, context, &binding.callback)?;
                if !delivery.invoke(invocation, Arc::clone(&row), scope, &mut stopping).await? {
                    return Ok(());
                }
            }
            source.frontier.store(candidate.sequence, Ordering::Release);
        }
        let advanced = page.through > after;
        after = page.through;
        if advanced {
            tokio::task::yield_now().await;
            continue;
        }
        tokio::select! {
            biased;
            _ = stopping.changed() => return Ok(()),
            _ = scope.closed() => return Ok(()),
            changed = changed.changed() => if changed.is_err() { return Ok(()); },
        }
    }
}
