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
use harmos_runtime::operations::{OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, Visibility, WireTable};
use harmos_runtime::sidecar::{Frame, Notice, SidecarError, Supervised};
use harmos_runtime::{
    Runtime, services::Service, services::ServiceExit, services::ServiceStatus, state::FactSource, state::Position, state::SystemFact,
};
use harmos_sidecar::{Attach, Refusal, Supervisor};
use status::service_status;
use support::{Bank, Ledger, catalog, streams};

/* === Constants === */

static GAUGE_DEACTIVATED: AtomicBool = AtomicBool::new(false);
const GAUGE_POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(1),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "meter.amplify",
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    },
    response = Amplified,
    visibility = Visibility::Shared,
)]
#[derive(Clone, PartialEq)]
struct Amplify {
    value: i64,
}

#[harmos_runtime::message]
#[derive(Clone, PartialEq)]
struct Amplified {
    value: i64,
}

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "meter.sample",
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    },
    response = (),
    visibility = Visibility::Host,
)]
#[derive(Clone, PartialEq, Eq)]
struct Sample {
    value: u64,
}

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "meter.emit",
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    },
    response = (),
    visibility = Visibility::Shared,
)]
#[derive(Clone, PartialEq)]
struct Emit {
    value: u64,
}

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "meter.publish-pulse",
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    },
    response = (),
    visibility = Visibility::Shared,
)]
#[derive(Clone, PartialEq)]
struct PublishPulse {
    sample: u64,
}

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "meter.collapse",
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    },
    response = (),
    visibility = Visibility::Shared,
)]
#[derive(Clone, PartialEq)]
struct Collapse {}

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "meter.finish",
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    },
    response = (),
    visibility = Visibility::Shared,
)]
#[derive(Clone, PartialEq)]
struct Finish {
    reason: String,
}

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "meter.budgeted",
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    },
    response = BudgetedReply,
    visibility = Visibility::Shared,
)]
#[derive(Clone, PartialEq)]
struct BudgetedRoute {
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

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "gauge.get-measure",
    version = 1,
    policy = GAUGE_POLICY,
    response = Measured,
)]
#[derive(Clone, PartialEq)]
pub(crate) struct GetMeasure {
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

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "gauge.stop-sampling",
    version = 1,
    policy = GAUGE_POLICY,
    response = StopSamplingResponse,
)]
#[derive(Clone, PartialEq)]
pub(crate) struct StopSampling {}

#[harmos_runtime::message]
#[derive(Clone, PartialEq)]
pub(crate) struct StopSamplingResponse {}

#[harmos_runtime::message]
#[harmos_runtime::operation(id = "gauge.get-factor", version = 1, policy = GAUGE_POLICY, response = Measured)]
#[derive(Clone, PartialEq)]
pub(crate) struct GetFactor {}

#[harmos_runtime::message]
#[harmos_runtime::operation(id = "gauge.run-echo", version = 1, policy = GAUGE_POLICY, response = String)]
struct RunEcho {
    text: String,
}

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "gauge.run-uppercase",
    version = 1,
    policy = GAUGE_POLICY,
    response = String,
)]
struct RunUppercase {
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

struct Measuring {
    resources: Arc<GaugeResources>,
}

#[derive(Debug, thiserror::Error)]
#[error("gRPC transport error: {0}")]
struct ProbeTransport(#[source] std::io::Error);

struct Echoing {
    _resources: Arc<GaugeResources>,
}

struct Uppercasing {
    _resources: Arc<GaugeResources>,
}

struct StoppingSampling {
    resources: Arc<GaugeResources>,
}

struct ReadingFactor {
    resources: Arc<GaugeResources>,
}

struct Amplifying {
    _resources: Arc<MeterHandler>,
}

struct Emitting {
    _resources: Arc<MeterHandler>,
}

struct PublishingPulse {
    _resources: Arc<MeterHandler>,
}

struct Collapsing {
    _resources: Arc<MeterHandler>,
}

struct Finishing {
    _resources: Arc<MeterHandler>,
}

struct BudgetedCall {
    _resources: Arc<MeterHandler>,
}

struct Sampling {
    resources: Arc<SampleProjection>,
}

/* === Enums === */

#[harmos_runtime::lanes]
enum HostLanes {
    #[default]
    Open,
}

#[harmos_runtime::catalog]
enum GaugeOperations {
    Measure(GetMeasure),
    Echo(RunEcho),
    Uppercase(RunUppercase),
    Stop(StopSampling),
    Factor(GetFactor),
    Private(visibility::ScalePrivately),
    Public(visibility::ScalePublicly),
}

#[harmos_runtime::catalog]
enum MeterOperations {
    Amplify(Amplify),
    Emit(Emit),
    Publish(PublishPulse),
    Collapse(Collapse),
    Finish(Finish),
    Budgeted(BudgetedRoute),
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

impl harmos_runtime::operations::lifecycle::Policy for Joinable {
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
    fn bind_operations(&self, _: &mut WireTable<Self::Context>) -> Result<(), harmos_runtime::operations::ServingError> {
        Ok(())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl harmos_runtime::operations::lifecycle::Policy for Gauge {
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
        Ok(Self { resources: Arc::new(GaugeResources { factor: config.factor, stopped: AtomicUsize::new(0) }) })
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    fn resources(&self) -> Arc<GaugeResources> {
        self.resources.clone()
    }
    fn bind_operations(&self, table: &mut WireTable<Supervisor>) -> Result<(), harmos_runtime::operations::ServingError> {
        table.bind_operation::<Measuring>(self.resources())?;
        table.bind_operation::<Echoing>(self.resources())?;
        table.bind_operation::<Uppercasing>(self.resources())?;
        table.bind_operation::<StoppingSampling>(self.resources())?;
        table.bind_operation::<ReadingFactor>(self.resources())?;
        table.bind_operation::<visibility::ScalingPrivately>(self.resources())?;
        table.bind_operation::<visibility::ScalingPublicly>(self.resources())
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

impl harmos_runtime::operations::Operation for Measuring {
    type Declaration = GetMeasure;
    type Resources = GaugeResources;
    type Error = MeasureError;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(resources: Arc<GaugeResources>, _: Self::Context<'ctx>) -> Result<Self, MeasureError> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut GetMeasure, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Measured>, MeasureError> {
        let result: ::core::result::Result<Measured, Self::Error> = async move {
            let _ = &request;
            if request.value == i64::MIN {
                let reset = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "connection reset");
                return Err(MeasureError::Probe(ProbeTransport(reset)));
            }
            if request.value < 0 {
                let refusal = Refusal::of("this gauge measures no negative value");
                return Err(MeasureError::Refused(refusal));
            }
            Ok(Measured { value: request.value * self.resources.factor })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::operations::Operation for Echoing {
    type Declaration = RunEcho;
    type Resources = GaugeResources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(resources: Arc<GaugeResources>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { _resources: resources })
    }
    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut RunEcho, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<String>, Refusal> {
        let result: ::core::result::Result<String, Self::Error> = async move { Ok(request.text.clone()) }.await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::operations::Operation for Uppercasing {
    type Declaration = RunUppercase;
    type Resources = GaugeResources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(resources: Arc<GaugeResources>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { _resources: resources })
    }
    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut RunUppercase, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<String>, Refusal> {
        let result: ::core::result::Result<String, Self::Error> = async move { Ok(request.text.to_uppercase()) }.await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::operations::Operation for StoppingSampling {
    type Declaration = StopSampling;
    type Resources = GaugeResources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(resources: Arc<GaugeResources>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut StopSampling, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<StopSamplingResponse>, Refusal> {
        let result: ::core::result::Result<StopSamplingResponse, Self::Error> = async move {
            let _ = &request;
            self.resources.stopped.fetch_add(1, Ordering::SeqCst);
            Ok(StopSamplingResponse {})
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::operations::Operation for ReadingFactor {
    type Declaration = GetFactor;
    type Resources = GaugeResources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(resources: Arc<GaugeResources>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut GetFactor, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Measured>, Refusal> {
        let result: ::core::result::Result<Measured, Self::Error> = async move {
            let _ = &request;
            Ok(Measured { value: self.resources.factor })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::operations::Operation for Amplifying {
    type Declaration = Amplify;
    type Resources = MeterHandler;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(resources: Arc<MeterHandler>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { _resources: resources })
    }
    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut Amplify, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Amplified>, Refusal> {
        let result: ::core::result::Result<Amplified, Self::Error> = async move {
            if request.value < 0 {
                return Err(Refusal::of("this meter measures no negative value"));
            }
            Ok(Amplified { value: request.value * 2 })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::operations::Operation for Emitting {
    type Declaration = Emit;
    type Resources = MeterHandler;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(resources: Arc<MeterHandler>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { _resources: resources })
    }
    async fn run<'ctx>(
        &mut self, host: Self::Context<'ctx>, request: &mut Emit, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let result: ::core::result::Result<(), Self::Error> =
            async move { host.publisher::<Meter>().publish_on::<MeterPulse>(&support::pulse(request.value)).await.map_err(Refusal::from) }
                .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::operations::Operation for PublishingPulse {
    type Declaration = PublishPulse;
    type Resources = MeterHandler;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(resources: Arc<MeterHandler>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { _resources: resources })
    }
    async fn run<'ctx>(
        &mut self, host: Self::Context<'ctx>, request: &mut PublishPulse, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let result: ::core::result::Result<(), Self::Error> =
            async move { host.publisher::<Meter>().publish_on::<MeterPulse>(&support::pulse(request.sample)).await.map_err(Refusal::from) }
                .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::operations::Operation for Collapsing {
    type Declaration = Collapse;
    type Resources = MeterHandler;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(resources: Arc<MeterHandler>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { _resources: resources })
    }
    async fn run<'ctx>(
        &mut self, host: Self::Context<'ctx>, request: &mut Collapse, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            let _ = (&request, &host);
            host.abandon();
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::operations::Operation for Finishing {
    type Declaration = Finish;
    type Resources = MeterHandler;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(resources: Arc<MeterHandler>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { _resources: resources })
    }
    async fn run<'ctx>(
        &mut self, host: Self::Context<'ctx>, request: &mut Finish, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            let _ = (&request, &host);
            host.exiting(request.reason.clone());
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::operations::Operation for BudgetedCall {
    type Declaration = BudgetedRoute;
    type Resources = MeterHandler;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(resources: Arc<MeterHandler>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { _resources: resources })
    }
    async fn run<'ctx>(
        &mut self, host: Self::Context<'ctx>, request: &mut BudgetedRoute, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<BudgetedReply>, Refusal> {
        let result: ::core::result::Result<BudgetedReply, Self::Error> = async move {
            let _ = (&request, &host);
            if request.refusal_bytes > 0 {
                return Err(Refusal::new(-42, "x".repeat(request.refusal_bytes as usize)));
            }
            Ok(BudgetedReply { payload: vec![0; request.reply_bytes as usize] })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::operations::Operation for Sampling {
    type Declaration = Sample;
    type Resources = SampleProjection;
    type Error = harmos_runtime::Error;
    type Endpoints = ();
    type Context<'a> = harmos_runtime::operations::Context<'a>;
    async fn initialize<'ctx>(resources: Arc<SampleProjection>, _: Self::Context<'ctx>) -> harmos_runtime::Result<Self> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut Sample, _endpoints: &mut Self::Endpoints,
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<()>> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.resources.streams.publish(support::pulse(request.value));
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::operations::lifecycle::Policy for Meter {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_runtime::plugin(id = "meter", provides = MeterOperations, requires(harmos_runtime::operations::streams::Publish),  streams(MeterPulse))]
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
    fn bind_operations(&self, table: &mut WireTable<Supervisor>) -> Result<(), harmos_runtime::operations::ServingError> {
        table.bind_operation::<Amplifying>(self.resources())?;
        table.bind_operation::<Emitting>(self.resources())?;
        table.bind_operation::<PublishingPulse>(self.resources())?;
        table.bind_operation::<Collapsing>(self.resources())?;
        table.bind_operation::<Finishing>(self.resources())?;
        table.bind_operation::<BudgetedCall>(self.resources())
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
    fn operations(&self) -> harmos_domain::Result<Vec<harmos_runtime::operations::Definition>> {
        self.source.operations()
    }
    fn operation_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_runtime::operations::Contract>>> {
        self.source.operation_contracts()
    }
    fn required_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_domain::inventory::contracts::Required>>> {
        self.source.required_contracts()
    }
    fn sdk_lifecycle(&self) -> harmos_domain::Result<Option<[harmos_runtime::operations::lifecycle::Descriptor; 2]>> {
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
    MeterSource { source: harmos_sidecar::PluginNative::new(NonZeroUsize::new(16).unwrap()).unwrap(), attachments, publications: None }
}

fn assembly() -> harmos_runtime::Builder<Bank> {
    Runtime::<Bank>::builder(Ledger::default(), ()).register(catalog()).register(streams()).register(HostLanes::catalog())
}

/* === Tests === */
