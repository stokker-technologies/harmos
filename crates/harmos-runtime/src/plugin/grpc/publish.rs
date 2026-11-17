/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Connection-Owned Publication Demand and Host Interval Windows

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::{BTreeMap, BTreeSet},
    error,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

// Third Party Imports
use futures_util::stream;
use tokio::{
    sync::{self, mpsc, watch},
    time::Instant,
};

// Local Imports
use super::{Activation, Error, PlacementSettings, model};
use crate::operations::clock::{Unit, WindowCall};
use crate::streams::protobuf::Row;
use crate::{
    Application,
    clock::Clock,
    streams::{ConnectionStreams, PublicationRow, StreamManager, StreamPublicationError, StreamWriter},
};
use harmos_protocol::plugin::instance_samples_request::Frame;
use harmos_protocol::plugin::{InstanceSamplesRequest, InstanceSamplesResponse, SamplesSelection, Window, instance_samples_response};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Publications {
    plugin: String,
    close: watch::Sender<bool>,
    jobs: tokio::task::JoinSet<Result<(), Error>>,
    /// Failures of Unscoped Demands, Which Their Attachment Reports.
    failures: mpsc::UnboundedReceiver<Arc<Error>>,
    registration: Arc<ConnectionStreams>,
    parent: Option<watch::Receiver<bool>>,
    source: Option<Arc<std::sync::OnceLock<ScopeSource>>>,
}

pub(super) struct ScopeSource {
    pub(super) registration: Arc<ConnectionStreams>,
    pub(super) close: watch::Receiver<bool>,
    pub(super) settings: PlacementSettings,
    pub(super) operations: crate::operations::Manager,
    pub(super) bindings: Mutex<Bindings>,
    /// Scopes Sampling Each Declared Stream; a Stream's Unscoped Demand Grants No Windows While Any Do.
    pub(super) sampling: BTreeMap<String, watch::Sender<usize>>,
}

/// One Scope's Share of `ScopeSource::sampling` for Its Requested Streams, Released When Dropped.
pub(super) struct Sampling(Vec<watch::Sender<usize>>);

/// Scoped Bindings the Attachment Owns; Admission Ends When Its Settlement Starts.
#[derive(Default)]
pub(super) struct Bindings {
    pub(super) closed: bool,
    pub(super) scopes: BTreeMap<String, watch::Receiver<Option<super::Outcome>>>,
    pub(super) tasks: tokio::task::JoinSet<()>,
}

pub(super) struct PreparedPublications {
    declarations: Vec<harmos_operations::StreamDeclaration>,
    demands: Vec<PreparedDemand>,
}
struct PreparedDemand {
    id: String,
    output: mpsc::Sender<InstanceSamplesRequest>,
    input: tonic::Streaming<InstanceSamplesResponse>,
    row: Row,
    cadence: Option<Cadence>,
}

/// An Interval Stream's Sample Period, Window Length and the Lag Its Windows Tolerate.
#[derive(Clone, Copy)]
struct Cadence {
    period: Duration,
    window: Duration,
    lag: Duration,
}

/// What Lets the Unscoped Demand Grant: Something Watches Its Stream and No Scope of Its Activation Samples It.
struct Gate {
    sampling: watch::Receiver<usize>,
    interest: crate::streams::Demand,
}

struct Demand {
    output: mpsc::Sender<InstanceSamplesRequest>,
    input: tonic::Streaming<InstanceSamplesResponse>,
    row: Row,
    cadence: Option<Cadence>,
    gate: Option<Gate>,
    lease: Arc<StreamWriter>,
    host: Clock,
    close: watch::Receiver<bool>,
    parent: Option<watch::Receiver<bool>>,
    closed: watch::Receiver<bool>,
    operations: crate::operations::Manager,
    caller: Arc<super::CallingPlugin>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Publications {
    pub(super) async fn open<A: Application>(
        streams: &StreamManager<A>, activation: &Activation, clock: &Clock, settings: &PlacementSettings,
        operations: &crate::operations::Manager,
    ) -> Result<Self, Error> {
        let prepared = Self::prepare(streams, activation, settings, operations, false, None).await?;
        Self::activate(prepared, streams, activation, clock, operations, settings, false)
    }
    /// Validate Every Declared Stream and Open Demand for the `selected` Ones, or for All Without a Selection.
    pub(super) async fn prepare<A: Application>(
        streams: &StreamManager<A>, activation: &Activation, settings: &PlacementSettings, operations: &crate::operations::Manager,
        replace: bool, selected: Option<&BTreeSet<String>>,
    ) -> Result<PreparedPublications, Error> {
        let catalog = activation.description.declarations.as_ref().expect("admitted description");
        let pool = prost_reflect::DescriptorPool::from_file_descriptor_set(catalog.types.clone().unwrap_or_default()).map_err(invalid)?;
        let mut declarations = Vec::new();
        let mut rows = Vec::new();
        for stream in &catalog.streams {
            let descriptor = pool
                .get_message_by_name(stream.row.trim_start_matches('.'))
                .ok_or_else(|| invalid("Prepare Samples: missing stream row"))?;
            let cadence = cadence(stream, &activation.settings, settings)?;
            // An Interval Stream Stores One Batch Row per Part: Its Row's Columns and Their Times.
            let row = match cadence {
                Some(_) => Row::new(harmos_protocol::columns::derive(&descriptor).map_err(invalid)?.batch)?,
                None => Row::new(descriptor)?,
            };
            declarations.push(harmos_operations::StreamDeclaration {
                id: stream.id.clone(),
                window: settings.stream_window.get(),
                row: row.schema.clone(),
                metadata: crate::Metadata::new(crate::Identifier::new(&stream.id).map_err(invalid)?, stream.id.clone(), None),
                row_metadata: Vec::new(),
            });
            rows.push((stream.id.clone(), row, cadence));
        }
        streams.validate_connection(activation.owner(), &declarations, replace).map_err(invalid)?;
        let mut demands = Vec::new();
        for (id, row, cadence) in rows {
            if selected.is_some_and(|selected| !selected.contains(&id)) {
                continue;
            }
            let (output, input) = mpsc::channel(1);
            output
                .send(InstanceSamplesRequest {
                    frame: Some(Frame::Open(SamplesSelection {
                        stream: id.clone(),
                        scope: operations.call_context().get::<crate::Scope>().map(|scope| scope.as_str().to_owned()).unwrap_or_default(),
                    })),
                })
                .await
                .map_err(invalid)?;
            let input = stream::unfold(input, |mut input| async { input.recv().await.map(|frame| (frame, input)) });
            let input =
                activation.connection.client().samples(input).await.map_err(|error| Error::Transport(error.to_string()))?.into_inner();
            demands.push(PreparedDemand { id, output, input, row, cadence });
        }
        Ok(PreparedPublications { declarations, demands })
    }
    pub(super) fn activate<A: Application>(
        prepared: PreparedPublications, streams: &StreamManager<A>, activation: &Activation, clock: &Clock,
        operations: &crate::operations::Manager, settings: &PlacementSettings, replace: bool,
    ) -> Result<Self, Error> {
        let sampling = prepared.declarations.iter().map(|declaration| (declaration.id.clone(), watch::channel(0).0)).collect();
        let registration = Arc::new(streams.connect_publications(activation.owner(), prepared.declarations, replace).map_err(invalid)?);
        let mut publications =
            Self::start(prepared.demands, registration.clone(), activation, clock, clock, operations, None, Some(&sampling));
        activation
            .publications
            .set(ScopeSource {
                registration,
                close: publications.close.subscribe(),
                settings: settings.clone(),
                operations: operations.clone(),
                bindings: Mutex::default(),
                sampling,
            })
            .map_err(|_| invalid("Activate Samples: activation already has publications"))?;
        publications.source = Some(activation.publications.clone());
        Ok(publications)
    }

    /// A Scope's Publications Before Its Samples Start: No Demand, but Settlement Still Covers Its Calls.
    pub(super) fn idle(
        source: &ScopeSource, activation: &Activation, clock: &Clock, host: &Clock, operations: &crate::operations::Manager,
    ) -> Self {
        Self::start(Vec::new(), source.registration.clone(), activation, clock, host, operations, Some(source.close.clone()), None)
    }

    pub(super) fn scoped(
        prepared: PreparedPublications, source: &ScopeSource, activation: &Activation, clock: &Clock, host: &Clock,
        operations: &crate::operations::Manager,
    ) -> Self {
        Self::start(prepared.demands, source.registration.clone(), activation, clock, host, operations, Some(source.close.clone()), None)
    }

    #[allow(clippy::too_many_arguments)]
    fn start(
        demands: Vec<PreparedDemand>, registration: Arc<ConnectionStreams>, activation: &Activation, clock: &Clock, host: &Clock,
        operations: &crate::operations::Manager, parent: Option<watch::Receiver<bool>>,
        sampling: Option<&BTreeMap<String, watch::Sender<usize>>>,
    ) -> Self {
        let close = watch::channel(false).0;
        let (failed, failures) = mpsc::unbounded_channel();
        let mut publications = Self {
            plugin: activation.description.id.clone(),
            close,
            jobs: tokio::task::JoinSet::new(),
            failures,
            registration,
            parent,
            source: None,
        };
        for prepared in demands {
            let activity = clock.activity(format!("Publish {}", prepared.id));
            let lease = publications
                .registration
                .leases()
                .iter()
                .find(|lease| lease.stream().definition().id() == prepared.id)
                .expect("staged stream lease")
                .clone();
            let close = publications.close.subscribe();
            let parent = publications.parent.clone();
            let connection = activation.connection.clone();
            let clock = clock.clone();
            let host = host.clone();
            let operations = operations.clone();
            // Only the Unscoped Demand Has a Gate; Its Failures Go to the Attachment, a Scope's to Its Settlement.
            let gate = sampling.map(|sampling| Gate {
                sampling: sampling[&prepared.id].subscribe(),
                interest: lease.stream().demand(),
            });
            let failed = gate.is_some().then(|| failed.clone());
            let caller = Arc::new(super::CallingPlugin {
                name: activation.entry_name.clone(),
                connection: activation.connection.clone(),
                provider: activation.provider.clone(),
                description: activation.description.clone(),
            });
            publications.jobs.spawn(async move {
                let demand = Demand {
                    output: prepared.output,
                    input: prepared.input,
                    row: prepared.row,
                    cadence: prepared.cadence,
                    gate,
                    lease,
                    host,
                    close,
                    parent,
                    closed: connection.closed.subscribe(),
                    operations,
                    caller,
                };
                let result = run(&clock, activity, demand).await;
                let Err(error) = result else { return Ok(()) };
                tracing::warn!(%error, "plugin publication ended");
                // A Stream's Own Failure Ends Its Demand Only; a Transport Failure Ends the Connection.
                if !matches!(
                    error,
                    Error::Invalid(_)
                        | Error::Lagging { .. }
                        | Error::PoolFull { .. }
                        | Error::WindowExceedsPool { .. }
                        | Error::WindowFailed { .. }
                ) {
                    connection.closed.send_replace(true);
                    return Err(error);
                }
                match failed {
                    Some(failed) => {
                        let _ = failed.send(Arc::new(error));
                        Ok(())
                    }
                    None => Err(error),
                }
            });
        }
        publications
    }

    /// Settle One Binding: Signal Close, Cancel Tracked Work, Drain by the Deadline, Then Revoke Leases.
    pub(super) async fn settle(
        &mut self, clock: &Clock, deadline: Instant, unit: Option<&Unit>, action: &'static str,
    ) -> Result<(), Error> {
        // Close Scoped Admission Before Signalling So No Binding Joins After This Snapshot.
        let bindings = self.source.as_ref().and_then(|source| source.get()).map(|source| {
            let mut bindings = source.bindings.lock().expect("scoped bindings lock is healthy");
            bindings.closed = true;
            let open = bindings.scopes.values().filter(|outcome| outcome.borrow().is_none()).cloned().collect::<Vec<_>>();
            (std::mem::take(&mut bindings.tasks), open)
        });
        self.close.send_replace(true);
        let pending = unit.map(Unit::stop).unwrap_or_default();
        let scoped = self.parent.is_some();
        let jobs = &mut self.jobs;
        // Demands Drain and Cancelled Calls Settle Together, So Each Has the Whole Grace.
        let drain = clock.timeout_at(deadline, async {
            let mut failure = None;
            while let Some(result) = jobs.join_next().await {
                if let Err(error) = result.map_err(invalid).and_then(|result| result)
                    && scoped
                {
                    failure.get_or_insert(error);
                }
            }
            failure
        });
        let calls = async {
            let mut failure = None;
            for instance in &pending {
                match clock.timeout_at(deadline, instance.settle()).await {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        failure.get_or_insert_with(|| invalid(format!("{action}: {error}")));
                    }
                    Err(_) => {
                        failure.get_or_insert(Error::Deadline { action });
                    }
                }
            }
            failure
        };
        let (drained, called) = tokio::join!(drain, calls);
        let mut failure = match drained {
            Ok(failure) => failure,
            Err(_) => {
                tracing::warn!(plugin = %self.plugin, %action, outcome = "deadline", "plugin publication drain wait gave up at settlement grace deadline");
                self.jobs.abort_all();
                while self.jobs.join_next().await.is_some() {}
                Some(Error::Deadline { action })
            }
        };
        failure = failure.or(called);
        if let Some((mut tasks, open)) = bindings {
            let joined = clock.timeout_at(deadline, async { while tasks.join_next().await.is_some() {} }).await;
            if joined.is_err() {
                tasks.abort_all();
                while tasks.join_next().await.is_some() {}
                failure.get_or_insert(Error::Deadline { action });
            }
            for outcome in open {
                if let Some(Err(error)) = outcome.borrow().as_ref() {
                    failure.get_or_insert_with(|| invalid(format!("Close Scoped Samples: {error}")));
                }
            }
            // Leases Belong to the Activation, So Only Its Own Settlement Revokes Them.
            for lease in self.registration.leases() {
                lease.revoke();
            }
        }
        failure.map_or(Ok(()), Err)
    }

    /// The Next Failure of an Unscoped Demand; Pending While None Has Failed.
    pub(super) async fn failure(&mut self) -> Arc<Error> {
        match self.failures.recv().await {
            Some(failure) => failure,
            None => std::future::pending().await,
        }
    }
}

impl Gate {
    fn open(&mut self) -> bool {
        let sampled = *self.sampling.borrow_and_update() > 0;
        let watched = *self.interest.borrow_and_update() > 0;
        watched && !sampled
    }

    /// Wait Until the Gate Is `open`; a Closed Channel Never Changes It Again.
    async fn until(&mut self, open: bool) {
        while self.open() != open {
            tokio::select! {
                changed = self.sampling.changed() => if changed.is_err() { std::future::pending::<()>().await },
                changed = self.interest.changed() => if changed.is_err() { std::future::pending::<()>().await },
            }
        }
    }
}

impl ScopeSource {
    /// Count One Scope Sampling Each of `streams` Until the Returned Share Is Dropped.
    pub(super) fn sampling(&self, streams: &BTreeSet<String>) -> Sampling {
        let counts = streams.iter().map(|stream| self.sampling[stream].clone()).collect::<Vec<_>>();
        for count in &counts {
            count.send_modify(|scopes| *scopes += 1);
        }
        Sampling(counts)
    }
}

impl Drop for Sampling {
    fn drop(&mut self) {
        for count in &self.0 {
            count.send_modify(|scopes| *scopes -= 1);
        }
    }
}

impl Drop for Publications {
    fn drop(&mut self) {
        self.close.send_replace(true);
        self.jobs.abort_all();
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid(error: impl std::fmt::Display) -> Error {
    Error::Invalid(error.to_string())
}

/// Resolve an Interval Stream's Period and Window Length; Device Streams Have Neither.
fn cadence(
    stream: &model::StreamDeclaration, values: &model::SettingValues, settings: &PlacementSettings,
) -> Result<Option<Cadence>, Error> {
    let mut interval = stream.interval;
    if !stream.interval_setting.is_empty() {
        interval = Some(duration_setting(values, &stream.interval_setting, "Read Stream Interval")?);
    }
    let Some(interval) = interval else {
        if !stream.window_setting.is_empty() {
            return Err(invalid("Read Stream Window: a device stream has no window; remove its window_setting"));
        }
        return Ok(None);
    };
    let period = positive(interval, "Stream Interval")?;
    if period.as_nanos() > u128::from(u64::MAX) {
        return Err(invalid("Read Stream Interval: declare an interval shorter than 584 years"));
    }
    let window = match stream.window_setting.as_str() {
        "" => settings.interval_window,
        setting => positive(duration_setting(values, setting, "Read Stream Window")?, "Stream Window")?,
    };
    if window.is_zero() {
        return Err(invalid("Read Stream Window: configure a positive PlacementSettings::interval_window"));
    }
    Ok(Some(Cadence {
        period,
        window,
        lag: settings.interval_lag.of(window),
    }))
}

fn duration_setting(values: &model::SettingValues, id: &str, action: &str) -> Result<prost_types::Duration, Error> {
    let value = values
        .values
        .get(id)
        .and_then(|value| value.value.as_ref())
        .ok_or_else(|| invalid(format!("{action}: setting `{id}` is not declared; declare it in the plugin's settings")))?;
    let model::setting_value::Value::Duration(value) = value else {
        return Err(invalid(format!("{action}: setting `{id}` must be a duration")));
    };
    Ok(*value)
}

fn positive(value: prost_types::Duration, name: &str) -> Result<Duration, Error> {
    if value.seconds < 0 || !(0..1_000_000_000).contains(&value.nanos) || value.seconds > 315_576_000_000 {
        return Err(invalid(format!("Invalid {name}")));
    }
    let period = Duration::new(value.seconds as u64, value.nanos as u32);
    if period.is_zero() {
        return Err(invalid(format!("{name} Must Be Positive")));
    }
    Ok(period)
}

/// A Refused Admission: a Full Shared Pool Names the Stream and Its Limits.
fn admission(stream: &str, error: StreamPublicationError) -> Error {
    match error {
        StreamPublicationError::Capacity => Error::PoolFull {
            stream: stream.to_owned(),
            bytes: crate::streams::RETAINED_BYTES,
            rows: crate::streams::RETAINED_ROWS,
        },
        error => invalid(error),
    }
}

/// Recover the Window Operation's Own Failure; Its Carrier Exposes It Only as a Shared `dyn Error` Whose Source Skips It.
fn sample_error(error: crate::Error) -> Error {
    let mut source: Option<&(dyn error::Error + 'static)> = Some(&error);
    while let Some(current) = source {
        let shared = current.downcast_ref::<Arc<dyn error::Error + Send + Sync>>().and_then(|shared| shared.downcast_ref::<Error>());
        let failure = current.downcast_ref::<Error>().or(shared);
        match failure {
            Some(Error::Invalid(message)) => return Error::Invalid(message.clone()),
            Some(Error::WindowFailed { stream, status, message }) => {
                return Error::WindowFailed {
                    stream: stream.clone(),
                    status: *status,
                    message: message.clone(),
                };
            }
            _ => source = current.source(),
        }
    }
    invalid(error)
}

/// The Span of `nanos` Nanoseconds, Refused Past the Clock's Range.
fn span(nanos: u128) -> Result<Duration, Error> {
    let seconds = u64::try_from(nanos / 1_000_000_000).map_err(|_| invalid("Grant Sample Window: the schedule left the clock's range"))?;
    Ok(Duration::new(seconds, (nanos % 1_000_000_000) as u32))
}

fn unix_nanos(date: SystemTime) -> Result<i64, Error> {
    let nanos = match date.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(after) => i128::try_from(after.as_nanos()).ok(),
        Err(before) => i128::try_from(before.duration().as_nanos()).ok().map(|nanos| -nanos),
    };
    nanos
        .and_then(|nanos| i64::try_from(nanos).ok())
        .ok_or_else(|| invalid("Grant Sample Window: sample times must fall between 1678 and 2262; move the clock's date"))
}

/// The Context JSON Each Stored Row Carries: the Trusted Scope and a Device's Own Time.
fn row_context(operations: &crate::operations::Manager, source_time: Option<prost_types::Timestamp>) -> Option<String> {
    match operations.call_context().get::<crate::Scope>() {
        Some(scope) => {
            let mut context = serde_json::Map::new();
            context.insert("scope".into(), serde_json::json!(scope.as_str()));
            if let Some(at) = source_time {
                context.insert("source_time".into(), serde_json::json!({"seconds": at.seconds, "nanos": at.nanos}));
            }
            Some(serde_json::Value::Object(context).to_string())
        }
        None => source_time.map(|at| format!("{{\"source_time\":{{\"seconds\":{},\"nanos\":{}}}}}", at.seconds, at.nanos)),
    }
}

async fn run(clock: &Clock, activity: crate::clock::Activity, demand: Demand) -> Result<(), Error> {
    let Demand {
        output,
        input,
        row,
        cadence,
        mut gate,
        lease,
        host,
        close,
        mut parent,
        closed,
        operations,
        caller,
    } = demand;
    let stream = lease.stream().definition().id().to_owned();
    let input = Arc::new(sync::Mutex::new(input));
    let seen = Arc::new(Mutex::new(BTreeSet::new()));
    let mut ingested_through = None;
    // A Scoped Interval Demand Joins Its Unit's Window Schedule; Its Windows' Call Trees Are Marked.
    let unit = cadence.and(operations.call_context().get::<Arc<Unit>>().cloned());
    let operations = match &unit {
        Some(_) => {
            let mut context = operations.call_context().clone();
            context.insert(WindowCall).map_err(invalid)?;
            operations.with_context(context)
        }
        None => operations,
    };
    let outcome = {
        let work = async {
            let Some(Cadence { period, window, lag }) = cadence else {
                // A Device Stream Publishes Whenever Its Device Produces a Sample.
                let mut sequence = 0;
                loop {
                    let response = input
                        .lock()
                        .await
                        .message()
                        .await
                        .map_err(|error| Error::Transport(error.to_string()))?
                        .ok_or_else(|| invalid("Read Device Samples: demand ended"))?;
                    if matches!(response.frame, Some(instance_samples_response::Frame::Batch(_))) {
                        return Err(invalid("Read Device Samples: a device stream answers with samples, not batches"));
                    }
                    if response.frame.is_some() {
                        return Err(invalid("Read Device Samples: child calls require an unanswered window"));
                    }
                    if response.samples.len() != 1 {
                        return Err(invalid("Expected One Sample per Message"));
                    }
                    let _sample = clock.activity("plugin sample ingest");
                    let _ingesting = host.activity("Ingest Accepted Samples");
                    let sample = response.samples.into_iter().next().expect("one sample");
                    sequence += 1;
                    if sample.sequence != sequence {
                        return Err(invalid("Invalid Stream Sequence"));
                    }
                    if sample.at.is_some_and(|at| {
                        !(-62_135_596_800..=253_402_300_799).contains(&at.seconds) || !(0..1_000_000_000).contains(&at.nanos)
                    }) {
                        return Err(invalid("Invalid Sample Time"));
                    }
                    let published: prost_types::Timestamp = clock.date().into();
                    let payload = row.decode(&sample.row)?;
                    let receipt = lease
                        .admit_clocked_row(
                            PublicationRow {
                                payload,
                                published_unix_micros: Some(published.seconds * 1_000_000 + i64::from(published.nanos) / 1000),
                                context: row_context(&operations, sample.at),
                            },
                            &[clock, &host],
                        )
                        .map_err(|error| admission(&stream, error))?;
                    ingested_through = Some(receipt);
                    lease.ingested_at(receipt).await.map_err(invalid)?;
                }
            };
            // Samples Sit on a Fixed Grid From the Origin; Window Ends Sit on Their Own Grid From It.
            let period_nanos = period.as_nanos();
            let window_nanos = window.as_nanos();
            let wire_period = prost_types::Duration::try_from(period).map_err(invalid)?;
            let mut origin = clock.read();
            let mut granted = 0_u128;
            let mut sequence = 0_u64;
            let schedule = unit.as_ref().map(|unit| unit.windows.schedule(origin.instant + period));
            loop {
                // The Unscoped Demand Grants Only While Its Stream Is Watched and No Scope of Its Activation
                // Samples That Stream; It Resumes From a Fresh Origin.
                if let Some(gate) = &mut gate
                    && !gate.open()
                {
                    activity.wait(gate.until(true)).await;
                    origin = clock.read();
                    granted = 0;
                    continue;
                }
                let due_nanos = (granted + 1) * period_nanos;
                let due = origin.instant + span(due_nanos)?;
                let end = origin.instant + span(due_nanos.div_ceil(window_nanos) * window_nanos)?;
                let paused = async {
                    match &mut gate {
                        Some(gate) => gate.until(false).await,
                        None => std::future::pending().await,
                    }
                };
                let cut = async {
                    match &schedule {
                        Some(schedule) => schedule.cut(due).await,
                        None => std::future::pending().await,
                    }
                };
                let wake = async {
                    tokio::select! {
                        biased;
                        frame = async { input.lock().await.message().await } => {
                            Err(invalid(format!("Read Interval Samples Outside a Window: {frame:?}")))
                        }
                        () = paused => Ok(false),
                        () = clock.sleep_until(end) => Ok(true),
                        () = cut => Ok(true),
                    }
                };
                if !activity.wait(wake).await? {
                    continue;
                }
                // One Reading Ends the Window: a Late or Advanced Clock Extends It to Where Time Reached.
                let reading = clock.read();
                // A Window Ending Further Past Its End Than the Lag Tolerates Fails the Demand Instead of Growing.
                let behind = reading.instant.saturating_duration_since(end);
                if behind > lag {
                    return Err(Error::Lagging {
                        stream: stream.clone(),
                        lag: behind,
                        limit: lag,
                    });
                }
                let through = reading.instant.saturating_duration_since(origin.instant).as_nanos() / period_nanos;
                if through <= granted {
                    continue;
                }
                let count = u64::try_from(through - granted).map_err(|_| invalid("Grant Sample Window: too many samples are due"))?;
                // A Window Even the Empty Shared Pool Cannot Hold Is Refused Before the Plugin Samples It.
                let (rows, payload) = row.least(count);
                let (needed, fits) = crate::streams::claim(rows, payload);
                if !fits {
                    return Err(Error::WindowExceedsPool {
                        stream: stream.clone(),
                        samples: count,
                        needed,
                        bytes: crate::streams::RETAINED_BYTES,
                        rows: crate::streams::RETAINED_ROWS,
                    });
                }
                let start = origin.date + span((granted + 1) * period_nanos)?;
                let start_nanos = unix_nanos(start)?;
                let grant = Window {
                    start: Some(start.into()),
                    period: Some(wire_period),
                    count,
                    sequence: sequence + 1,
                };
                let parts = activity
                    .wait(operations.call_native::<super::samples::Window>(super::samples::Request {
                        input: input.clone(),
                        output: output.clone(),
                        caller: caller.clone(),
                        seen: seen.clone(),
                        lease: lease.clone(),
                        window: grant,
                    }))
                    .await
                    .map_err(sample_error)?;
                // Accepted Ingestion Is Host Work: It Holds Both the Demand's and the Runtime Clock.
                let _window = clock.activity("plugin window ingest");
                let _ingesting = host.activity("Ingest Accepted Samples");
                let context = row_context(&operations, None);
                let mut receipt = None;
                for part in parts {
                    let first = i128::from(start_nanos) + i128::try_from(u128::from(part.offset) * period_nanos).map_err(invalid)?;
                    let first = i64::try_from(first).map_err(|_| invalid("Grant Sample Window: sample times must fall before 2262"))?;
                    let mut stored = Vec::with_capacity(part.columns.len() + 32);
                    prost::encoding::int64::encode(harmos_protocol::columns::batch::START_UNIX_NANOS, &first, &mut stored);
                    prost::encoding::uint64::encode(harmos_protocol::columns::batch::PERIOD_NANOS, &(period_nanos as u64), &mut stored);
                    prost::encoding::uint64::encode(harmos_protocol::columns::batch::COUNT, &part.count, &mut stored);
                    prost::encoding::bytes::encode(harmos_protocol::columns::batch::COLUMNS, &part.columns, &mut stored);
                    for (first, _, payload) in row.decode_batch(&stored, part.count, period_nanos as u64)? {
                        receipt = Some(
                            lease
                                .admit_clocked_row(
                                    PublicationRow {
                                        payload,
                                        published_unix_micros: Some(first.div_euclid(1000)),
                                        context: context.clone(),
                                    },
                                    &[clock, &host],
                                )
                                .map_err(|error| admission(&stream, error))?,
                        );
                    }
                }
                let receipt = receipt.expect("an answered window has at least one part");
                ingested_through = Some(receipt);
                lease.ingested_at(receipt).await.map_err(invalid)?;
                granted = through;
                sequence += count;
                if let Some(schedule) = &schedule {
                    schedule.due(origin.instant + span((granted + 1) * period_nanos)?);
                }
            }
        };
        tokio::pin!(work);
        let requested = close.clone();
        let parent_requested = parent.clone();
        tokio::select! {
            biased;
            () = super::disconnected(closed) => {
                if parent.is_none() { return Ok(()); }
                Err(Error::Transport("Close Scoped Samples: plugin connection closed".into()))
            },
            () = super::disconnected(close.clone()) => Ok(()),
            () = async {
                match &mut parent {
                    Some(parent) => super::disconnected(parent.clone()).await,
                    None => std::future::pending().await,
                }
            } => Ok(()),
            result = &mut work => {
                // Close Requested First Owns the Outcome of an Attempt It Cancelled.
                if *requested.borrow() || parent_requested.as_ref().is_some_and(|parent| *parent.borrow()) {
                    Ok(())
                } else if parent.is_none() {
                    return result;
                } else {
                    result
                }
            },
        }
    };
    if let Some(published) = ingested_through {
        // Accepted Ingestion Is Host Work, So It Holds the Runtime Clock Until It Completes.
        let _ingesting = host.activity("Drain Accepted Samples");
        if parent.is_some() {
            lease.drained_at(published).await.map_err(|error| invalid(format!("Close Scoped Samples: {error}")))?;
        } else {
            lease.ingested_at(published).await.map_err(invalid)?;
        }
    }
    outcome?;
    drop(output);
    // Input EOF Ends Demand and Awaits the SDK's Producer/Observer Cleanup; That Wait Is on the Plugin.
    tracing::debug!(plugin = %caller.description.id, %stream, "waiting for plugin Samples response EOF");
    let drained = activity
        .wait(async {
            while input.lock().await.message().await.map_err(|error| Error::Transport(error.to_string()))?.is_some() {}
            Ok(())
        })
        .await;
    tracing::debug!(
        plugin = %caller.description.id,
        %stream,
        outcome = if drained.is_ok() { "eof" } else { "failed" },
        error = ?drained.as_ref().err(),
        "plugin Samples response EOF wait completed"
    );
    drained
}

/* === Tests === */
