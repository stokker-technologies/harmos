/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "composition.rs"]
mod composition;

#[path = "../support/mod.rs"]
mod support;

#[path = "../sidecar/admission.rs"]
mod admission;

#[path = "../sidecar/consumption.rs"]
mod consumption;

#[cfg(feature = "grpc")]
#[path = "../sidecar/grants.rs"]
mod grants;

#[path = "../sidecar/graph.rs"]
mod graph;

#[path = "../sidecar/listeners.rs"]
mod listeners;

#[path = "../sidecar/observer_demand.rs"]
mod observer_demand;

#[path = "../sidecar/observer_hosts.rs"]
mod observer_hosts;
pub(crate) mod operations;

#[path = "../sidecar/process.rs"]
mod process;

#[path = "../sidecar/publications.rs"]
mod publications;

#[path = "../sidecar/routing.rs"]
mod routing;

#[path = "../support/witness.rs"]
mod witness;

#[path = "../sidecar/status.rs"]
mod status;

#[path = "../sidecar/visibility.rs"]
mod visibility;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Third Party Imports

// Local Imports
use harmos_runtime::Message;
use harmos_runtime::operations::{OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, WireTable};
use harmos_runtime::sidecar::{Frame, Notice, SidecarError, Supervised};
use harmos_runtime::{
    Runtime, services::ServiceContext, services::ServiceExit, services::ServiceStatus, state::FactSource, state::Position,
    state::SystemFact,
};
use harmos_sidecar::{Attach, Refusal, Supervisor};
use status::service_status;
use support::{Bank, Ledger, catalog, streams};

/* === Constants === */

static GAUGE_DEACTIVATED: AtomicBool = AtomicBool::new(false);
const GAUGE_POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(1),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_runtime::operation(resources = [MeterHandler], id = "device.amplify",
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    },
    response = Amplified,
    visibility = Shared,
)]
#[derive(Clone, PartialEq)]
struct AmplifyDeclaration {
    value: i64,
}

#[harmos_runtime::message]
#[derive(Clone, PartialEq)]
struct Amplified {
    value: i64,
}

#[harmos_runtime::operation(resources = [SampleProjection], id = "device.sample",
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    },
    response = (),
    visibility = Host,
)]
#[derive(Clone, PartialEq, Eq)]
struct SampleDeclaration {
    value: u64,
}

#[harmos_runtime::operation(resources = [MeterHandler, Supervisor], id = "device.emit",
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    },
    response = (),
    visibility = Shared,
)]
#[derive(Clone, PartialEq)]
struct EmitDeclaration {
    value: u64,
}

#[harmos_runtime::operation(resources = [MeterHandler, Supervisor], id = "publish.pulse",
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    },
    response = (),
    visibility = Shared,
)]
#[derive(Clone, PartialEq)]
struct PublishPulseDeclaration {
    sample: u64,
}

#[harmos_runtime::operation(resources = [MeterHandler, harmos_sidecar::Supervisor], id = "device.collapse",
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    },
    response = (),
    visibility = Shared,
)]
#[derive(Clone, PartialEq)]
struct CollapseDeclaration {}

#[harmos_runtime::operation(resources = [MeterHandler, harmos_sidecar::Supervisor], id = "device.finish",
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    },
    response = (),
    visibility = Shared,
)]
#[derive(Clone, PartialEq)]
struct FinishDeclaration {
    reason: String,
}

#[harmos_runtime::operation(resources = [MeterHandler], id = "device.budgeted",
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    },
    response = BudgetedReply,
    visibility = Shared,
)]
#[derive(Clone, PartialEq)]
struct BudgetedRouteDeclaration {
    payload: Vec<u8>,

    reply_bytes: u32,

    refusal_bytes: u32,
}

#[harmos_runtime::message]
#[derive(Clone, PartialEq)]
struct BudgetedReply {
    payload: Vec<u8>,
}

#[harmos_runtime::stream(description = "The whole meter sidecar, served in this process.\nCounting attachments is how a restart is observed from outside: the factory runs once per attempt, exactly as a process would be started once per attempt.", id = "meter.pulse", window = 8, row = support::Pulse)]
struct MeterPulse;

struct SampleProjection {
    streams: harmos_runtime::streams::StreamManager<Bank>,
}

struct Joinable {
    resources: Arc<()>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
struct Calibration {
    factor: i64,
}

#[harmos_runtime::operation(resources = [GaugeResources], id = "get.measure",
    policy = GAUGE_POLICY,
    response = Measured,
)]
#[derive(Clone, PartialEq)]
struct GetMeasureDeclaration {
    value: i64,
}

#[harmos_runtime::message]
#[derive(Clone, PartialEq)]
pub(crate) struct Measured {
    value: i64,
}

struct Gauge {
    resources: Arc<GaugeResources>,
}

struct GaugeResources {
    factor: i64,
    stopped: AtomicUsize,
}

#[harmos_runtime::operation(resources = [GaugeResources], id = "stop.sampling",
    policy = GAUGE_POLICY,
    response = StopSamplingResponse,
)]
#[derive(Clone, PartialEq)]
struct StopSamplingDeclaration {}

#[harmos_runtime::message]
#[derive(Clone, PartialEq)]
pub(crate) struct StopSamplingResponse {}

#[harmos_runtime::operation(resources = [GaugeResources], id = "get.factor",  policy = GAUGE_POLICY, response = Measured)]
#[derive(Clone, PartialEq)]
struct GetFactorDeclaration {}

#[harmos_runtime::operation(resources = [GaugeResources], id = "run.echo",  policy = GAUGE_POLICY, response = String)]
struct RunEchoDeclaration {
    text: String,
}

#[harmos_runtime::operation(resources = [GaugeResources], id = "run.uppercase",
    policy = GAUGE_POLICY,
    response = String,
)]
struct RunUppercaseDeclaration {
    text: String,
}

struct MeterHandler;

struct Meter {
    resources: Arc<MeterHandler>,
}

#[derive(Clone)]
struct MeterSource {
    source: harmos_sidecar::PluginNative<Meter>,
    attachments: Arc<AtomicUsize>,
    publications: Option<Vec<harmos_sidecar::StreamDeclaration>>,
}

#[derive(Debug, thiserror::Error)]
#[error("gRPC transport error: {0}")]
struct ProbeTransport(#[source] std::io::Error);

/* === Enums === */

use support::Lanes as HostLanes;

#[harmos_runtime::catalog]
enum GaugeOperations {
    Measure(GetMeasureDeclaration),
    Echo(RunEchoDeclaration),
    Uppercase(RunUppercaseDeclaration),
    Stop(StopSamplingDeclaration),
    Factor(GetFactorDeclaration),
    Private(visibility::ScalePrivatelyDeclaration),
    Public(visibility::ScalePubliclyDeclaration),
}

#[harmos_runtime::catalog]
enum MeterOperations {
    AmplifyDeclaration(AmplifyDeclaration),
    EmitDeclaration(EmitDeclaration),
    Publish(PublishPulseDeclaration),
    CollapseDeclaration(CollapseDeclaration),
    FinishDeclaration(FinishDeclaration),
    Budgeted(BudgetedRouteDeclaration),
}

// A handler error shaped like a gRPC client failure: every layer repeats the
// one beneath it, the way tonic and hyper render a reset connection.
#[derive(Debug, thiserror::Error)]
enum MeasureError {
    #[error("measure refused")]
    Refused(#[source] Refusal),

    #[error("operation `gauge.probe.read` failed: {0}")]
    Probe(#[source] ProbeTransport),
}

/* === Traits === */

/* === Implementations === */

impl harmos_operations::lifecycle::Policy for Joinable {
    const INITIALIZATION_POLICY: OperationPolicy = GAUGE_POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = GAUGE_POLICY;
}

#[harmos_runtime::plugin(id = "joined", provides = (), requires = ())]
impl harmos_runtime::Plugin for Joinable {
    type Config = ();
    type Context = Supervisor;
    type Error = Refusal;
    type Resources = ();
    fn prepare(_: ()) -> Result<Self, Self::Error> {
        Ok(Self { resources: Arc::new(()) })
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    fn resources(&self) -> Arc<()> {
        self.resources.clone()
    }
    fn bind_operations(&self, _: &mut WireTable) -> Result<(), harmos_runtime::operations::ServingError> {
        Ok(())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl harmos_operations::lifecycle::Policy for Gauge {
    const INITIALIZATION_POLICY: OperationPolicy = GAUGE_POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = GAUGE_POLICY;
}

#[harmos_runtime::plugin(description = "Measures Scaled Gauge Values With One SDK Resource Owner.", id = "gauge", provides = GaugeOperations, requires = (),
    restart(credit = 2, sustained_health_ms = 250, backoff_base_ms = 10, backoff_cap_ms = 5_000))]
impl harmos_runtime::Plugin for Gauge {
    type Config = Calibration;
    type Context = Supervisor;
    type Error = Refusal;
    type Resources = GaugeResources;
    fn prepare(config: Calibration) -> Result<Self, Self::Error> {
        Ok(Self {
            resources: Arc::new(GaugeResources {
                factor: config.factor,
                stopped: AtomicUsize::new(0),
            }),
        })
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    fn resources(&self) -> Arc<GaugeResources> {
        self.resources.clone()
    }
    fn bind_operations(&self, table: &mut WireTable) -> Result<(), harmos_runtime::operations::ServingError> {
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<GetMeasureDeclaration>(),
        )?;
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<RunEchoDeclaration>(),
        )?;
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<RunUppercaseDeclaration>(),
        )?;
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<StopSamplingDeclaration>(),
        )?;
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<GetFactorDeclaration>(),
        )?;
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<visibility::ScalePrivatelyDeclaration>(),
        )?;
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<visibility::ScalePubliclyDeclaration>(),
        )
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        tokio::task::yield_now().await;
        GAUGE_DEACTIVATED.store(true, Ordering::SeqCst);
        Ok(())
    }
}

impl harmos_runtime::Operation for GetMeasureDeclaration {
    type Error = MeasureError;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<Measured>, MeasureError> {
        let resources = ctx.resource::<GaugeResources>();
        let result: ::core::result::Result<Measured, Self::Error> = async move {
            let _ = &self;
            if self.value == i64::MIN {
                let reset = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "connection reset");
                return Err(MeasureError::Probe(ProbeTransport(reset)));
            }
            if self.value < 0 {
                let refusal = Refusal::of("this gauge measures no negative value");
                return Err(MeasureError::Refused(refusal));
            }
            Ok(Measured { value: self.value * resources.factor })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for RunEchoDeclaration {
    type Error = Refusal;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<String>, Refusal> {
        let _resources = ctx.resource::<GaugeResources>();
        let result: ::core::result::Result<String, Self::Error> = async move { Ok(self.text.clone()) }.await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for RunUppercaseDeclaration {
    type Error = Refusal;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<String>, Refusal> {
        let _resources = ctx.resource::<GaugeResources>();
        let result: ::core::result::Result<String, Self::Error> = async move { Ok(self.text.to_uppercase()) }.await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for StopSamplingDeclaration {
    type Error = Refusal;

    async fn run(
        self, ctx: harmos_runtime::Context<'_, Self>,
    ) -> Result<harmos_runtime::operations::Response<StopSamplingResponse>, Refusal> {
        let resources = ctx.resource::<GaugeResources>();
        let result: ::core::result::Result<StopSamplingResponse, Self::Error> = async move {
            let _ = &self;
            resources.stopped.fetch_add(1, Ordering::SeqCst);
            Ok(StopSamplingResponse {})
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for GetFactorDeclaration {
    type Error = Refusal;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<Measured>, Refusal> {
        let resources = ctx.resource::<GaugeResources>();
        let result: ::core::result::Result<Measured, Self::Error> = async move {
            let _ = &self;
            Ok(Measured { value: resources.factor })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for AmplifyDeclaration {
    type Error = Refusal;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<Amplified>, Refusal> {
        let _resources = ctx.resource::<MeterHandler>();
        let result: ::core::result::Result<Amplified, Self::Error> = async move {
            if self.value < 0 {
                return Err(Refusal::of("this meter measures no negative value"));
            }
            Ok(Amplified { value: self.value * 2 })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for EmitDeclaration {
    type Error = Refusal;

    async fn run(self, host: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let _resources = host.resource::<MeterHandler>();
        let host = host.resource::<Supervisor>();
        let result: ::core::result::Result<(), Self::Error> =
            async move { host.publisher::<Meter>().publish_on::<MeterPulse>(&support::pulse(self.value)).await.map_err(Refusal::from) }
                .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for PublishPulseDeclaration {
    type Error = Refusal;

    async fn run(self, host: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let _resources = host.resource::<MeterHandler>();
        let host = host.resource::<Supervisor>();
        let result: ::core::result::Result<(), Self::Error> =
            async move { host.publisher::<Meter>().publish_on::<MeterPulse>(&support::pulse(self.sample)).await.map_err(Refusal::from) }
                .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for CollapseDeclaration {
    type Error = Refusal;

    async fn run(self, host: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let _resources = host.resource::<MeterHandler>();
        let result: ::core::result::Result<(), Self::Error> = async move {
            let _ = (&self, &host);
            host.resource::<harmos_sidecar::Supervisor>().abandon();
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for FinishDeclaration {
    type Error = Refusal;

    async fn run(self, host: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let _resources = host.resource::<MeterHandler>();
        let result: ::core::result::Result<(), Self::Error> = async move {
            let _ = (&self, &host);
            host.resource::<harmos_sidecar::Supervisor>().exiting(self.reason.clone());
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for BudgetedRouteDeclaration {
    type Error = Refusal;

    async fn run(self, host: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<BudgetedReply>, Refusal> {
        let _resources = host.resource::<MeterHandler>();
        let result: ::core::result::Result<BudgetedReply, Self::Error> = async move {
            let _ = (&self, &host);
            if self.refusal_bytes > 0 {
                return Err(Refusal::new(-42, "x".repeat(self.refusal_bytes as usize)));
            }
            Ok(BudgetedReply {
                payload: vec![0; self.reply_bytes as usize],
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for SampleDeclaration {
    type Error = harmos_runtime::Error;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::operations::Response<()>> {
        let resources = ctx.resource::<SampleProjection>();
        let result: ::core::result::Result<(), Self::Error> = async move {
            resources.streams.publish(support::pulse(self.value));
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_operations::lifecycle::Policy for Meter {
    const INITIALIZATION_POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_runtime::plugin(id = "meter", provides = MeterOperations, requires(harmos_operations::streams::Publish),  streams(MeterPulse))]
impl harmos_runtime::Plugin for Meter {
    type Config = ();
    type Context = Supervisor;
    type Resources = MeterHandler;
    type Error = Refusal;
    fn prepare(_: ()) -> Result<Self, Self::Error> {
        Ok(Self { resources: Arc::new(MeterHandler) })
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Self::Error> {
        Ok(())
    }
    fn resources(&self) -> Arc<MeterHandler> {
        self.resources.clone()
    }
    fn bind_operations(&self, table: &mut WireTable) -> Result<(), harmos_runtime::operations::ServingError> {
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<AmplifyDeclaration>(),
        )?;
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<EmitDeclaration>(),
        )?;
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<PublishPulseDeclaration>(),
        )?;
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<CollapseDeclaration>(),
        )?;
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<FinishDeclaration>(),
        )?;
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<BudgetedRouteDeclaration>(),
        )
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Attach for MeterSource {
    type Error = std::io::Error;
    fn owner(&self) -> &str {
        self.source.owner()
    }
    fn operations(&self) -> harmos_domain::Result<Vec<harmos_runtime::operations::OperationDefinition>> {
        self.source.operations()
    }
    fn operation_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_runtime::operations::Contract>>> {
        self.source.operation_contracts()
    }
    fn required_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_domain::inventory::contracts::Required>>> {
        self.source.required_contracts()
    }
    fn sdk_lifecycle(&self) -> harmos_domain::Result<Option<[harmos_operations::lifecycle::Descriptor; 2]>> {
        self.source.sdk_lifecycle()
    }
    fn publications(&self) -> Vec<harmos_sidecar::StreamDeclaration> {
        self.publications.clone().unwrap_or_else(|| self.source.publications())
    }
    fn observers(&self) -> Vec<harmos_sidecar::inventory::observers::Declaration> {
        self.source.observers()
    }
    fn observe_contract(&self) -> harmos_domain::Result<Option<harmos_runtime::operations::Contract>> {
        self.source.observe_contract()
    }
    fn listener_declarations(&self) -> harmos_domain::Result<Option<Vec<harmos_domain::listeners::Definition>>> {
        self.source.listener_declarations()
    }
    async fn attach(&self) -> Result<harmos_sidecar::Attached, Self::Error> {
        self.attachments.fetch_add(1, Ordering::SeqCst);
        self.source.attach().await
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn meter(attachments: Arc<AtomicUsize>) -> MeterSource {
    MeterSource {
        source: harmos_sidecar::PluginNative::new(NonZeroUsize::new(16).unwrap()).unwrap(),
        attachments,
        publications: None,
    }
}

fn publication_registration<A, S, H>(resident: Arc<Supervised<A, S, H>>) -> harmos_runtime::RegistrationBatch
where
    A: harmos_runtime::Application,
    S: harmos_runtime::sidecar::Attach,
    H: Fn(&ServiceContext<A>, Notice) + Send + Sync + 'static,
{
    let mut dependencies = harmos_runtime::Dependencies::new();
    dependencies.insert(resident).unwrap();
    harmos_runtime::RegistrationBatch::new(dependencies).operation::<harmos_operations::streams::Publish>()
}

fn assembly() -> harmos_runtime::Builder<Bank> {
    Runtime::<Bank>::builder(Ledger::default(), ()).register(catalog()).register(streams()).register(HostLanes::catalog())
}

/* === Tests === */
