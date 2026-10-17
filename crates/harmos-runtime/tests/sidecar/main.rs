/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../support/mod.rs"]
mod support;

mod admission;
mod consumption;
#[cfg(feature = "grpc")]
mod grants;
mod graph;
mod listeners;
mod observer_demand;
mod observer_hosts;
pub(crate) mod operations;
mod process;
mod publications;
mod routing;
#[path = "../support/witness.rs"]
mod witness;

mod visibility {
    use super::*;
    #[harmos_runtime::message]
    #[derive(Clone, PartialEq)]
    pub struct Scale {
        pub value: i64,
    }
    #[harmos_runtime::message]
    #[harmos_runtime::operation(
        id = "gauge.scale-privately",
        version = 1,
        policy = GAUGE_POLICY,
        response = Scale,
    )]
    #[derive(Clone, PartialEq)]
    pub struct ScalePrivately {
        pub value: i64,
    }
    #[harmos_runtime::message]
    #[harmos_runtime::operation(
        id = "gauge.scale-publicly",
        version = 1,
        policy = GAUGE_POLICY,
        response = Scale,
    )]
    #[derive(Clone, PartialEq)]
    pub struct ScalePublicly {}
    pub(super) struct ScalingPrivately {
        resources: Arc<GaugeResources>,
    }

    impl harmos_runtime::operations::Operation for ScalingPrivately {
        type Declaration = ScalePrivately;
        type Resources = GaugeResources;
        type Error = Refusal;
        type Endpoints = ();
        type Context<'a> = &'a Supervisor;
        async fn initialize<'ctx>(resources: Arc<GaugeResources>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
            Ok(Self { resources })
        }
        async fn run<'ctx>(
            &mut self, _: Self::Context<'ctx>, request: &mut ScalePrivately, _endpoints: &mut Self::Endpoints,
        ) -> Result<harmos_runtime::operations::Response<Scale>, Refusal> {
            let result: ::core::result::Result<Scale, Self::Error> =
                async move { Ok(Scale { value: request.value * self.resources.factor }) }.await;
            result.map(::core::convert::Into::into)
        }
    }
    pub(super) struct ScalingPublicly {
        resources: Arc<GaugeResources>,
    }

    impl harmos_runtime::operations::Operation for ScalingPublicly {
        type Declaration = ScalePublicly;
        type Resources = GaugeResources;
        type Error = Refusal;
        type Endpoints = ();
        type Context<'a> = &'a Supervisor;
        async fn initialize<'ctx>(resources: Arc<GaugeResources>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
            Ok(Self { resources })
        }
        async fn run<'ctx>(
            &mut self, _: Self::Context<'ctx>, _request: &mut ScalePublicly, _endpoints: &mut Self::Endpoints,
        ) -> Result<harmos_runtime::operations::Response<Scale>, Refusal> {
            let result: ::core::result::Result<Scale, Self::Error> = async move { Ok(Scale { value: self.resources.factor }) }.await;
            result.map(::core::convert::Into::into)
        }
    }
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
// Third Party Imports
use sha2::{Digest, Sha256};
use support::{Bank, Ledger, catalog, streams};
// Local Imports
use crate::support;
use harmos_runtime::Message;
use harmos_runtime::operations::{
    OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, Visibility, WireTable,
};
use harmos_runtime::sidecar::inventory::{Declaration, Facility};
use harmos_runtime::sidecar::{Executable, Frame, Notice, SidecarError, Supervised};
use harmos_runtime::{
    Runtime, services::Service, services::ServiceExit, services::ServiceStatus, state::FactSource,
    state::Position, state::SystemFact,
};
use harmos_sidecar::{Attach, Refusal, Supervisor};

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
    id = "meter.ask",
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_millis(25),
        attempt_timeout: Duration::from_millis(25),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    },
    response = (),
    description = "An operation that never answers, so the host's deadline ends the call.",
)]
#[derive(Clone, PartialEq)]
struct Ask {}

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
#[harmos_runtime::operation(
    id = "meter.echo",
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_millis(80),
        attempt_timeout: Duration::from_millis(80),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    },
    response = String,
    description = "Answers with what it was handed, publishing first when asked to.",
)]
struct MeterEcho {
    text: String,
}

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
        Ok(Self {
            resources: Arc::new(()),
        })
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    fn resources(&self) -> Arc<()> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        _: &mut WireTable<Self::Context>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
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
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
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

struct Measuring {
    resources: Arc<GaugeResources>,
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

#[derive(Debug, thiserror::Error)]
#[error("gRPC transport error: {0}")]
struct ProbeTransport(#[source] std::io::Error);

impl harmos_runtime::operations::Operation for Measuring {
    type Declaration = GetMeasure;
    type Resources = GaugeResources;
    type Error = MeasureError;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<GaugeResources>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, MeasureError> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut GetMeasure,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Measured>, MeasureError> {
        let result: ::core::result::Result<Measured, Self::Error> = async move {
            let _ = &request;
            if request.value == i64::MIN {
                let reset =
                    std::io::Error::new(std::io::ErrorKind::ConnectionReset, "connection reset");
                return Err(MeasureError::Probe(ProbeTransport(reset)));
            }
            if request.value < 0 {
                let refusal = Refusal::of("this gauge measures no negative value");
                return Err(MeasureError::Refused(refusal));
            }
            Ok(Measured {
                value: request.value * self.resources.factor,
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

struct Echoing {
    _resources: Arc<GaugeResources>,
}

impl harmos_runtime::operations::Operation for Echoing {
    type Declaration = RunEcho;
    type Resources = GaugeResources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<GaugeResources>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut RunEcho,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<String>, Refusal> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            Ok(request.text.clone())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

struct Uppercasing {
    _resources: Arc<GaugeResources>,
}

impl harmos_runtime::operations::Operation for Uppercasing {
    type Declaration = RunUppercase;
    type Resources = GaugeResources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<GaugeResources>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut RunUppercase,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<String>, Refusal> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            Ok(request.text.to_uppercase())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

struct StoppingSampling {
    resources: Arc<GaugeResources>,
}

impl harmos_runtime::operations::Operation for StoppingSampling {
    type Declaration = StopSampling;
    type Resources = GaugeResources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<GaugeResources>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut StopSampling,
        _endpoints: &mut Self::Endpoints,
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

struct ReadingFactor {
    resources: Arc<GaugeResources>,
}

impl harmos_runtime::operations::Operation for ReadingFactor {
    type Declaration = GetFactor;
    type Resources = GaugeResources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<GaugeResources>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut GetFactor,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Measured>, Refusal> {
        let result: ::core::result::Result<Measured, Self::Error> = async move {
            let _ = &request;
            Ok(Measured {
                value: self.resources.factor,
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

struct Amplifying {
    _resources: Arc<MeterHandler>,
}

impl harmos_runtime::operations::Operation for Amplifying {
    type Declaration = Amplify;
    type Resources = MeterHandler;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<MeterHandler>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Amplify,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Amplified>, Refusal> {
        let result: ::core::result::Result<Amplified, Self::Error> = async move {
            if request.value < 0 {
                return Err(Refusal::of("this meter measures no negative value"));
            }
            Ok(Amplified {
                value: request.value * 2,
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

struct Emitting {
    _resources: Arc<MeterHandler>,
}

impl harmos_runtime::operations::Operation for Emitting {
    type Declaration = Emit;
    type Resources = MeterHandler;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<MeterHandler>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        host: Self::Context<'ctx>,
        request: &mut Emit,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            host.publisher::<Meter>()
                .publish_on::<MeterPulse>(&support::pulse(request.value))
                .await
                .map_err(Refusal::from)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

struct PublishingPulse {
    _resources: Arc<MeterHandler>,
}

impl harmos_runtime::operations::Operation for PublishingPulse {
    type Declaration = PublishPulse;
    type Resources = MeterHandler;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<MeterHandler>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        host: Self::Context<'ctx>,
        request: &mut PublishPulse,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            host.publisher::<Meter>()
                .publish_on::<MeterPulse>(&support::pulse(request.sample))
                .await
                .map_err(Refusal::from)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

struct Collapsing {
    _resources: Arc<MeterHandler>,
}

impl harmos_runtime::operations::Operation for Collapsing {
    type Declaration = Collapse;
    type Resources = MeterHandler;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<MeterHandler>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        host: Self::Context<'ctx>,
        request: &mut Collapse,
        _endpoints: &mut Self::Endpoints,
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

struct Finishing {
    _resources: Arc<MeterHandler>,
}

impl harmos_runtime::operations::Operation for Finishing {
    type Declaration = Finish;
    type Resources = MeterHandler;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<MeterHandler>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        host: Self::Context<'ctx>,
        request: &mut Finish,
        _endpoints: &mut Self::Endpoints,
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

struct BudgetedCall {
    _resources: Arc<MeterHandler>,
}

impl harmos_runtime::operations::Operation for BudgetedCall {
    type Declaration = BudgetedRoute;
    type Resources = MeterHandler;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<MeterHandler>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        host: Self::Context<'ctx>,
        request: &mut BudgetedRoute,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<BudgetedReply>, Refusal> {
        let result: ::core::result::Result<BudgetedReply, Self::Error> = async move {
            let _ = (&request, &host);
            if request.refusal_bytes > 0 {
                return Err(Refusal::new(
                    -42,
                    "x".repeat(request.refusal_bytes as usize),
                ));
            }
            Ok(BudgetedReply {
                payload: vec![0; request.reply_bytes as usize],
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

struct Sampling {
    resources: Arc<SampleProjection>,
}

impl harmos_runtime::operations::Operation for Sampling {
    type Declaration = Sample;
    type Resources = SampleProjection;
    type Error = harmos_runtime::Error;
    type Endpoints = ();
    type Context<'a> = harmos_runtime::operations::Context<'a>;
    async fn initialize<'ctx>(
        resources: Arc<SampleProjection>,
        _: Self::Context<'ctx>,
    ) -> harmos_runtime::Result<Self> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Sample,
        _endpoints: &mut Self::Endpoints,
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<()>> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.resources
                .streams
                .publish(support::pulse(request.value));
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
        Ok(Self {
            resources: Arc::new(MeterHandler),
        })
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Self::Error> {
        Ok(())
    }
    fn resources(&self) -> Arc<MeterHandler> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
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
    fn operation_contracts(
        &self,
    ) -> harmos_domain::Result<Option<Vec<harmos_runtime::operations::Contract>>> {
        self.source.operation_contracts()
    }
    fn required_contracts(
        &self,
    ) -> harmos_domain::Result<Option<Vec<harmos_domain::inventory::contracts::Required>>> {
        self.source.required_contracts()
    }
    fn sdk_lifecycle(
        &self,
    ) -> harmos_domain::Result<Option<[harmos_runtime::operations::lifecycle::Descriptor; 2]>> {
        self.source.sdk_lifecycle()
    }
    fn publications(&self) -> Vec<harmos_sidecar::StreamDeclaration> {
        self.publications
            .clone()
            .unwrap_or_else(|| self.source.publications())
    }
    fn observers(&self) -> Vec<harmos_sidecar::inventory::observers::Declaration> {
        self.source.observers()
    }
    fn observe_contract(&self) -> harmos_domain::Result<Option<harmos_runtime::operations::Contract>> {
        self.source.observe_contract()
    }
    fn listener_declarations(
        &self,
    ) -> harmos_domain::Result<Option<Vec<harmos_domain::listeners::Definition>>> {
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

fn assert_transport_refusal(error: harmos_runtime::Error, code: i64, detail: &str) {
    let harmos_runtime::Error::OperationFailed { source, .. } = error else { panic!("transport refusal preserves its operation cause: {error}") };
    assert!(
        matches!(source.downcast_ref::<harmos_runtime::operations::NativeError>(),
        Some(harmos_runtime::operations::NativeError::Transport(SidecarError::Refused { code: actual, failure }))
            if *actual == code && failure.cause().contains(detail)),
        "{source}"
    );
}

// Await The Resident Status Change The Story Names; The Lane Deadline Ends A Hang.
async fn service_status(
    mut services: tokio::sync::watch::Receiver<BTreeMap<harmos_runtime::operations::resident::Key, harmos_runtime::operations::resident::Snapshot>>,
    name: &str, wanted: impl Fn(&ServiceStatus) -> bool,
) -> ServiceStatus {
    let key = harmos_runtime::operations::resident::Key::root(name);
    let snapshot = services
        .wait_for(|items| items.get(&key).and_then(|item| item.health.as_ref()).is_some_and(&wanted))
        .await
        .expect("the runtime keeps the status sender open");
    snapshot.get(&key).and_then(|item| item.health.clone()).expect("the awaited status is present")
}

async fn work_facts(runtime: &Runtime<Bank>) -> Vec<SystemFact> {
    runtime
        .state
        .query::<SystemFact>(Position::ORIGIN)
        .await
        .filter_map(Result::ok)
        .map(|entry| entry.value)
        .filter(|fact| matches!(fact.source, FactSource::Work(_)))
        .collect()
}

#[tokio::test]
async fn declared_publication_enters() {
    let mut fixture = operations::Managed::attach(
        meter(Arc::new(AtomicUsize::new(0))),
        serde_json::to_vec(&()).unwrap(),
        operations::Lanes::NativeOpen,
        NonZeroUsize::new(16).unwrap(),
        vec![],
        |resident| resident.ingest::<MeterPulse>(),
    )
    .await;
    fixture.publication().unwrap();
    fixture
        .runtime
        .operations
        .register_operation::<Sampling>(
            "meter-sample",
            operations::Lanes::NativeOpen,
            Arc::new(SampleProjection { streams: fixture.runtime.streams.clone() }),
        )
        .unwrap();
    fixture.initialize().await.unwrap();
    publications::ready(fixture.ready().clone()).await;
    let runtime = &fixture.runtime;
    fixture.call(PublishPulse { sample: 42 }).await.expect("the sidecar publishes the requested sample");

    let samples: Vec<u64> = runtime
        .streams
        .query::<support::Pulse>(..)
        .await
        .expect("the declared stream answers from its window")
        .map(|row| row.value.sample)
        .collect();
    assert_eq!(samples, [42]);

    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn crash_spends_credit() {
    let attachments = Arc::new(AtomicUsize::new(0));
    let source = meter(Arc::clone(&attachments));
    let definition = operations::inspected(&source, serde_json::to_vec(&()).unwrap()).definitions().next().unwrap().clone();
    let calls = Supervised::new(source, (|_: &Service<Bank>, _: Notice| {}) as fn(&Service<Bank>, Notice));
    let policy = harmos_runtime::operations::OperationPolicy {
        restart: harmos_runtime::operations::RestartPolicy::OnTransient {
            credit: 2,
            sustained_health: Duration::from_secs(30),
            backoff: harmos_runtime::operations::Backoff::Exponential {
                initial: Duration::from_millis(1),
                maximum: Duration::from_millis(2),
                jitter: harmos_runtime::operations::Jitter::Deterministic,
            },
        },
        ..harmos_runtime::services::POLICY
    };
    let runtime = assembly()
        .register(operations::Lanes::catalog())
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_wire_operation::<harmos_runtime::sidecar::Publish<_, _, _>>(
            "meter-publication-core",
            operations::Lanes::NativeOpen,
            Arc::new(calls.clone()),
        )
        .unwrap();
    runtime
        .operations
        .register_operation::<Sampling>(
            "meter-sample",
            operations::Lanes::NativeOpen,
            Arc::new(SampleProjection { streams: runtime.streams.clone() }),
        )
        .unwrap();
    let resident = calls.clone();
    runtime
        .operations
        .register_service(harmos_runtime::services::ServiceDefinition { id: "meter".into(), policy }, move |_: Arc<()>| {
            harmos_runtime::sidecar::Managed::new(
                resident.clone(),
                definition.clone(),
                operations::Lanes::NativeOpen,
                NonZeroUsize::new(16).unwrap(),
                NonZeroUsize::new(4096).unwrap(),
            )
            .unwrap()
        })
        .unwrap();
    let lease = runtime.operations.submit_resident("meter").await.unwrap();

    let statuses = runtime.operations.watch_residents();
    service_status(statuses.clone(), "meter", |status| *status == ServiceStatus::Running { credit: 2 }).await;
    let original = calls.operations().unwrap();
    let generation = original.scope().unwrap().generation();
    let outcome = runtime.operations.call_encoded(Collapse::ID, Collapse {}.encode_to_vec()).await;
    let disconnected = match &outcome {
        Err(harmos_runtime::Error::Closing { domain: "operation" | "operation plugin" }) => true,
        Err(harmos_runtime::Error::OperationFailed { operation, source }) if operation == Collapse::ID => {
            matches!(
                source.downcast_ref::<harmos_runtime::operations::NativeError>(),
                Some(harmos_runtime::operations::NativeError::Transport(SidecarError::Unavailable))
            )
        }
        _ => false,
    };
    assert!(disconnected, "the abandoned generation cannot answer: {outcome:?}");

    service_status(statuses.clone(), "meter", |status| *status == ServiceStatus::Running { credit: 1 }).await;
    assert_eq!(attachments.load(Ordering::SeqCst), 2, "a fresh sidecar was attached for the restart");
    assert!(work_facts(&runtime).await.is_empty(), "an environmental restart is silent");

    assert!(
        matches!(original.scope().unwrap().ready().await, Err(harmos_runtime::Error::Closing { domain: "plugin scope" })),
        "the retired generation cannot admit calls"
    );
    assert_ne!(calls.operations().unwrap().scope().unwrap().generation(), generation);
    let amplified = runtime
        .operations
        .call_encoded(Amplify::ID, Amplify { value: 5 }.encode_to_vec())
        .await
        .expect("the restarted sidecar serves the same routes");
    assert_eq!(Amplified::decode(&amplified).unwrap(), Amplified { value: 10 });
    lease.release().await.unwrap();
    runtime.operations.unregister_resident("meter").await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn oversized_exchange_survives() {
    let mut fixture = operations::Managed::attach(
        meter(Arc::new(AtomicUsize::new(0))),
        serde_json::to_vec(&()).unwrap(),
        operations::Lanes::NativeOpen,
        NonZeroUsize::new(16).unwrap(),
        vec![],
        |resident| resident.ingest::<MeterPulse>(),
    )
    .await;
    fixture.publication().unwrap();
    fixture
        .runtime
        .operations
        .register_operation::<Sampling>(
            "meter-sample",
            operations::Lanes::NativeOpen,
            Arc::new(SampleProjection { streams: fixture.runtime.streams.clone() }),
        )
        .unwrap();
    fixture.initialize().await.unwrap();
    publications::ready(fixture.ready().clone()).await;
    let request = BudgetedRoute { payload: vec![0; Frame::LIMIT], reply_bytes: 0, refusal_bytes: 0 };
    assert!(!Frame::request_fits::<BudgetedRoute>(&request));
    let oversized = fixture.call(request).await.unwrap_err();
    assert_transport_refusal(oversized, Frame::OVERSIZED, "request");

    for request in [
        BudgetedRoute { payload: Vec::new(), reply_bytes: Frame::LIMIT as u32, refusal_bytes: 0 },
        BudgetedRoute { payload: Vec::new(), reply_bytes: 0, refusal_bytes: Frame::LIMIT as u32 },
    ] {
        let refused = fixture.call(request).await.unwrap_err();
        assert_transport_refusal(refused, Frame::OVERSIZED, "reply");
    }

    assert_eq!(
        fixture.call(Amplify { value: 5 }).await.unwrap(),
        Amplified { value: 10 },
        "every oversize refusal leaves the same exchange available to a later call"
    );

    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn announced_exit_reason() {
    let attachments = Arc::new(AtomicUsize::new(0));
    let source = meter(Arc::clone(&attachments));
    let definition = operations::definition(&source, serde_json::to_vec(&()).unwrap());
    let resident = Supervised::new(source, (|_: &Service<Bank>, _: Notice| {}) as fn(&Service<Bank>, Notice));
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_wire_operation::<harmos_runtime::sidecar::Publish<_, _, _>>("meter-publication-core", HostLanes::Open, Arc::new(resident.clone()))
        .unwrap();
    runtime
        .operations
        .register_service(harmos_runtime::services::ServiceDefinition { id: "meter".into(), policy: harmos_runtime::services::POLICY }, move |_: Arc<()>| {
            harmos_runtime::sidecar::Managed::new(
                resident.clone(),
                definition.clone(),
                HostLanes::Open,
                NonZeroUsize::new(16).unwrap(),
                NonZeroUsize::new(4096).unwrap(),
            )
            .unwrap()
        })
        .unwrap();
    let _lease = runtime.operations.submit_resident("meter").await.unwrap();
    service_status(runtime.operations.watch_residents(), "meter", |status| matches!(status, ServiceStatus::Running { .. })).await;
    let outcome =
        runtime.operations.call_encoded(Finish::ID, Finish { reason: "the recording is complete".to_owned() }.encode_to_vec()).await;
    assert!(
        matches!(outcome, Err(harmos_runtime::Error::Closing { domain: "operation" | "operation plugin" })),
        "a reply from the closed generation is revoked: {outcome:?}"
    );

    let status =
        service_status(runtime.operations.watch_residents(), "meter", |status| matches!(status, ServiceStatus::Inactive { .. })).await;

    assert!(
        matches!(
            status,
            ServiceStatus::Inactive {
                cause: ServiceExit::Application(ref reason),
                ref history,
            } if reason == "the recording is complete" && history.is_empty()
        ),
        "an announced exit is the sidecar's own decision, not an outage: {status:?}"
    );
    assert_eq!(attachments.load(Ordering::SeqCst), 1, "a sidecar that meant to leave is not started again");

    let facts = work_facts(&runtime).await;
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].name, "deactivated");
    assert!(matches!(runtime.stop().await, Err(harmos_runtime::Error::TaskFailed { domain: "service", .. })));
}

#[tokio::test]
async fn retired_control_refused() {
    let attached = meter(Arc::new(AtomicUsize::new(0))).attach().await.expect("the in-process sidecar attaches");
    let (write, read, control, _diagnostics, end) = attached.into_parts();
    let harmos_sidecar::transport::Connection { sender: outgoing, receiver: mut incoming, mut driver } =
        harmos_sidecar::transport::Connection::new((read, write), control.expect("the in-process attachment carries a control channel"));
    let driving = tokio::spawn(async move { driver.run().await });

    // Activation Answers On The Same Exchange
    outgoing
        .send(harmos_sidecar::Frame::Call { id: 1, method: harmos_sidecar::Frame::REGISTER.to_owned(), params: Vec::new() })
        .await
        .expect("the activation is sent");
    let answer = received(incoming.receive().await.expect("the exchange remains valid"));
    assert!(matches!(answer, Some(harmos_sidecar::Frame::Answer { id: 1, .. })));

    // The Retired Control Method Is Refused As Unknown
    outgoing
        .send(harmos_sidecar::Frame::Call { id: 2, method: "harmos.initialize".to_owned(), params: b"{}".to_vec() })
        .await
        .expect("the retired control request is sent");
    let refusal = received(incoming.receive().await.expect("the transport remains valid"));
    assert!(matches!(
        refusal,
        Some(harmos_sidecar::Frame::Refusal { id: 2, refusal })
            if refusal.code() == harmos_sidecar::UNKNOWN
    ));

    outgoing.close();
    let _ = driving.await.unwrap();
    drop(outgoing);
    if let Some(mut end) = end {
        end.settle().await.expect("the attachment ending joins");
    }
}

fn received(received: Option<harmos_sidecar::transport::Received>) -> Option<harmos_sidecar::Frame> {
    received.map(|received| {
        if let Some(consumption) = received.consumption {
            consumption.consumed().unwrap();
        }
        received.frame
    })
}

#[tokio::test]
async fn public_private_handlers() {
    let capacity = NonZeroUsize::new(16).unwrap();
    let source = harmos_sidecar::PluginNative::<Gauge>::new(capacity).unwrap();
    let mut fixture = operations::Managed::attach(
        source,
        serde_json::to_vec(&Calibration { factor: 5 }).unwrap(),
        operations::Lanes::NativeOpen,
        capacity,
        vec![],
        std::convert::identity,
    )
    .await;
    fixture.initialize().await.unwrap();
    publications::ready(fixture.ready().clone()).await;

    assert_eq!(
        fixture.call(visibility::ScalePrivately { value: 4 }).await.unwrap(),
        visibility::Scale { value: 20 },
        "a private method's route is still named and dispatched from outside          the module it was written in"
    );
    assert_eq!(
        fixture.call(visibility::ScalePublicly {}).await.unwrap(),
        visibility::Scale { value: 5 },
        "and a `pub` method's carrier reaches outside the module with it"
    );

    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn generated_entry_attaches() {
    GAUGE_DEACTIVATED.store(false, Ordering::SeqCst);

    // Bind The Original SDK Owner Before Authored Initialization
    let capacity = NonZeroUsize::new(16).unwrap();
    let source = harmos_sidecar::PluginNative::<Gauge>::new(capacity).unwrap();
    let mut fixture = operations::Managed::attach(
        source,
        serde_json::to_vec(&Calibration { factor: 3 }).unwrap(),
        operations::Lanes::NativeOpen,
        capacity,
        vec![],
        std::convert::identity,
    )
    .await;
    fixture.initialize().await.unwrap();
    publications::ready(fixture.ready().clone()).await;

    assert_eq!(
        fixture.call(GetMeasure { value: 7 }).await.unwrap(),
        Measured { value: 21 },
        "the route answers under the configuration the entry was activated with"
    );
    let error = fixture.call(GetMeasure { value: -1 }).await.unwrap_err();
    let failure = error.failure().expect("handler refusal preserves its structured failure");
    assert_eq!(failure.code(), harmos_runtime::operations::FailureKind::Permanent);
    assert_eq!(failure.cause(), "this gauge measures no negative value");
    assert_eq!(failure.context().operation(), Some(GetMeasure::ID));
    assert_eq!(
        error.to_string(),
        format!("operation `{}` failed", GetMeasure::ID),
        "the operation frame prints its own line, never the cause"
    );

    // A Nested Transport Failure Arrives As One Cause And One Operation Frame
    let error = fixture.call(GetMeasure { value: i64::MIN }).await.unwrap_err();
    let failure = error.failure().expect("the handler failure crosses as a structured value");
    assert_eq!(failure.cause(), "connection reset");
    assert_eq!(failure.context().operation(), Some(GetMeasure::ID));
    assert_eq!(failure.context().step(), None);
    let diagnostics = failure.diagnostics().expect("origin diagnostics");
    assert_eq!(diagnostics.origin(), harmos_runtime::failure::Origin::Plugin);
    assert_eq!(diagnostics.chain().len(), 3, "the full origin chain stays in diagnostics");
    let rendered = std::iter::successors(Some(&error as &(dyn std::error::Error + 'static)), |error| error.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ");
    assert_eq!(
        rendered,
        format!("operation `{}` failed: connection reset", GetMeasure::ID),
        "no carrier repeats the cause or adds a frame"
    );

    assert_eq!(fixture.call(RunEcho { text: "hello".to_owned() }).await.unwrap(), "hello".to_owned());
    assert_eq!(fixture.call(RunUppercase { text: "hello".to_owned() }).await.unwrap(), "HELLO".to_owned());

    assert_eq!(
        fixture.call(StopSampling {}).await.unwrap(),
        StopSamplingResponse {},
        "a route carrying nothing either way still names one message each way"
    );
    assert_eq!(
        fixture.call(GetFactor {}).await.unwrap(),
        Measured { value: 3 },
        "a reply written through the crate's own alias dispatches like any other"
    );

    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
    assert!(GAUGE_DEACTIVATED.load(Ordering::SeqCst), "the local attachment runs the generated entry's awaited deactivation");
}

#[tokio::test]
async fn guest_row_refused() {
    let noticed = Arc::new(AtomicUsize::new(0));
    let counter = noticed.clone();
    let source = native_process();
    let definition = process_definition(&source);
    let sidecar = Supervised::new(source, move |_context: &Service<Bank>, notice: Notice| {
        if matches!(notice, Notice::Published { .. }) {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    })
    .grace(Duration::from_millis(20));
    let ready = sidecar.ready();
    let calls = sidecar.clone();
    let (runtime, _lease) = process_runtime(
        sidecar,
        definition,
        harmos_runtime::operations::OperationPolicy { restart: harmos_runtime::operations::RestartPolicy::Never, ..harmos_runtime::services::POLICY },
    )
    .await;
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut ready = ready;
        while !*ready.borrow_and_update() {
            ready.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    let refused = runtime
        .operations
        .call_encoded(MeterEcho::ID, Message::encode_to_vec(&MeterEcho { text: "publish-foreign".to_owned() }))
        .await
        .unwrap_err();
    let harmos_runtime::Error::OperationFailed { source, .. } = refused else { panic!("the real host returns its publication refusal: {refused}") };
    assert!(
        matches!(source.downcast_ref::<harmos_runtime::operations::NativeError>(),
        Some(harmos_runtime::operations::NativeError::Remote(failure))
            if failure.code() == harmos_runtime::operations::FailureKind::Permanent
                && failure.cause().contains("stream or schema differs from the frozen generation inventory")),
        "{source}"
    );
    assert!(
        matches!(
            runtime.operations.resident(&harmos_runtime::operations::resident::Key::root("meter")).and_then(|instance| instance.health),
            Some(ServiceStatus::Running { .. })
        ),
        "a refused publication keeps the original carrier usable"
    );
    assert!(runtime.streams.publication("foreign", 1).is_none());
    assert_eq!(noticed.load(Ordering::SeqCst), 0, "ungranted bytes never reach the notice fallback");
    drop(calls);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn ingestion_authority_bounded() {
    for grants in [
        Vec::new(),
        vec![harmos_sidecar::StreamDeclaration {
            metadata: <MeterPulse as harmos_runtime::StreamSpec>::metadata(),
            row_metadata: Vec::new(),
            id: <MeterPulse as harmos_runtime::StreamSpec>::ID.into(),
            schema: support::Pulse::VERSION + 1,
            window: support::Pulse::WINDOW,
            row: harmos_runtime::schema::graph::Schema::from_declaration(
                <support::Pulse as harmos_runtime::schema::Described>::DEFINITION,
                &harmos_runtime::config::SchemaSettings::default(),
            )
            .unwrap(),
        }],
    ] {
        let mut peer = meter(Arc::new(AtomicUsize::new(0)));
        peer.publications = Some(grants);
        let mut fixture = operations::Managed::attach(
            peer,
            serde_json::to_vec(&()).unwrap(),
            operations::Lanes::NativeOpen,
            NonZeroUsize::new(16).unwrap(),
            vec![],
            |resident| resident.ingest::<MeterPulse>(),
        )
        .await;
        fixture.publication().unwrap();
        fixture
            .runtime
            .operations
            .register_operation::<Sampling>(
                "meter-sample",
                operations::Lanes::NativeOpen,
                Arc::new(SampleProjection { streams: fixture.runtime.streams.clone() }),
            )
            .unwrap();
        fixture.initialize().await.unwrap();
        publications::ready(fixture.ready().clone()).await;
        let refused = fixture.call(PublishPulse { sample: 42 }).await.unwrap_err();
        assert!(matches!(refused, harmos_runtime::Error::OperationFailed { source, .. }
            if matches!(source.downcast_ref::<harmos_runtime::operations::NativeError>(),
                Some(harmos_runtime::operations::NativeError::Remote(failure))
                if failure.cause().contains("stream or schema differs from the frozen generation inventory"))));
        assert_eq!(fixture.runtime.streams.query::<support::Pulse>(..).await.unwrap().count(), 0);
        fixture.shutdown().await.unwrap();
        fixture.runtime.stop().await.unwrap();
    }
}

#[tokio::test]
async fn graceful_stop_joins() {
    #[derive(Clone)]
    struct Joined<S> {
        source: S,
        ending: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }
    struct GatedEnding {
        original: harmos_sidecar::attach::Ending,
        ending: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
        joined: bool,
        waiting: Option<std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>>,
        released: bool,
    }
    impl harmos_sidecar::attach::End for GatedEnding {
        fn settle(&mut self) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + Send + '_>> {
            Box::pin(async move {
                if !self.joined {
                    self.original.settle().await?;
                    self.joined = true;
                    self.ending.notify_one();
                }
                if !self.released {
                    if self.waiting.is_none() {
                        let release = self.release.clone();
                        self.waiting = Some(Box::pin(async move { release.notified().await }));
                    }
                    self.waiting.as_mut().unwrap().await;
                    self.waiting.take();
                    self.released = true;
                }
                Ok(())
            })
        }
    }
    impl<S: Attach> Attach for Joined<S> {
        type Error = S::Error;
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
            self.source.publications()
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
            let (write, read, control, diagnostics, original) = self.source.attach().await?.into_parts();
            let receipt = harmos_sidecar::attach::Ending::new(GatedEnding {
                original: original.expect("the SDK retains its original physical ending"),
                ending: self.ending.clone(),
                release: self.release.clone(),
                joined: false,
                waiting: None,
                released: false,
            });
            let mut attached = harmos_sidecar::Attached::new(write, read).ending(receipt);
            if let Some(control) = control {
                attached = attached.control(control);
            }
            if let Some(diagnostics) = diagnostics {
                attached = attached.diagnostics(diagnostics);
            }
            Ok(attached)
        }
    }

    let ending = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let capacity = NonZeroUsize::new(1).unwrap();
    let source = Joined {
        source: harmos_sidecar::PluginNative::<Joinable>::new(capacity).unwrap(),
        ending: ending.clone(),
        release: release.clone(),
    };
    let mut fixture = operations::Managed::attach(
        source,
        serde_json::to_vec(&()).unwrap(),
        operations::Lanes::NativeOpen,
        capacity,
        vec![],
        std::convert::identity,
    )
    .await;
    fixture.initialize().await.unwrap();
    publications::ready(fixture.ready().clone()).await;
    {
        let stop = fixture.shutdown();
        tokio::pin!(stop);
        tokio::select! {
            result = &mut stop => panic!("shutdown returned without joining its attachment: {result:?}"),
            _ = ending.notified() => {}
        }
        // Cancel The Borrow While The Original Ending Retains Its Gate Wait.
    }
    release.notify_one();
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

/* === Tests === */
