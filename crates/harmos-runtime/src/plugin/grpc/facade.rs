/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Plugin Activation Calls

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{ffi::OsString, mem, num::NonZeroUsize, sync::Arc, time::Duration};

// Third Party Imports
use futures_util::future::BoxFuture;
use tokio::sync::oneshot;

// Local Imports
use super::scoped::Generation;
use super::{
    Activation, Attached, Connection, Error, Placement, PlacementSettings, ScopedBinding, Startup, catalog, describe, managed::Process,
};
use crate::facade::{Call, CallOptions};
use crate::operations::{CallContext, ExecutionContext, HostContext, Manager, NativeOperation, OperationPolicy, ServingError};
use crate::runtime::StopParticipant;
use crate::streams::StreamManager;
use crate::{Application, MemorySize, RetryPolicy, clock::Clock};
use harmos_domain::btree_map_memory;
use harmos_operations::operations::UNBOUNDED;
use harmos_protocol::plugin::InstanceGetResponse;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Participants = Arc<dyn Fn() -> StopParticipant + Send + Sync>;

/* === Structs === */

#[derive(Clone)]
pub(crate) struct Host(Arc<dyn Hosting>);

struct Owner<A: Application> {
    streams: StreamManager<A>,
    participants: Participants,
}

/// One Activation Generation; Cloning Never Follows a Reconnection.
#[derive(Clone)]
pub struct Plugin {
    state: Arc<Loaded>,
}

struct Loaded {
    attached: Attached,
    activation: Activation,
    runtime: Arc<()>,
}

/// One Scope on One Activation; Settlement Closes Every Clone.
#[derive(Clone)]
pub struct Binding {
    state: Arc<Bound>,
}

struct Bound {
    plugin: Plugin,
    scoped: ScopedBinding,
    context: CallContext,
}

#[derive(Clone)]
pub struct LoadRequest {
    pub placement: Placement,
    pub settings: PlacementSettings,
}

#[derive(Clone)]
pub struct BindRequest {
    pub plugin: Plugin,
    pub scope: String,
    pub clock: Clock,
}

#[derive(Clone)]
pub struct StartSamplesRequest {
    pub binding: Binding,
    pub streams: Vec<String>,
}

#[derive(Clone)]
pub struct Samples {
    binding: Binding,
    generation: Generation,
}

#[derive(Clone)]
pub struct StopSamplesRequest {
    pub samples: Samples,
}

#[derive(Clone)]
pub struct InspectRequest {
    pub placement: Placement,
    pub settings: PlacementSettings,
    pub metadata_bytes: NonZeroUsize,
}

/// Frozen Metadata Only; Inspection Does Not Admit or Initialize the Candidate.
pub struct Inspection {
    pub metadata: InstanceGetResponse,
}

pub struct Load;
pub struct Bind;
pub struct StartSamples;
pub struct StopSamples;
pub struct Settle;
pub struct Inspect;

/* === Enums === */

#[derive(Clone)]
pub enum SettleRequest {
    Plugin(Plugin),
    Binding(Binding),
}

/* === Traits === */

trait Hosting: Send + Sync {
    fn load(&self, operations: Manager, request: LoadRequest) -> BoxFuture<'_, Result<Attached, Error>>;
    fn bind(&self, operations: &Manager, request: BindRequest) -> Result<ScopedBinding, Error>;
    fn participant(&self) -> StopParticipant;
}

/* === Implementations === */

impl Host {
    pub(crate) fn new<A: Application>(streams: StreamManager<A>, participants: Participants) -> Self {
        Self(Arc::new(Owner { streams, participants }))
    }
}

impl<A: Application> Hosting for Owner<A> {
    fn load(&self, operations: Manager, request: LoadRequest) -> BoxFuture<'_, Result<Attached, Error>> {
        Box::pin(Attached::start(
            operations,
            self.streams.clone(),
            request.placement,
            request.settings,
            Startup {
                participant: (self.participants)(),
                process_participant: Some((self.participants)()),
                entry: None,
                cancelled: None,
                options: None,
            },
        ))
    }

    fn bind(&self, operations: &Manager, request: BindRequest) -> Result<ScopedBinding, Error> {
        ScopedBinding::bind(self.streams.clone(), operations, &request.plugin.state.activation, request.scope, request.clock)
    }

    fn participant(&self) -> StopParticipant {
        (self.participants)()
    }
}

impl Plugin {
    fn check_runtime(&self, operations: &Manager) -> Result<(), ServingError> {
        if !Arc::ptr_eq(&self.state.runtime, &operations.resident_identity()) {
            return Err(failure("Use Plugin Handle: select the runtime that loaded this plugin"));
        }
        Ok(())
    }

    fn check(&self, operations: &Manager) -> Result<(), ServingError> {
        self.check_runtime(operations)?;
        let same_generation = {
            let current = self.state.attached.status.borrow();
            current.activation.as_ref().is_some_and(|current| Arc::ptr_eq(&current.connection, &self.state.activation.connection))
        };
        if !same_generation || !self.state.activation.is_connected() || self.state.attached.stop.borrow().is_some() {
            return Err(failure("Use Plugin Handle: the activation retired; settle this handle and load the plugin again"));
        }
        Ok(())
    }
}

impl Binding {
    /// Bind Native Calls to This Scope's Clock and Cancellation Unit.
    pub fn for_call(&self) -> Call {
        Call::new(
            self.state.plugin.state.activation.publications.get().expect("admitted activation").operations.clone(),
            &self.state.context,
        )
    }

    pub fn for_call_with(&self, options: CallOptions) -> Call {
        let mut context = self.state.context.clone();
        context.remove::<CallOptions>();
        context.insert(options).expect("one scoped call override");
        Call::new(self.state.plugin.state.activation.publications.get().expect("admitted activation").operations.clone(), &context)
    }
}

impl MemorySize for LoadRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(placement_memory(&self.placement))
    }
}

impl MemorySize for InspectRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(placement_memory(&self.placement))
    }
}

impl MemorySize for BindRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.scope.capacity())
    }
}

impl MemorySize for StartSamplesRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.streams.memory_size().saturating_sub(mem::size_of_val(&self.streams)))
    }
}

impl MemorySize for StopSamplesRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
    }
}

impl MemorySize for SettleRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
    }
}

impl NativeOperation for Load {
    type Request = LoadRequest;
    type Response = Plugin;
    type Endpoints = ();
    const ID: &'static str = "plugins.activate";
    const POLICY: OperationPolicy = super::Load::POLICY;
    async fn run(context: ExecutionContext, request: LoadRequest, _: &()) -> Result<Plugin, ServingError> {
        let operations = operations(&context)?;
        let host = host(&context)?;
        let attached = host.0.load(operations.clone(), request).await.map_err(native_failure)?;
        let activation = attached.activation().ok_or_else(|| failure("Load Plugin: activation ended; load the plugin again"))?;
        Ok(Plugin {
            state: Arc::new(Loaded {
                attached,
                activation,
                runtime: operations.resident_identity(),
            }),
        })
    }
}

impl NativeOperation for Bind {
    type Request = BindRequest;
    type Response = Binding;
    type Endpoints = ();
    const ID: &'static str = "plugins.bind";
    const POLICY: OperationPolicy = super::Load::POLICY;
    async fn run(context: ExecutionContext, request: BindRequest, _: &()) -> Result<Binding, ServingError> {
        let operations = operations(&context)?;
        request.plugin.check(&operations)?;
        let plugin = request.plugin.clone();
        let scoped = host(&context)?.0.bind(&operations, request).map_err(native_failure)?;
        let context = scoped.operations().call_context().clone();
        Ok(Binding {
            state: Arc::new(Bound { plugin, scoped, context }),
        })
    }
}

impl NativeOperation for StartSamples {
    type Request = StartSamplesRequest;
    type Response = Samples;
    type Endpoints = ();
    const ID: &'static str = "plugins.start_samples";
    const POLICY: OperationPolicy = super::Load::POLICY;
    async fn run(context: ExecutionContext, request: StartSamplesRequest, _: &()) -> Result<Samples, ServingError> {
        request.binding.state.plugin.check(&operations(&context)?)?;
        let generation = request.binding.state.scoped.start_generation(request.streams).await.map_err(native_failure)?;
        Ok(Samples { binding: request.binding, generation })
    }
}

impl NativeOperation for StopSamples {
    type Request = StopSamplesRequest;
    type Response = ();
    type Endpoints = ();
    const ID: &'static str = "plugins.stop_samples";
    const POLICY: OperationPolicy = Settle::POLICY;
    async fn run(context: ExecutionContext, request: StopSamplesRequest, _: &()) -> Result<(), ServingError> {
        request.samples.binding.state.plugin.check(&operations(&context)?)?;
        request.samples.binding.state.scoped.stop_generation(request.samples.generation).await.map_err(native_failure)
    }
}

impl NativeOperation for Settle {
    type Request = SettleRequest;
    type Response = ();
    type Endpoints = ();
    const ID: &'static str = "plugins.settle";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: UNBOUNDED,
        attempt_timeout: UNBOUNDED,
        cancellation_timeout: Duration::ZERO,
        retries: RetryPolicy::Never,
    };
    async fn run(context: ExecutionContext, request: SettleRequest, _: &()) -> Result<(), ServingError> {
        let operations = operations(&context)?;
        match request {
            SettleRequest::Plugin(plugin) => {
                plugin.check_runtime(&operations)?;
                plugin.state.attached.stop("Settle Plugin").await.map_err(|error| ServingError::Native(error))
            }
            SettleRequest::Binding(binding) => {
                binding.state.plugin.check_runtime(&operations)?;
                binding.state.scoped.settle().await.map_err(|error| ServingError::Native(error))
            }
        }
    }
}

impl NativeOperation for Inspect {
    type Request = InspectRequest;
    type Response = Inspection;
    type Endpoints = ();
    const ID: &'static str = "plugins.inspect";
    const POLICY: OperationPolicy = super::Load::POLICY;
    async fn run(context: ExecutionContext, request: InspectRequest, _: &()) -> Result<Inspection, ServingError> {
        if request.settings.stop_grace.is_zero() {
            return Err(failure("Inspect Plugin: supply a positive PlacementSettings.stop_grace"));
        }
        let operations = operations(&context)?;
        let host = host(&context)?;
        let participant = host.0.participant();
        let process_participant = host.0.participant();
        let (retained, dropped) = oneshot::channel();
        let message_bytes = operations.transfer_message_bytes().get();
        let mut task =
            tokio::spawn(inspect_owned(operations.clone(), request, message_bytes, participant, process_participant, dropped));
        let result = tokio::select! {
            biased;
            () = context.cancellation.cancelled() => {
                drop(retained);
                task.await
            }
            result = &mut task => result,
        };
        let result = result.map_err(|error| failure(&format!("Inspect Plugin: {error}; inspect the candidate again")))?;
        result.map_err(|error| native_failure(Error::Inspection(Box::new(error))))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn operations(context: &ExecutionContext) -> Result<Manager, ServingError> {
    let host = context.resource::<HostContext>().map_err(|_| ServingError::Catalog)?;
    Ok(host.borrow().operations.clone())
}

fn host(context: &ExecutionContext) -> Result<Host, ServingError> {
    context.call.get::<Host>().cloned().ok_or_else(|| failure("Call Plugin Operation: start the full runtime before calling plugins"))
}

fn failure(message: &str) -> ServingError {
    native_failure(Error::Invalid(message.into()))
}
fn native_failure(error: Error) -> ServingError {
    ServingError::Native(Arc::new(error))
}

fn placement_memory(placement: &Placement) -> usize {
    match placement {
        Placement::Remote(address) => address.capacity(),
        Placement::Managed(managed) => {
            let mut bytes = managed.executable.capacity();
            bytes = bytes.saturating_add(managed.arguments.capacity().saturating_mul(mem::size_of::<OsString>()));
            bytes = bytes.saturating_add(
                btree_map_memory::<OsString, OsString>(managed.environment.len()).saturating_sub(mem::size_of_val(&managed.environment)),
            );
            for argument in &managed.arguments {
                bytes = bytes.saturating_add(argument.capacity());
            }
            for (name, value) in &managed.environment {
                bytes = bytes.saturating_add(name.capacity()).saturating_add(value.capacity());
            }
            bytes
        }
    }
}

async fn inspect_owned(
    operations: Manager, request: InspectRequest, message_bytes: usize, mut participant: StopParticipant, process_participant: StopParticipant,
    dropped: oneshot::Receiver<()>,
) -> Result<Inspection, Error> {
    let clock = operations.clock().clone();
    let mut process = None;
    let mut connection = None;
    let work = async {
        let address = match &request.placement {
            Placement::Managed(managed) => {
                let owner = managed.executable.display().to_string();
                let (spawned, mut launch) = Process::spawn(
                    managed,
                    process_participant,
                    &operations.registry.launches,
                    operations.registry.startup_output,
                    &owner,
                )
                .await?;
                process.insert(spawned).ready(&mut launch).await?
            }
            Placement::Remote(address) => address.clone(),
        };
        let connected = connection.insert(Connection::open(&address, request.metadata_bytes.get()).await?);
        let client = connected.client().max_encoding_message_size(message_bytes);
        let metadata = describe(client).await.map_err(|error| {
            Error::Invalid(format!("Inspect Plugin Metadata: {error}; supply metadata within metadata_bytes and a reachable endpoint"))
        })?;
        super::validate_description(&metadata)?;
        catalog::verify(metadata.declarations.as_ref().expect("validated declarations"))?;
        Ok(Inspection { metadata })
    };
    let result = tokio::select! {
        biased;
        () = participant.stopping() => Err(Error::Invalid("Inspect Plugin: host is stopping; inspect after restart".into())),
        _ = dropped => Err(Error::Invalid("Inspect Plugin: call ended; inspect the candidate again".into())),
        result = work => result,
    };
    if let Some(connection) = connection {
        connection.closed.send_replace(true);
    }
    match (&mut process, result) {
        (Some(process), Err(error)) => Err(process.fail(&clock, request.settings.stop_grace, "Settle Plugin Inspection", error).await?),
        (Some(process), Ok(inspection)) => {
            process.terminate(&clock, request.settings.stop_grace, "Settle Plugin Inspection").await?;
            Ok(inspection)
        }
        (None, result) => result,
    }
}

/* === Tests === */
