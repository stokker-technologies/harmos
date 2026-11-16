/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Connection-Owned gRPC Plugin Placement

/* === Modules === */

mod catalog;
mod config;
mod connection;
pub(crate) mod invoke;
mod load;
mod managed;
mod publish;
pub(crate) mod samples;
mod scoped;
/* === Re-Exports === */

pub use config::{IntervalLag, PlacementSettings};
pub use harmos_protocol::model;
pub use invoke::CallError;
pub use load::{Load, LoadOptions};
pub use managed::Managed;
pub use scoped::ScopedBinding;

/* === Imports === */

use harmos_protocol::plugin::InstanceGetRequest;
use harmos_protocol::plugin::InstanceGetResponse;
use harmos_protocol::plugin::InstanceStopRequest;

// Standard Library Imports
use std::{future::Future, sync::Arc};

// Third Party Imports
use tokio::sync::{oneshot, watch};

// Local Imports
use crate::{Application, Runtime, clock::Clock};
use connection::Connection;
use harmos_protocol::model::SettingValues;
use managed::Process;
use publish::{PreparedPublications, Publications};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Outcome = Result<(), Arc<Error>>;

/* === Structs === */

#[derive(Clone)]
pub struct CallingPlugin {
    pub name: Option<String>,
    pub description: InstanceGetResponse,
    connection: Arc<Connection>,
    provider: String,
}

#[derive(Clone)]
pub struct Activation {
    pub entry_name: Option<String>,
    provider: String,
    pub settings: SettingValues,
    pub number: u64,
    pub description: InstanceGetResponse,
    pub previous_started_at: Option<prost_types::Timestamp>,
    pub executable_digest: Option<[u8; 32]>,
    pub process_id: Option<u32>,
    connection: Arc<Connection>,
    clock: Clock,
    admission: Option<Arc<crate::operations::GrpcAdmission>>,
    publications: Arc<std::sync::OnceLock<publish::ScopeSource>>,
}

#[derive(Clone)]
pub struct Status {
    pub activation: Option<Activation>,
    pub failure: Option<Arc<Error>>,
}

pub struct Attached {
    status: watch::Receiver<Status>,
    stop: watch::Sender<Option<String>>,
    stopped: watch::Receiver<Option<Outcome>>,
}

pub(crate) struct PreparedPlugin {
    process: Option<Process>,
    address: String,
    pub(crate) activation: Activation,
    admission: crate::operations::GrpcAdmission,
    publications: PreparedPublications,
}

struct Startup {
    participant: crate::runtime::StopParticipant,
    process_participant: Option<crate::runtime::StopParticipant>,
    entry: Option<EntrySettings>,
    cancelled: Option<oneshot::Receiver<()>>,
    options: LoadOptions,
}

struct EntrySettings {
    name: String,
    values: Option<SettingValues>,
}

struct ConnectedPlugin {
    process: Option<Process>,
    address: String,
    activation: Activation,
    publications: Publications,
}

struct Session<A: Application> {
    operations: crate::operations::Manager,
    streams: crate::streams::StreamManager<A>,
    lane: &'static str,
    clock: Clock,
    settings: PlacementSettings,
    process: Option<Process>,
    address: String,
    activation: Activation,
    publications: Publications,
    stop: watch::Receiver<Option<String>>,
    participant: crate::runtime::StopParticipant,
}

/* === Enums === */

#[derive(Clone, Debug)]
pub enum Placement {
    Managed(Managed),
    Remote(String),
}

enum Ending {
    Stop(String),
    Exit(Result<std::process::ExitStatus, Error>),
    Disconnect,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),

    #[error("{0}")]
    Reap(String),

    #[error("plugin transport failed: {0}")]
    Transport(String),

    #[error("plugin connection closed")]
    Disconnected,

    #[error("plugin {0} exceeded its host-clock budget")]
    Timeout(&'static str),

    #[error("{action}: the plugin did not settle within PlacementSettings.stop_grace; its remaining work was cancelled")]
    Deadline { action: &'static str },

    #[error(
        "Grant Sample Window: stream `{stream}` fell {lag:?} behind its window end, past PlacementSettings::interval_lag ({limit:?}); lower the stream's declared rate, the number of streams or the window length"
    )]
    Lagging { stream: String, lag: std::time::Duration, limit: std::time::Duration },

    #[error(
        "Publish Samples: stream `{stream}` found the shared plugin publication pool ({bytes} bytes, {rows} rows) full of rows storage has not taken in; lower the declared rates, the number of streams or the window length"
    )]
    PoolFull { stream: String, bytes: usize, rows: usize },

    #[error(
        "Grant Sample Window: stream `{stream}` needs at least {needed} bytes for one window of {samples} samples, more than the shared plugin publication pool holds ({bytes} bytes, {rows} rows); shorten the window length or lower the stream's declared rate"
    )]
    WindowExceedsPool { stream: String, samples: u64, needed: u128, bytes: usize, rows: usize },

    #[error(
        "Answer Sample Window: the plugin failed a window of stream `{stream}` with {status:?}: {message}; correct the plugin's sampling of that stream, then reattach the plugin or start the scope's samples again"
    )]
    WindowFailed { stream: String, status: tonic::Code, message: String },

    #[error(
        "Load Plugin {plugin}: the plugin did not load within {timeout:?} (configured by {configured_by}); make the plugin start faster, or raise the plugins.load timeout for this load with LoadOptions"
    )]
    LoadDeadline { plugin: String, timeout: std::time::Duration, configured_by: &'static str },

    #[error("managed plugin exited with {0}")]
    Exit(std::process::ExitStatus),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/* === Traits === */

/* === Implementations === */

impl PreparedPlugin {
    pub(crate) async fn stop(&mut self, clock: &Clock, settings: &PlacementSettings) -> Result<(), Error> {
        if let Some(process) = &mut self.process {
            process.terminate(clock, settings.stop_grace, "Stop Staged Plugin Replacement").await?;
        }
        Ok(())
    }
}

impl CallingPlugin {
    pub(crate) fn connection_identity(&self) -> usize {
        Arc::as_ptr(&self.connection) as usize
    }

    pub(crate) fn same_activation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.connection, &other.connection)
    }
    pub(crate) fn owner(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.provider)
    }
}
impl Activation {
    pub(crate) fn owner(&self) -> &str {
        self.entry_name.as_deref().unwrap_or(&self.provider)
    }
}

impl<A: Application> Runtime<A> {
    pub(crate) async fn prepare_grpc_entry(
        &self, name: String, placement: Placement, settings: PlacementSettings, values: Option<SettingValues>,
    ) -> Result<PreparedPlugin, Error> {
        let operations = self.operations.execution();
        let clock = operations.clock();
        let mut participant = self.stop_participant();
        let Placement::Managed(managed) = placement else {
            return Err(Error::Invalid("Replace Plugin: managed placement required".into()));
        };
        if participant.is_stopping() {
            return Err(Error::Invalid("Replace Plugin Startup: host is shutting down".into()));
        }
        // Spawning Is Never Raced Against Shutdown, So Every Forked Child Reaches This Owner.
        let process = Process::spawn(&managed, self.stop_participant()).await?;
        // The Load Borrows the Process From Here, So a Load Its Deadline Ends Still Returns It to This Owner.
        let spawned = Arc::new(tokio::sync::Mutex::new(Some(process)));
        let stage = {
            let spawned = spawned.clone();
            let operations = operations.clone();
            let streams = self.streams.clone();
            let clock = clock.clone();
            let settings = settings.clone();
            let name = name.clone();
            async move {
                let mut held = spawned.lock().await;
                let process = held.as_mut().expect("the spawned process is held until the load ends");
                let address = process.ready(&managed).await?;
                let mut activation =
                    connect(&clock, &address, 1, None, Some(process.digest), operations.transfer_message_bytes().get()).await?;
                activation.entry_name = Some(name);
                activation.settings = crate::plugin::manager::settings::select(
                    activation.description.declarations.as_ref().expect("admitted description"),
                    values,
                )
                .map_err(|error| Error::Invalid(error.to_string()))?;
                activation.process_id = process.id();
                let admission = operations.prepare_grpc_admission(&activation, crate::operations::Lane::default().name(), true)?;
                let publications = Publications::prepare(&streams, &activation, &settings, &operations, true, None).await?;
                Ok((address, activation, admission, publications))
            }
        };
        let result = tokio::select! {
            biased;
            () = participant.stopping() => Err(Error::Invalid("Replace Plugin Startup: host is shutting down".into())),
            result = load::load(&operations, &name, LoadOptions::default(), stage) => result,
        };
        // An Ended Load Has Released the Process, Whether It Finished, Missed Its Deadline or Was Cancelled.
        let mut process = spawned.lock().await.take().expect("the spawned process returns to its owner");
        match result {
            Ok((address, activation, admission, publications)) => Ok(PreparedPlugin {
                process: Some(process),
                address,
                activation,
                admission,
                publications,
            }),
            Err(error) => {
                process.terminate(clock, settings.stop_grace, "Stop Failed Replacement Startup").await?;
                Err(error)
            }
        }
    }
    pub(crate) async fn activate_grpc_entry(&self, prepared: PreparedPlugin, settings: PlacementSettings) -> Result<Attached, Error> {
        let PreparedPlugin {
            mut process,
            address,
            mut activation,
            mut admission,
            publications,
        } = prepared;
        let operations = self.operations.execution();
        let clock = operations.clock().clone();
        let activated = (|| {
            admission.commit().map_err(|error| Error::Invalid(format!("Replace Plugin Operations: {error}")))?;
            Publications::activate(publications, &self.streams, &activation, &clock, &operations, &settings, true)
        })();
        let publications = match activated {
            Ok(publications) => publications,
            Err(error) => {
                if let Some(process) = &mut process {
                    process.terminate(&clock, settings.stop_grace, "Stop Failed Replacement Activation").await?;
                }
                return Err(error);
            }
        };
        admission.seal();
        activation.admission = Some(Arc::new(admission));
        Ok(Attached::launch(
            operations,
            self.streams.clone(),
            crate::operations::Lane::default().name(),
            settings,
            self.stop_participant(),
            ConnectedPlugin {
                process,
                address,
                activation,
                publications,
            },
        ))
    }

    pub(crate) async fn attach_grpc_entry(
        &self, name: String, placement: Placement, settings: PlacementSettings, values: Option<SettingValues>,
        cancelled: oneshot::Receiver<()>,
    ) -> Result<Attached, Error> {
        Attached::start(
            self.operations.execution(),
            self.streams.clone(),
            crate::operations::Lane::default().name(),
            placement,
            settings,
            Startup {
                participant: self.stop_participant(),
                process_participant: Some(self.stop_participant()),
                entry: Some(EntrySettings { name, values }),
                cancelled: Some(cancelled),
                options: LoadOptions::default(),
            },
        )
        .await
    }

    pub async fn attach_grpc(&self, placement: Placement, settings: PlacementSettings) -> Result<Attached, Error> {
        self.attach_grpc_with(placement, settings, LoadOptions::default()).await
    }

    /// Attach Like `attach_grpc`, With `options` Applying to This Plugin Load Only.
    pub async fn attach_grpc_with(
        &self, placement: Placement, settings: PlacementSettings, options: LoadOptions,
    ) -> Result<Attached, Error> {
        // Startup Itself Refuses a Stopping Host Before It Spawns Anything.
        let participant = self.stop_participant();
        Attached::start(
            self.operations.execution(),
            self.streams.clone(),
            crate::operations::Lane::default().name(),
            placement,
            settings,
            Startup {
                participant,
                process_participant: Some(self.stop_participant()),
                entry: None,
                cancelled: None,
                options,
            },
        )
        .await
    }
}

impl Activation {
    pub fn is_connected(&self) -> bool {
        !*self.connection.closed.borrow()
    }

    /// Inspect This Connection Only; An Old Handle Never Follows Reconnection.
    pub async fn get(&self) -> Result<InstanceGetResponse, Error> {
        self.connected(describe(&self.connection)).await
    }

    async fn connected<T>(&self, future: impl Future<Output = Result<T, Error>>) -> Result<T, Error> {
        let _activity = self.clock.activity("plugin transport");
        let closed = self.connection.closed.subscribe();
        tokio::select! {
            biased;
            () = disconnected(closed) => Err(Error::Disconnected),
            result = future => result,
        }
    }
}

impl Attached {
    async fn start<A: Application>(
        operations: crate::operations::Manager, streams: crate::streams::StreamManager<A>, lane: &'static str, placement: Placement,
        settings: PlacementSettings, startup: Startup,
    ) -> Result<Self, Error> {
        let (cancel, dropped) = oneshot::channel();
        let task = tokio::spawn(Self::start_owned(operations, streams, lane, placement, settings, startup, dropped));
        let result = task.await.map_err(|error| Error::Invalid(format!("Join Plugin Startup: {error}")))?;
        drop(cancel);
        result
    }

    async fn start_owned<A: Application>(
        operations: crate::operations::Manager, streams: crate::streams::StreamManager<A>, lane: &'static str, placement: Placement,
        settings: PlacementSettings, startup: Startup, dropped: oneshot::Receiver<()>,
    ) -> Result<Self, Error> {
        let Startup {
            mut participant,
            mut process_participant,
            entry,
            cancelled,
            options,
        } = startup;
        let clock = operations.clock().clone();
        if settings.reconnect_delay.is_zero() || settings.stop_grace.is_zero() || settings.interval_window.is_zero() {
            return Err(Error::Invalid("Placement Durations Must Be Positive".into()));
        }
        // A Stopping Host Spawns Nothing It Would Immediately Have to Reap.
        if participant.is_stopping() {
            return Err(Error::Invalid("Start Plugin: the host is shutting down; attach the plugin after it restarts".into()));
        }
        let _activity = clock.activity("plugin startup");
        let plugin = match (&entry, &placement) {
            (Some(entry), _) => entry.name.clone(),
            (None, Placement::Managed(managed)) => managed.executable.display().to_string(),
            (None, Placement::Remote(address)) => address.clone(),
        };
        // Spawning Is Never Raced Against Cancellation, So Every Forked Child Reaches This Owner.
        let process = match &placement {
            Placement::Managed(managed) => {
                Some(Process::spawn(managed, process_participant.take().expect("managed process participant")).await?)
            }
            Placement::Remote(_) => None,
        };
        // The Load Borrows the Process From Here, So a Load Its Deadline Ends Still Returns It to This Owner.
        let spawned = Arc::new(tokio::sync::Mutex::new(process));
        let setup = {
            let spawned = spawned.clone();
            let operations = operations.clone();
            let streams = streams.clone();
            let clock = clock.clone();
            let settings = settings.clone();
            let placement = placement.clone();
            async move {
                let mut process = spawned.lock().await;
                let address = match &placement {
                    Placement::Managed(managed) => process.as_mut().expect("spawned process").ready(managed).await?,
                    Placement::Remote(address) => address.clone(),
                };
                let mut activation = connect(
                    &clock,
                    &address,
                    1,
                    None,
                    process.as_ref().map(|process| process.digest),
                    operations.transfer_message_bytes().get(),
                )
                .await?;
                let values = entry.and_then(|entry| {
                    activation.entry_name = Some(entry.name);
                    entry.values
                });
                activation.settings = crate::plugin::manager::settings::select(
                    activation.description.declarations.as_ref().expect("admitted description"),
                    values,
                )
                .map_err(|error| Error::Invalid(error.to_string()))?;
                activation.process_id = process.as_ref().and_then(Process::id);
                let admission = operations.admit_grpc(&activation, lane).inspect_err(|_| {
                    activation.connection.closed.send_replace(true);
                })?;
                activation.admission = Some(Arc::new(admission));
                let publications = Publications::open(&streams, &activation, &clock, &settings, &operations).await?;
                Ok::<_, Error>((address, activation, publications))
            }
        };
        let cancelled = async {
            match cancelled {
                Some(cancelled) => {
                    let _ = cancelled.await;
                }
                None => std::future::pending().await,
            }
        };
        let result = tokio::select! {
            biased;
            _ = dropped => Err(Error::Invalid("Cancel Plugin Startup: attachment dropped".into())),
            () = cancelled => Err(Error::Invalid("Cancel Plugin Startup: entry stopped".into())),
            () = participant.stopping() => Err(Error::Invalid("Stop Plugin Startup: host is shutting down".into())),
            result = load::load(&operations, &plugin, options, setup) => result,
        };
        // An Ended Load Has Released the Process, Whether It Finished, Missed Its Deadline or Was Cancelled.
        let mut process = spawned.lock().await.take();
        let (address, activation, publications) = match result {
            Ok(connected) => connected,
            Err(error) => {
                if let Some(process) = &mut process {
                    process.terminate(&clock, settings.stop_grace, "Stop Plugin Startup").await?;
                }
                return Err(error);
            }
        };
        let connected = ConnectedPlugin {
            process,
            address,
            activation,
            publications,
        };
        Ok(Self::launch(operations, streams, lane, settings, participant, connected))
    }

    fn launch<A: Application>(
        operations: crate::operations::Manager, streams: crate::streams::StreamManager<A>, lane: &'static str, settings: PlacementSettings,
        participant: crate::runtime::StopParticipant, connected: ConnectedPlugin,
    ) -> Self {
        let ConnectedPlugin {
            process,
            address,
            activation,
            publications,
        } = connected;
        let clock = operations.clock().clone();
        let (status, receiver) = watch::channel(Status {
            activation: Some(activation.clone()),
            failure: None,
        });
        let (stop, stopping) = watch::channel(None);
        let (stopped, completed) = watch::channel(None);
        tokio::spawn(async move {
            let session = Session {
                operations,
                streams,
                lane,
                clock,
                settings,
                process,
                address,
                activation,
                publications,
                stop: stopping,
                participant,
            };
            let result = supervise(session, &status).await;
            status.send_modify(|status| {
                status.activation = None;
                if let Err(error) = &result {
                    status.failure = Some(error.clone());
                }
            });
            stopped.send_replace(Some(result));
        });
        Self {
            status: receiver,
            stop,
            stopped: completed,
        }
    }

    pub fn status(&self) -> watch::Receiver<Status> {
        self.status.clone()
    }

    pub fn activation(&self) -> Option<Activation> {
        self.status.borrow().activation.clone()
    }

    pub async fn stop(&self, reason: impl Into<String>) -> Outcome {
        self.stop.send_if_modified(|current| {
            if current.is_some() {
                return false;
            }
            *current = Some(reason.into());
            true
        });
        let mut stopped = self.stopped.clone();
        loop {
            if let Some(result) = stopped.borrow_and_update().clone() {
                return result;
            }
            if stopped.changed().await.is_err() {
                return Err(Arc::new(Error::Disconnected));
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn describe(connection: &Connection) -> Result<InstanceGetResponse, Error> {
    let mut client = connection.client();
    client
        .get(InstanceGetRequest {
            revision: 1,
            host: "harmos".into(),
            host_version: env!("CARGO_PKG_VERSION").into(),
        })
        .await
        .map(|reply| reply.into_inner())
        .map_err(|error| Error::Transport(error.to_string()))
}

async fn connect(
    clock: &Clock, address: &str, number: u64, previous_started_at: Option<prost_types::Timestamp>, executable_digest: Option<[u8; 32]>,
    message_bytes: usize,
) -> Result<Activation, Error> {
    let connection = Arc::new(Connection::open(address, harmos_protocol::transfer::default_message_bytes().get()).await?);
    let description = describe(&connection).await?;
    if description.message_bytes < message_bytes as u64 {
        return Err(Error::Invalid(
            "Instance.Get: peer message_bytes cannot carry StorageSettings.transfer.chunk_bytes plus framing".into(),
        ));
    }
    let valid_time = description
        .started_at
        .is_some_and(|time| (-62_135_596_800..=253_402_300_799).contains(&time.seconds) && (0..1_000_000_000).contains(&time.nanos));
    if description.revision == 0
        || description.id.is_empty()
        || description.sdk.is_empty()
        || description.sdk_version.is_empty()
        || semver::Version::parse(&description.version).is_err()
        || !valid_time
        || description.declarations.is_none()
    {
        return Err(Error::Invalid("Get Plugin returned an invalid response".into()));
    }
    catalog::verify(description.declarations.as_ref().expect("description catalog was checked"))?;
    Ok(Activation {
        number,
        settings: crate::plugin::manager::settings::select(description.declarations.as_ref().expect("admitted description"), None)
            .map_err(|error| Error::Invalid(error.to_string()))?,
        entry_name: None,
        provider: connection.owner.clone(),
        description,
        previous_started_at,
        executable_digest,
        process_id: None,
        connection,
        admission: None,
        publications: Arc::new(std::sync::OnceLock::new()),
        clock: clock.clone(),
    })
}

async fn disconnected(mut closed: watch::Receiver<bool>) {
    while !*closed.borrow_and_update() {
        if closed.changed().await.is_err() {
            break;
        }
    }
}

async fn stopping(stop: &mut watch::Receiver<Option<String>>, participant: &mut crate::runtime::StopParticipant) -> String {
    loop {
        if let Some(reason) = stop.borrow_and_update().clone() {
            return reason;
        }
        tokio::select! {
            result = stop.changed() => if result.is_err() { return "Attachment Dropped".into(); },
            () = participant.stopping() => return "Host Shutting Down".into(),
        }
    }
}

async fn supervise<A: Application>(session: Session<A>, status: &watch::Sender<Status>) -> Outcome {
    let Session {
        operations,
        streams,
        lane,
        clock,
        settings,
        mut process,
        address,
        mut activation,
        mut publications,
        mut stop,
        mut participant,
    } = session;
    loop {
        let ending = loop {
            let exited = async {
                match &mut process {
                    Some(process) => process.exited().await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                biased;
                reason = stopping(&mut stop, &mut participant) => break Ending::Stop(reason),
                result = exited => break Ending::Exit(result),
                () = disconnected(activation.connection.closed.subscribe()) => break Ending::Disconnect,
                // A Failed Unscoped Demand Ends Its Stream Only; the Attachment's Owner Reads Why.
                failure = publications.failure() => status.send_modify(|status| status.failure = Some(failure)),
            }
        };
        // Every Ending Settles Its Bindings Before the Connection or Process Is Released. Teardown
        // Parks Its Activity While It Waits on the Plugin, So Every Clock Kind Reaches Its Deadlines.
        let teardown = clock.activity("plugin teardown");
        if !matches!(ending, Ending::Stop(_)) {
            activation.connection.closed.send_replace(true);
        }
        let settling = publications.settle(&clock, clock.now() + settings.stop_grace, None, "Settle Plugin Publications");
        let settled = teardown.wait(settling).await;
        if let Err(error) = &settled {
            tracing::warn!(plugin = %activation.description.id, action = "Settle Plugin Publications", %error, "plugin publications did not settle");
        }
        activation.admission.as_ref().unwrap().remove();
        // The Plugin Is Told to Stop Before It Is Killed: Stop and Its Exit Get a Fresh `stop_grace`.
        let deadline = clock.now() + settings.stop_grace;
        match ending {
            Ending::Stop(reason) => {
                let mut client = activation.connection.client();
                let stopped = teardown
                    .wait(clock.timeout_at(deadline, client.stop(InstanceStopRequest { reason })))
                    .await
                    .map_err(|_| Error::Timeout("Stop"))
                    .and_then(|result| result.map(|_| ()).map_err(|error| Error::Transport(error.to_string())));
                if let Err(error) = &stopped {
                    tracing::warn!(plugin = %activation.description.id, action = "Stop Plugin", %error, "plugin Stop failed; closing its connection");
                }
                activation.connection.closed.send_replace(true);
                let exit = match &mut process {
                    Some(process) => {
                        match teardown.wait(process.stop(&clock, deadline, settings.stop_grace, "Stop Managed Plugin")).await {
                            Ok(exit) if exit.success() => Ok(()),
                            Ok(exit) => Err(Error::Exit(exit)),
                            Err(error) => Err(error),
                        }
                    }
                    None => Ok(()),
                };
                // A Reap Failure Wins So the Owner Retains the Installation.
                return match exit {
                    Err(error @ Error::Reap(_)) => Err(Arc::new(error)),
                    exit => settled.and(stopped).and(exit).map_err(Arc::new),
                };
            }
            Ending::Exit(result) => return Err(Arc::new(result.map_or_else(|error| error, Error::Exit))),
            Ending::Disconnect => {}
        }
        if let Some(process) = &mut process {
            teardown.wait(process.terminate(&clock, settings.stop_grace, "Stop Disconnected Managed Plugin")).await.map_err(Arc::new)?;
            return Err(Arc::new(Error::Disconnected));
        }
        drop(teardown);
        let failure = settled.err().unwrap_or(Error::Disconnected);
        status.send_replace(Status {
            activation: None,
            failure: Some(Arc::new(failure)),
        });
        let previous = activation.description.started_at;
        let number = activation.number + 1;
        loop {
            tokio::select! {
                biased;
                _ = stopping(&mut stop, &mut participant) => return Ok(()),
                () = clock.sleep(settings.reconnect_delay) => {},
            }
            let _activity = clock.activity("plugin reconnect");
            // One Plugin Load Bounds the Whole Attempt, and Stop Interrupts Every Step of It.
            let attempt = {
                let address = address.clone();
                let operations = operations.clone();
                let streams = streams.clone();
                let clock = clock.clone();
                let settings = settings.clone();
                let previous_activation = activation.clone();
                async move {
                    let mut next = connect(&clock, &address, number, previous, None, operations.transfer_message_bytes().get()).await?;
                    if next.description.id != previous_activation.description.id {
                        next.connection.closed.send_replace(true);
                        return Err(Error::Invalid("Reconnect Changed Plugin Identity".into()));
                    }
                    next.entry_name = previous_activation.entry_name.clone();
                    next.provider = previous_activation.provider.clone();
                    let selected = crate::plugin::manager::settings::select(
                        next.description.declarations.as_ref().expect("admitted description"),
                        Some(previous_activation.settings.clone()),
                    );
                    next.settings = match selected {
                        Ok(settings) => settings,
                        Err(error) => {
                            next.connection.closed.send_replace(true);
                            return Err(Error::Invalid(format!(
                                "Reconnect Plugin: the retained settings do not fit the reconnected plugin ({error}); set settings that match its declarations"
                            )));
                        }
                    };
                    match operations.admit_grpc(&next, lane) {
                        Ok(admission) => next.admission = Some(Arc::new(admission)),
                        Err(error) => {
                            next.connection.closed.send_replace(true);
                            return Err(error);
                        }
                    }
                    match Publications::open(&streams, &next, &clock, &settings, &operations).await {
                        Ok(opened) => Ok((next, opened)),
                        Err(error) => {
                            next.connection.closed.send_replace(true);
                            next.admission.as_ref().unwrap().remove();
                            Err(error)
                        }
                    }
                }
            };
            let reconnected = tokio::select! {
                biased;
                _ = stopping(&mut stop, &mut participant) => return Ok(()),
                result = load::load(&operations, activation.owner(), LoadOptions::default(), attempt) => result,
            };
            let next = match reconnected {
                Ok((next, opened)) => {
                    publications = opened;
                    next
                }
                Err(error) => {
                    status.send_modify(|status| status.failure = Some(Arc::new(error)));
                    continue;
                }
            };
            activation = next;
            status.send_replace(Status {
                activation: Some(activation.clone()),
                failure: None,
            });
            break;
        }
    }
}

/* === Tests === */
