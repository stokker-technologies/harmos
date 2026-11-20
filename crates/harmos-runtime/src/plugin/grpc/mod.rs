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
pub(crate) mod facade;
pub(crate) mod invoke;
mod launches;
mod load;
mod managed;
mod publish;
pub(crate) mod samples;
mod scoped;

/* === Re-Exports === */

pub use config::{GrpcPluginPolicy, GrpcPluginSettings, IntervalLag, PlacementSettings, StartupOutput};
pub use harmos_protocol::model;
pub use invoke::CallError;
pub use launches::{HOST_ENDPOINT, LAUNCH_TOKEN, Launch, Launches};
pub use load::{Load, LoadOptions};
pub use managed::Managed;
pub use scoped::ScopedBinding;

/* === Imports === */

// Standard Library Imports
use std::{
    future::{self, Future},
    process::ExitStatus,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

// Third Party Imports
use prost_types::Timestamp;
use tokio::sync::{self, oneshot, watch};
use tonic::transport::Channel;

// Local Imports
use crate::operations::{GrpcAdmission, Manager};
use crate::runtime::StopParticipant;
use crate::streams::StreamManager;
use crate::{Application, Runtime, clock::Clock};
use connection::Connection;
use harmos_protocol::model::SettingValues;
use harmos_protocol::plugin::instance_client::InstanceClient;
use harmos_protocol::plugin::{InstanceGetRequest, InstanceGetResponse, InstanceStopRequest};
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
    pub previous_started_at: Option<Timestamp>,
    pub executable_digest: Option<[u8; 32]>,
    pub process_id: Option<u32>,
    connection: Arc<Connection>,
    clock: Clock,
    admission: Option<Arc<GrpcAdmission>>,
    publications: Arc<OnceLock<publish::ScopeSource>>,
    finalized: Arc<AtomicBool>,
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
    participant: StopParticipant,
    process: Option<Process>,
    address: String,
    pub(crate) activation: Activation,
    admission: GrpcAdmission,
    publications: PreparedPublications,
}

struct Startup {
    participant: StopParticipant,
    process_participant: Option<StopParticipant>,
    entry: Option<EntrySettings>,
    cancelled: Option<oneshot::Receiver<()>>,
    options: Option<LoadOptions>,
}

struct EntrySettings {
    manager: super::manager::Manager,
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
    operations: Manager,
    streams: StreamManager<A>,
    clock: Clock,
    settings: PlacementSettings,
    process: Option<Process>,
    address: String,
    activation: Activation,
    publications: Publications,
    stop: watch::Receiver<Option<String>>,
    participant: StopParticipant,
}

/* === Enums === */

#[derive(Clone, Debug)]
pub enum Placement {
    Managed(Managed),
    Remote(String),
}

enum Ending {
    HostShutdown,
    Stop(String),
    Exit(Result<ExitStatus, Error>),
    Disconnect,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Inspect Plugin: {0}; correct the candidate or its placement settings before inspecting again")]
    Inspection(#[source] Box<Error>),
    #[error("{operation}: {source}")]
    Lifecycle {
        operation: String,
        #[source]
        source: crate::Error,
    },

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
    Lagging { stream: String, lag: Duration, limit: Duration },

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

    /// The Load Exceeded Its `plugins.load` Deadline; the Text Is the Plugin's Operator-Facing Status.
    ///
    /// A direct library caller can replace the deadline for one load with [`LoadOptions`].
    ///
    /// For a managed process that had not announced, `diagnostics` names it, its exit status and its standard-error tail.
    #[error(
        "Load Plugin {plugin}: the plugin did not load within {timeout:?} (configured by {configured_by}); check the plugin's startup, or raise the plugins.load deadline in the plugin settings (GrpcPluginSettings.policy.load)"
    )]
    LoadDeadline { plugin: String, timeout: Duration, configured_by: &'static str, diagnostics: String },

    /// A Managed Process Ended Before Announcing; `diagnostics` Holds Its Standard-Error Tail.
    #[error(
        "Start Managed Plugin: process {process} exited with {status} before announcing its address; inspect the plugin's startup diagnostics, then restart it"
    )]
    Exited { process: String, status: ExitStatus, diagnostics: String },

    #[error("managed plugin exited with {0}")]
    Exit(ExitStatus),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/* === Traits === */

/* === Implementations === */

impl Error {
    /// Support Facts of a Failed Managed Startup: the Process, Its Exit Status and Its Standard-Error Tail.
    pub fn diagnostics(&self) -> Option<&str> {
        match self {
            Self::LoadDeadline { diagnostics, .. } | Self::Exited { diagnostics, .. } if !diagnostics.is_empty() => Some(diagnostics),
            _ => None,
        }
    }

    pub(crate) fn lifecycle_failure(&self, plugin: &str) -> Option<model::Failure> {
        match self {
            Self::Lifecycle { operation, source } => {
                let mut failure = invoke::failure(source, operation, plugin);
                if failure.operation.is_empty() {
                    failure.operation = operation.clone();
                }
                Some(failure)
            }
            _ => None,
        }
    }
}

impl PreparedPlugin {
    pub(crate) async fn stop(&mut self, clock: &Clock, settings: &PlacementSettings) -> Result<(), Error> {
        if let Some(process) = &mut self.process {
            process.terminate(clock, settings.stop_grace, "Stop Staged Plugin Replacement").await?;
        }
        Ok(())
    }
}

impl CallingPlugin {
    pub(crate) fn is_ready(&self) -> bool {
        self.connection.ready.load(Ordering::Acquire)
    }

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

    fn has_hooks(&self) -> bool {
        self.description
            .declarations
            .as_ref()
            .expect("admitted plugin declarations")
            .operations
            .iter()
            .any(|operation| lifecycle_operation(&self.description.id, &operation.id))
    }

    async fn initialize(&self) -> Result<(), Error> {
        self.hook(".initialize").await
    }

    async fn hook(&self, name: &str) -> Result<(), Error> {
        let operation = format!("{}{name}", self.description.id);
        let admission = self.admission.as_ref().expect("lifecycle follows admission");
        admission.lifecycle(name).await.map_err(|source| Error::Lifecycle { operation, source })
    }

    async fn finalize(&self) -> Result<(), Error> {
        if self.finalized.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        self.connection.ready.store(false, Ordering::Release);
        let admission = self.admission.as_ref().expect("finalize follows admission");
        let settled = admission.settle().await;
        let finalized = self.hook(".finalize").await;
        admission.remove();
        settled
            .map_err(|source| Error::Lifecycle {
                operation: format!("{}.finalize", self.description.id),
                source,
            })
            .and(finalized)
    }
}

impl<A: Application> Runtime<A> {
    pub(crate) async fn prepare_grpc_entry(
        &self, name: String, placement: Placement, settings: PlacementSettings, values: Option<SettingValues>,
    ) -> Result<PreparedPlugin, Error> {
        let operations = self.operations.execution();
        let clock = operations.clock();
        let mut participant = self.plugin_participant();
        let Placement::Managed(managed) = placement else {
            return Err(Error::Invalid("Replace Plugin: managed placement required".into()));
        };
        if participant.is_settling() || participant.is_stopping() {
            return Err(Error::Invalid("Replace Plugin Startup: host is shutting down".into()));
        }
        // Spawning Is Part of the Load. The Load Keeps the Process Here, So One Its Deadline Ends Returns It to This
        // Owner; a Child Forked After the Load Ended Is Killed and Reaped Where the Spawner Drops It.
        let spawned = Arc::new(sync::Mutex::new(None));
        let process_participant = self.stop_participant();
        let launches = operations.registry.launches.clone();
        let output = operations.registry.startup_output;
        let stage = {
            let spawned = spawned.clone();
            let operations = operations.clone();
            let streams = self.streams.clone();
            let clock = clock.clone();
            let settings = settings.clone();
            let name = name.clone();
            async move {
                let mut held = spawned.lock().await;
                let (process, mut launch) = Process::spawn(&managed, process_participant, &launches, output, &name).await?;
                let process = held.insert(process);
                let address = process.ready(&mut launch).await?;
                let mut activation = connect(&clock, &address, 1, None, Some(process.digest), operations.transfer_message_bytes().get())
                    .await
                    .map_err(|error| unanswered(&address, error))?;
                activation.entry_name = Some(name);
                activation.settings = crate::plugin::manager::settings::select(
                    activation.description.declarations.as_ref().expect("admitted description"),
                    values,
                )
                .map_err(|error| Error::Invalid(error.to_string()))?;
                activation.process_id = process.id();
                let admission = operations.prepare_grpc_admission(&activation, true)?;
                let publications = Publications::prepare(&streams, &activation, &settings, &operations, true, None).await?;
                Ok((address, activation, admission, publications))
            }
        };
        let result = tokio::select! {
            biased;
            () = participant.settling() => Err(Error::Invalid("Replace Plugin Startup: host is shutting down".into())),
            result = load::load(&operations, &name, LoadOptions::default(), stage) => result,
        };
        // An Ended Load Has Released the Process, Whether It Finished, Missed Its Deadline or Was Cancelled.
        let mut process = spawned.lock().await.take();
        match result {
            Ok((address, activation, admission, publications)) => {
                if let Some(process) = &process {
                    process.settled();
                }
                Ok(PreparedPlugin {
                participant,
                process,
                address,
                activation,
                    admission,
                    publications,
                })
            }
            Err(error) => match &mut process {
                Some(process) => Err(process.fail(clock, settings.stop_grace, "Stop Failed Replacement Startup", error).await?),
                None => Err(error),
            },
        }
    }
    pub(crate) async fn activate_grpc_entry(&self, prepared: PreparedPlugin, settings: PlacementSettings) -> Result<Attached, Error> {
        let PreparedPlugin {
            mut participant,
            mut process,
            address,
            mut activation,
            mut admission,
            publications,
        } = prepared;
        let operations = self.operations.execution();
        let clock = operations.clock().clone();
        // Initialize Before Publishing the Replacement's Operations or Sample Demands.
        let mut attempted = false;
        let initializing = async {
            attempted = true;
            admission.lifecycle(".initialize").await
        };
        let initialized = tokio::select! {
            biased;
            () = participant.settling() => Err(Error::Invalid("Initialize Replacement Plugin: host is shutting down".into())),
            result = initializing => result.map_err(|source| Error::Lifecycle {
                operation: format!("{}.initialize", activation.description.id),
                source,
            }),
        };
        let activated = initialized.and_then(|()| {
            admission.commit().map_err(|error| Error::Invalid(format!("Replace Plugin Operations: {error}")))?;
            Publications::activate(publications, &self.streams, &activation, &clock, &operations, &settings, true)
        });
        let publications = match activated {
            Ok(publications) => publications,
            Err(error) => {
                activation.admission = Some(Arc::new(admission));
                if attempted && let Err(failure) = activation.finalize().await {
                    tracing::warn!(plugin = %activation.description.id, %failure, "plugin finalization failed");
                }
                if activation.has_hooks() {
                    let mut client = activation.connection.client();
                    let request = InstanceStopRequest {
                        reason: "Replacement Initialization Failed".into(),
                    };
                    let _ = clock.timeout(settings.stop_grace, client.stop(request)).await;
                }
                if let Some(process) = &mut process {
                    process.terminate(&clock, settings.stop_grace, "Stop Failed Replacement Activation").await?;
                }
                return Err(error);
            }
        };
        admission.seal();
        activation.admission = Some(Arc::new(admission));
        activation.connection.ready.store(true, Ordering::Release);
        Ok(Attached::run(
            operations,
            self.streams.clone(),
            settings,
            participant,
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
        cancelled: oneshot::Receiver<()>, manager: super::manager::Manager,
    ) -> Result<Attached, Error> {
        Attached::start(
            self.operations.execution(),
            self.streams.clone(),
            placement,
            settings,
            Startup {
                participant: self.plugin_participant(),
                process_participant: Some(self.stop_participant()),
                entry: Some(EntrySettings { name, values, manager }),
                cancelled: Some(cancelled),
                options: Some(LoadOptions::default()),
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
        let participant = self.plugin_participant();
        Attached::start(
            self.operations.execution(),
            self.streams.clone(),
            placement,
            settings,
            Startup {
                participant,
                process_participant: Some(self.stop_participant()),
                entry: None,
                cancelled: None,
                options: Some(options),
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
        self.connected(describe(self.connection.client())).await
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
        operations: Manager, streams: StreamManager<A>, placement: Placement, settings: PlacementSettings, startup: Startup,
    ) -> Result<Self, Error> {
        let (cancel, dropped) = oneshot::channel();
        let task = tokio::spawn(Self::start_owned(operations, streams, placement, settings, startup, dropped));
        let result = task.await.map_err(|error| Error::Invalid(format!("Join Plugin Startup: {error}")))?;
        drop(cancel);
        result
    }

    async fn start_owned<A: Application>(
        operations: Manager, streams: StreamManager<A>, placement: Placement, settings: PlacementSettings, startup: Startup,
        dropped: oneshot::Receiver<()>,
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
        if participant.is_settling() || participant.is_stopping() {
            return Err(Error::Invalid("Start Plugin: the host is shutting down; attach the plugin after it restarts".into()));
        }
        let _activity = clock.activity("plugin startup");
        let plugin = match (&entry, &placement) {
            (Some(entry), _) => entry.name.clone(),
            (None, Placement::Managed(managed)) => managed.executable.display().to_string(),
            (None, Placement::Remote(address)) => address.clone(),
        };
        // Spawning Is Part of the Load. The Load Keeps the Process Here, So One Its Deadline Ends Returns It to This
        // Owner; a Child Forked After the Load Ended Is Killed and Reaped Where the Spawner Drops It.
        let spawned = Arc::new(sync::Mutex::new(None));
        let admitted = Arc::new(Mutex::new(None));
        let launches = operations.registry.launches.clone();
        let output = operations.registry.startup_output;
        let owner = plugin.clone();
        let setup = {
            let spawned = spawned.clone();
            let operations = operations.clone();
            let streams = streams.clone();
            let clock = clock.clone();
            let settings = settings.clone();
            let placement = placement.clone();
            let admitted = admitted.clone();
            async move {
                let mut process = spawned.lock().await;
                let address = match &placement {
                    Placement::Managed(managed) => {
                        let participant = process_participant.take().expect("managed process participant");
                        let (spawned, mut launch) = Process::spawn(managed, participant, &launches, output, &owner).await?;
                        process.insert(spawned).ready(&mut launch).await?
                    }
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
                .await
                .map_err(|error| if process.is_some() { unanswered(&address, error) } else { error })?;
                let values = entry.as_ref().and_then(|entry| {
                    activation.entry_name = Some(entry.name.clone());
                    entry.values.clone()
                });
                activation.settings = crate::plugin::manager::settings::select(
                    activation.description.declarations.as_ref().expect("admitted description"),
                    values,
                )
                .map_err(|error| Error::Invalid(error.to_string()))?;
                activation.process_id = process.as_ref().and_then(Process::id);
                let admission = operations.admit_grpc(&activation).inspect_err(|_| {
                    activation.connection.closed.send_replace(true);
                })?;
                activation.admission = Some(Arc::new(admission));
                if let Some(entry) = entry {
                    entry.manager.admitted(activation.clone()).await.map_err(|error| Error::Invalid(error.to_string()))?;
                }
                *admitted.lock().expect("startup activation lock is healthy") = Some(activation.clone());
                activation.initialize().await?;
                let publications = Publications::open(&streams, &activation, &clock, &settings, &operations).await?;
                activation.connection.ready.store(true, Ordering::Release);
                Ok::<_, Error>((address, activation, publications))
            }
        };
        let cancelled = async {
            match cancelled {
                Some(cancelled) => {
                    let _ = cancelled.await;
                }
                None => future::pending().await,
            }
        };
        let setup = async {
            match options {
                Some(options) => load::load(&operations, &plugin, options, setup).await,
                None => setup.await,
            }
        };
        let result = tokio::select! {
            biased;
            _ = dropped => Err(Error::Invalid("Cancel Plugin Startup: attachment dropped".into())),
            () = cancelled => Err(Error::Invalid("Cancel Plugin Startup: entry stopped".into())),
            () = participant.settling() => Err(Error::Invalid("Stop Plugin Startup: host is shutting down".into())),
            result = setup => result,
        };
        // An Ended Load Has Released the Process, Whether It Finished, Missed Its Deadline or Was Cancelled.
        let mut process = spawned.lock().await.take();
        let admitted = admitted.lock().expect("startup activation lock is healthy").take();
        let (address, activation, publications) = match result {
            Ok(connected) => connected,
            Err(error) => {
                if let Some(activation) = admitted {
                    if let Err(failure) = activation.finalize().await {
                        tracing::warn!(plugin = %activation.description.id, %failure, "plugin finalization failed");
                    }
                    if activation.has_hooks() {
                        let mut client = activation.connection.client();
                        let _ = clock
                            .timeout(settings.stop_grace, client.stop(InstanceStopRequest { reason: "Initialization Failed".into() }))
                            .await;
                    }
                    activation.connection.closed.send_replace(true);
                }
                return match &mut process {
                    Some(process) => Err(process.fail(&clock, settings.stop_grace, "Stop Plugin Startup", error).await?),
                    None => Err(error),
                };
            }
        };
        if let Some(process) = &process {
            process.settled();
        }
        let connected = ConnectedPlugin {
            process,
            address,
            activation,
            publications,
        };
        Ok(Self::run(operations, streams, settings, participant, connected))
    }

    fn run<A: Application>(
        operations: Manager, streams: StreamManager<A>, settings: PlacementSettings, participant: StopParticipant,
        connected: ConnectedPlugin,
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

/// A Managed Plugin Announced an Address Nothing Answers On.
fn unanswered(address: &str, error: Error) -> Error {
    match error {
        Error::Io(cause) => unreachable_address(address, &cause.to_string()),
        Error::Transport(cause) => unreachable_address(address, &cause),
        error => error,
    }
}

fn unreachable_address(address: &str, cause: &str) -> Error {
    Error::Invalid(format!(
        "Connect Managed Plugin: the plugin announced {address}, which did not answer ({cause}); start serving before announcing, then restart the plugin"
    ))
}

pub(crate) fn lifecycle_operation(plugin: &str, operation: &str) -> bool {
    matches!(operation.strip_prefix(plugin), Some(".initialize" | ".finalize"))
}

async fn describe(mut client: InstanceClient<Channel>) -> Result<InstanceGetResponse, Error> {
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
    clock: &Clock, address: &str, number: u64, previous_started_at: Option<Timestamp>, executable_digest: Option<[u8; 32]>,
    message_bytes: usize,
) -> Result<Activation, Error> {
    let connection = Arc::new(Connection::open(address, harmos_protocol::transfer::default_message_bytes().get()).await?);
    let description = describe(connection.client()).await?;
    if description.message_bytes < message_bytes as u64 {
        return Err(Error::Invalid(
            "Instance.Get: peer message_bytes cannot carry StorageSettings.transfer.chunk_bytes plus framing".into(),
        ));
    }
    validate_description(&description)?;
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
        publications: Arc::new(OnceLock::new()),
        finalized: Arc::new(AtomicBool::new(false)),
        clock: clock.clone(),
    })
}

fn validate_description(description: &InstanceGetResponse) -> Result<(), Error> {
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
    Ok(())
}

async fn disconnected(mut closed: watch::Receiver<bool>) {
    while !*closed.borrow_and_update() {
        if closed.changed().await.is_err() {
            break;
        }
    }
}

async fn stopping(stop: &mut watch::Receiver<Option<String>>, participant: &mut StopParticipant) -> Ending {
    loop {
        if let Some(reason) = stop.borrow_and_update().clone() {
            return Ending::Stop(reason);
        }
        tokio::select! {
            result = stop.changed() => if result.is_err() { return Ending::Stop("Attachment Dropped".into()); },
            () = participant.settling() => return Ending::HostShutdown,
        }
    }
}

async fn supervise<A: Application>(session: Session<A>, status: &watch::Sender<Status>) -> Outcome {
    let Session {
        operations,
        streams,
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
                    None => future::pending().await,
                }
            };
            tokio::select! {
                biased;
                ending = stopping(&mut stop, &mut participant) => break ending,
                result = exited => break Ending::Exit(result),
                () = disconnected(activation.connection.closed.subscribe()) => break Ending::Disconnect,
                // A Failed Unscoped Demand Ends Its Stream Only; the Attachment's Owner Reads Why.
                failure = publications.failure() => status.send_modify(|status| status.failure = Some(failure)),
            }
        };
        // Every Ending Settles Its Bindings Before the Connection or Process Is Released. Teardown
        // Parks Its Activity While It Waits on the Plugin, So Every Clock Kind Reaches Its Deadlines.
        let teardown = clock.activity("plugin teardown");
        activation.connection.ready.store(false, Ordering::Release);
        activation.admission.as_ref().expect("connected plugin admission").close();
        if matches!(ending, Ending::Exit(_) | Ending::Disconnect) {
            activation.connection.closed.send_replace(true);
        }
        let settling = publications.settle(&clock, clock.now() + settings.stop_grace, None, "Settle Plugin Publications");
        let settled = teardown.wait(settling).await;
        if let Err(error) = &settled {
            tracing::warn!(plugin = %activation.description.id, action = "Settle Plugin Publications", %error, "plugin publications did not settle");
        }
        let finalized = teardown.wait(activation.finalize()).await;
        if let Err(error) = &finalized {
            tracing::warn!(plugin = %activation.description.id, %error, "plugin finalization failed; continuing Stop");
            status.send_modify(|status| status.failure = Some(Arc::new(Error::Invalid(error.to_string()))));
        }
        let ending = match ending {
            Ending::HostShutdown => {
                participant.settled();
                teardown.wait(participant.stopping()).await;
                Ending::Stop("Host Shutting Down".into())
            }
            ending => ending,
        };
        // The Plugin Is Told to Stop Before It Is Killed: Stop and Its Exit Get a Fresh `stop_grace`.
        let deadline = clock.now() + settings.stop_grace;
        match ending {
            Ending::HostShutdown => unreachable!("host settlement precedes transport Stop"),
            Ending::Stop(reason) => {
                let mut client = activation.connection.client();
                tracing::debug!(plugin = %activation.description.id, "waiting for plugin Instance.Stop reply");
                let stopped = teardown
                    .wait(clock.timeout_at(deadline, client.stop(InstanceStopRequest { reason })))
                    .await
                    .map_err(|_| Error::Timeout("Stop"))
                    .and_then(|result| result.map(|_| ()).map_err(|error| Error::Transport(error.to_string())));
                match &stopped {
                    Err(error @ Error::Timeout(_)) => {
                        tracing::warn!(plugin = %activation.description.id, action = "Stop Plugin", outcome = "deadline", %error, "plugin Instance.Stop reply wait gave up at stop_grace; closing its connection");
                    }
                    outcome => {
                        tracing::debug!(plugin = %activation.description.id, outcome = if outcome.is_ok() { "stopped" } else { "failed" }, error = ?outcome.as_ref().err(), "plugin Instance.Stop reply wait completed");
                        if let Err(error) = outcome {
                            tracing::warn!(plugin = %activation.description.id, action = "Stop Plugin", %error, "plugin Stop failed; closing its connection");
                        }
                    }
                }
                activation.connection.closed.send_replace(true);
                let exit = match &mut process {
                    Some(process) => {
                        let process_id = process.id();
                        tracing::debug!(plugin = %activation.description.id, ?process_id, "waiting for managed plugin child exit and reap");
                        let exit = teardown.wait(process.stop(&clock, deadline, settings.stop_grace, "Stop Managed Plugin")).await;
                        match &exit {
                            Err(error @ Error::Deadline { .. }) => {
                                tracing::warn!(plugin = %activation.description.id, ?process_id, outcome = "deadline", %error, "managed plugin child exit and reap wait gave up at stop_grace");
                            }
                            Err(error @ Error::Reap(_)) => {
                                tracing::warn!(plugin = %activation.description.id, ?process_id, outcome = "failed", %error, "managed plugin child exit and reap wait gave up");
                            }
                            outcome => {
                                tracing::debug!(plugin = %activation.description.id, ?process_id, outcome = ?outcome, "managed plugin child exit and reap wait completed");
                            }
                        }
                        match exit {
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
                    exit => settled.and(finalized).and(stopped).and(exit).map_err(Arc::new),
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
            let admitted = Arc::new(Mutex::new(None));
            // One Plugin Load Bounds the Whole Attempt, and Stop Interrupts Every Step of It.
            let attempt = {
                let address = address.clone();
                let operations = operations.clone();
                let streams = streams.clone();
                let clock = clock.clone();
                let settings = settings.clone();
                let previous_activation = activation.clone();
                let admitted = admitted.clone();
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
                    match operations.admit_grpc(&next) {
                        Ok(admission) => next.admission = Some(Arc::new(admission)),
                        Err(error) => {
                            next.connection.closed.send_replace(true);
                            return Err(error);
                        }
                    }
                    *admitted.lock().expect("reconnect activation lock is healthy") = Some(next.clone());
                    if let Err(error) = next.initialize().await {
                        let _ = next.finalize().await;
                        return Err(error);
                    }
                    match Publications::open(&streams, &next, &clock, &settings, &operations).await {
                        Ok(opened) => {
                            next.connection.ready.store(true, Ordering::Release);
                            Ok((next, opened))
                        }
                        Err(error) => {
                            let _ = next.finalize().await;
                            next.connection.closed.send_replace(true);
                            next.admission.as_ref().unwrap().remove();
                            Err(error)
                        }
                    }
                }
            };
            let reconnected = tokio::select! {
                biased;
                _ = stopping(&mut stop, &mut participant) => None,
                result = load::load(&operations, activation.owner(), LoadOptions::default(), attempt) => Some(result),
            };
            let admitted = admitted.lock().expect("reconnect activation lock is healthy").take();
            if !matches!(reconnected, Some(Ok(_)))
                && let Some(next) = admitted
            {
                if let Err(error) = next.finalize().await {
                    tracing::warn!(plugin = %next.description.id, %error, "plugin reconnect finalization failed");
                }
                if next.has_hooks() {
                    let mut client = next.connection.client();
                    let _ =
                        clock.timeout(settings.stop_grace, client.stop(InstanceStopRequest { reason: "Reconnect Failed".into() })).await;
                }
                next.connection.closed.send_replace(true);
            }
            let Some(reconnected) = reconnected else { return Ok(()) };
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
