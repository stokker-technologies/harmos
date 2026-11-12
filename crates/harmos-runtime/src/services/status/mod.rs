/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */




/* === Modules === */

mod history;
mod diagnostics;
mod redaction;
mod layer;


/* === Re-Exports === */

pub use diagnostics::{ContentSource, Entry, Range};
pub use redaction::Redaction;
pub use layer::StatusLayer;


/* === Imports === */

use std::{collections::BTreeMap, num::NonZeroUsize, sync::{Arc, Mutex, Weak}};
use harmos_protocol::{v1, status as wire};
use harmos_operations::{status::{EventStream, Pending}, execution::operations::ServingError};
use tokio::sync::watch;
use history::History;


// Standard Library Imports

// Third Party Imports

// Local Imports


/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Port(pub Arc<Status>);

pub struct StatusSettings {
 pub application_name: String, pub application_version: String, pub host_id: String,
 pub history_items: NonZeroUsize, pub history_bytes: NonZeroUsize,
 pub read_leases: NonZeroUsize, pub archive_bytes: NonZeroUsize, pub range_bytes: NonZeroUsize,
 pub redaction_bytes: NonZeroUsize, pub contributions: NonZeroUsize,
}

pub struct Status {
 clock: crate::clock::Clock, settings: StatusSettings, identity: v1::HostIdentity,
 pub redaction: Redaction,
 state: Mutex<State>, changes: watch::Sender<u64>,
}
struct State {
 logs: History, traces: History, status_history: History,
 providers: BTreeMap<u64, Arc<dyn Contribution>>, next_provider: u64, providers_revision: u64,
 components: Vec<v1::HealthComponent>, available: bool,
 archive: Option<diagnostics::Archive>,
}
pub struct Registration { owner: Weak<Status>, id: u64 }


/* === Enums === */

/* === Traits === */

pub trait Contribution: Send + Sync + 'static {
 fn health(&self) -> Result<Vec<v1::HealthComponent>, Box<dyn std::error::Error + Send + Sync>>;
 fn diagnostics(&self) -> Result<Vec<Entry>, Box<dyn std::error::Error + Send + Sync>>;
}


/* === Implementations === */

impl std::ops::Deref for Port {
 type Target = Arc<Status>;
 fn deref(&self) -> &Self::Target { &self.0 }
}

impl Status {
 pub(super) fn new(settings: StatusSettings, clock: crate::clock::Clock, started: prost_types::Timestamp) -> Result<Arc<Self>, ServingError> {
  if settings.application_name.is_empty() || settings.application_version.is_empty() || settings.host_id.is_empty() || settings.range_bytes > settings.archive_bytes {
   return Err(failure("Status.Configure", "identity must be complete and range_bytes must not exceed archive_bytes"));
  }
  let epoch = identifier("Status.Configure")?;
  let history = |name| History::new(name, &epoch, settings.history_items.get(), settings.history_bytes.get(), settings.read_leases.get());
  let identity = v1::HostIdentity { application_name: settings.application_name.clone(), application_version: settings.application_version.clone(),
   host_id: settings.host_id.clone(), library_version: env!("CARGO_PKG_VERSION").into(), process_started_at: Some(started) };
  let state = State { logs: history("status.logs"), traces: history("status.trace_events"), status_history: history("status"), providers: BTreeMap::new(), next_provider: 0, providers_revision: 0,
   components: Vec::new(), available: true, archive: None };
  let redaction = Redaction::new(settings.redaction_bytes.get());
  Ok(Arc::new(Self { clock, identity, redaction, state: Mutex::new(state), changes: watch::channel(0).0, settings }))
 }
 pub fn contribute(self: &Arc<Self>, contribution: Arc<dyn Contribution>) -> Result<Registration, ServingError> {
  let id = {
   let mut state = self.state.lock().expect("status lock is healthy");
   if state.providers.len() >= self.settings.contributions.get() { return Err(failure("Status.Contribute", "contribution capacity exceeded")); }
   state.next_provider = state.next_provider.checked_add(1).ok_or_else(|| failure("Status.Contribute", "contribution identity exhausted"))?;
   let id = state.next_provider; state.providers.insert(id, contribution); state.providers_revision = state.providers_revision.wrapping_add(1); id
  };
  if let Err(error) = self.refresh() { self.state.lock().expect("status lock is healthy").providers.remove(&id); return Err(error); }
  Ok(Registration { owner: Arc::downgrade(self), id })
 }
 pub(super) fn refresh(&self) -> Result<(), ServingError> {
  loop {
  let (revision, providers) = {
   let state = self.state.lock().expect("status lock is healthy");
   (state.providers_revision, state.providers.iter().map(|(id,p)| (*id,p.clone())).collect::<Vec<_>>())
  };
  let mut components = BTreeMap::from([("services".to_owned(), v1::HealthComponent { name: "services".into(), state: v1::ComponentState::Healthy as i32, reason: String::new() })]);
  for (id, provider) in providers {
   let facts = provider.health().unwrap_or_else(|_| vec![v1::HealthComponent { name: format!("contribution.{id}"), state: v1::ComponentState::Unavailable as i32, reason: "Status.Get: contribution unavailable".into() }]);
   for mut component in facts {
    if component.name.is_empty() || v1::ComponentState::try_from(component.state).ok().is_none_or(|s| s == v1::ComponentState::Unspecified) {
     return Err(failure("Status.Get", "contribution returned invalid health"));
    }
    component.name = self.redaction.text(&component.name); component.reason = self.redaction.text(&component.reason);
    if components.insert(component.name.clone(), component).is_some() { return Err(failure("Status.Get", "duplicate health component")); }
   }
  }
  let mut state = self.state.lock().expect("status lock is healthy");
  if state.providers_revision != revision { continue; }
  let components: Vec<_> = components.into_values().collect();
  let status = v1::HostStatus { identity: Some(self.identity.clone()), available: state.available, components: components.clone() };
  if prost::Message::encoded_len(&status) > self.settings.history_bytes.get() { return Err(failure("Status.Get", "status exceeds configured byte bound")); }
  state.components = components;
  state.status_history.append(wire::Event { observed_at: Some(self.clock.date().into()), ..Default::default() })?;
  drop(state); self.changed(); return Ok(());
  }
 }
 fn changed(&self) { self.changes.send_modify(|sequence| *sequence = sequence.wrapping_add(1)); }
 pub fn record_log(&self, event: wire::Event) -> Result<(), ServingError> { self.record(event, false) }
 pub fn record_trace(&self, event: wire::Event) -> Result<(), ServingError> { self.record(event, true) }
 fn record(&self, mut event: wire::Event, trace: bool) -> Result<(), ServingError> {
  event.observed_at = Some(self.clock.date().into()); event.target = self.redaction.text(&event.target);
  event.message = self.redaction.text(&event.message); event.level = self.redaction.text(&event.level);
  event.fields = event.fields.into_iter().map(|(key,value)| { let value = self.redaction.field(&key,&value); (self.redaction.text(&key),value) }).collect();
  let mut state = self.state.lock().expect("status lock is healthy");
  if trace { state.traces.append(event)?; } else { state.logs.append(event)?; }
  drop(state); self.changed(); Ok(())
 }
 pub fn layer(self: &Arc<Self>) -> StatusLayer { StatusLayer::new(Arc::downgrade(self)) }
 pub(crate) fn close(&self) {
  let mut state = self.state.lock().expect("status lock is healthy"); state.available = false;
  let _ = state.status_history.append(wire::Event::default()); drop(state); self.changed();
 }
 fn snapshot(&self, state: &State) -> v1::HostStatus {
  v1::HostStatus { identity: Some(self.identity.clone()), available: state.available && state.components.iter().all(|c| c.state != v1::ComponentState::Unavailable as i32), components: state.components.clone() }
 }
 fn page(&self, trace: bool, at: Option<&v1::Position>, page: Option<&v1::PageRequest>) -> Result<(Vec<wire::Event>,v1::Position,String),ServingError> {
  let action = if trace { "Status.TraceEvents.List" } else { "Status.Logs.List" };
  let page = page.ok_or_else(|| failure(action,"page bounds are required"))?;
  let mut state = self.state.lock().expect("status lock is healthy");
  let history = if trace { &mut state.traces } else { &mut state.logs };
  history.page(at,page,action)
 }
 fn event_watch(self: Arc<Self>, trace: bool, at: Option<v1::Position>, maximum_items: usize, maximum_bytes: usize) -> Result<EventStream<(Vec<wire::Event>,v1::Position)>,ServingError> {
  let action = if trace { "Status.TraceEvents.Watch" } else { "Status.Logs.Watch" };
  if maximum_items == 0 || maximum_bytes < 128 { return Err(failure(action,"positive event and byte bounds are required")); }
  let changes = self.changes.subscribe();
  let boundary = {
   let mut state = self.state.lock().expect("status lock is healthy");
   let history = if trace { &mut state.traces } else { &mut state.logs };
   history.resolve(at.as_ref(),action)?
  };
  let stream = futures_util::stream::try_unfold((self,changes,boundary.sequence,Some(boundary)),move |(owner,mut changes,mut after,opening)| async move {
   if let Some(boundary) = opening { return Ok(Some(((Vec::new(),boundary),(owner,changes,after,None)))); }
   loop {
    let next = {
     let mut state = owner.state.lock().expect("status lock is healthy");
     let history = if trace { &mut state.traces } else { &mut state.logs };
     let boundary = history.latest();
     let events = history.range(after,boundary.sequence,maximum_items,maximum_bytes,action)?;
     if events.is_empty() { None } else {
      after = events.last().expect("nonempty events").sequence;
      let position = history.issue(after);
      Some((events,position))
     }
    };
    if let Some(next) = next { return Ok(Some((next,(owner,changes,after,None)))); }
    if changes.changed().await.is_err() { return Ok(None); }
   }
  });
  Ok(Box::pin(stream))
 }
}
impl Registration {
 pub fn changed(&self) -> Result<(), ServingError> { self.owner.upgrade().ok_or_else(|| failure("Status.Get","host is closed"))?.refresh() }
}
impl Drop for Registration {
 fn drop(&mut self) { if let Some(owner) = self.owner.upgrade() { { let mut state = owner.state.lock().expect("status lock is healthy"); state.providers.remove(&self.id); state.providers_revision = state.providers_revision.wrapping_add(1); } let _ = owner.refresh(); } }
}
impl harmos_operations::status::Status for Port {
 fn get(&self, _: v1::StatusGetRequest) -> Pending<'_,v1::StatusGetResponse> { Box::pin(async move {
  let mut state = self.state.lock().expect("status lock is healthy");
  let status = self.snapshot(&state); let resolved_at = Some(state.status_history.latest());
  Ok(v1::StatusGetResponse { status: Some(status), resolved_at })
 }) }
 fn watch(&self, request: v1::StatusWatchRequest) -> Pending<'_,EventStream<v1::StatusWatchResponse>> { Box::pin(async move {
  let changes = self.changes.subscribe();
  self.state.lock().expect("status lock is healthy").status_history.resolve(request.at.as_ref(),"Status.Watch")?;
  let owner = self.0.clone();
  let stream = futures_util::stream::try_unfold((owner,changes,None), |(owner,mut changes,opening)| async move {
   loop {
    let (status,boundary) = { let mut state = owner.state.lock().expect("status lock is healthy"); (owner.snapshot(&state), state.status_history.latest()) };
    if opening.is_none() || Some(boundary.sequence) != opening {
     changes.borrow_and_update();
     let sequence = boundary.sequence;
     let event = v1::StatusWatchResponse { status: Some(status), resolved_at: Some(boundary) };
     return Ok(Some((event,(owner,changes,Some(sequence)))));
    }
    if changes.changed().await.is_err() { return Ok(None); }
   }
  });
  Ok(Box::pin(stream) as EventStream<_>)
 }) }
 fn logs_list(&self, r: wire::LogsListRequest) -> Pending<'_,wire::LogsListResponse> { Box::pin(async move { let (events,resolved_at,next_page_token)=self.page(false,r.at.as_ref(),r.page.as_ref())?; Ok(wire::LogsListResponse{events,resolved_at:Some(resolved_at),next_page_token}) }) }
 fn trace_events_list(&self, r: wire::TraceEventsListRequest) -> Pending<'_,wire::TraceEventsListResponse> { Box::pin(async move { let (events,resolved_at,next_page_token)=self.page(true,r.at.as_ref(),r.page.as_ref())?; Ok(wire::TraceEventsListResponse{events,resolved_at:Some(resolved_at),next_page_token}) }) }
 fn logs_watch(&self, r: wire::LogsWatchRequest) -> Pending<'_,EventStream<wire::LogsWatchResponse>> { use futures_util::TryStreamExt; Box::pin(async move { Ok(Box::pin(self.0.clone().event_watch(false,r.at,r.max_events as usize,r.max_bytes as usize)?.map_ok(|(events,position)|wire::LogsWatchResponse{events,resolved_at:Some(position)})) as EventStream<_>) }) }
 fn trace_events_watch(&self, r: wire::TraceEventsWatchRequest) -> Pending<'_,EventStream<wire::TraceEventsWatchResponse>> { use futures_util::TryStreamExt; Box::pin(async move { Ok(Box::pin(self.0.clone().event_watch(true,r.at,r.max_events as usize,r.max_bytes as usize)?.map_ok(|(events,position)|wire::TraceEventsWatchResponse{events,resolved_at:Some(position)})) as EventStream<_>) }) }
 fn diagnostics_get(&self, r: wire::DiagnosticsGetRequest) -> Pending<'_,wire::DiagnosticsGetResponse> { Box::pin(async move { self.diagnostics(r) }) }
}


/* === Public Functions === */

pub(crate) fn failure(action: &'static str, reason: &str) -> ServingError {
 let failure = harmos_operations::Failure::new(harmos_operations::operations::FailureKind::Permanent,format!("{action}: {reason}")).with_detail("action",action);
 ServingError::Handler(failure)
}


/* === Private Functions === */

pub(crate) fn identifier(action: &'static str) -> Result<String,ServingError> {
 let mut bytes = [0u8;32]; getrandom::fill(&mut bytes).map_err(|_| failure(action,"secure randomness unavailable"))?;
 Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}


/* === Tests === */
