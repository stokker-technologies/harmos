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
pub(crate) mod operations;
mod routing;
mod listeners;
#[cfg(feature = "grpc")]
mod grants;
mod publications;
mod process;
mod consumption;
mod graph;
mod observer_demand;
mod observer_hosts;
#[path = "../support/witness.rs"]
mod witness;

mod visibility {
    use super::*;
    #[harmos::message]
    #[derive(Clone, PartialEq)]
    pub struct Scale {
        pub value: i64,
    }
    #[harmos::message]
    #[derive(Clone, PartialEq)]
    pub struct ScalePubliclyRequest {}
    pub struct ScalePrivately;
    pub struct ScalePublicly;
    impl OperationDeclaration for ScalePrivately {
        type Request = Scale;
        type Response = Scale;
        const ID: &'static str = "gauge.scale-privately";
        const POLICY: OperationPolicy = GAUGE_POLICY;
    }
    impl OperationDeclaration for ScalePublicly {
        type Request = ScalePubliclyRequest;
        type Response = Scale;
        const ID: &'static str = "gauge.scale-publicly";
        const POLICY: OperationPolicy = GAUGE_POLICY;
    }
    pub(super) struct ScalingPrivately {
        resources: Arc<GaugeResources>,
    }

    impl harmos::operations::Operation for ScalingPrivately {
        type Declaration = ScalePrivately;
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
            request: &mut Scale,
            _endpoints: &mut Self::Endpoints,
        ) -> Result<harmos::operations::Response<Scale>, Refusal> {
            let result: ::core::result::Result<Scale, Self::Error> = async move {
                Ok(Scale {
                    value: request.value * self.resources.factor,
                })
            }
            .await;
            result.map(::core::convert::Into::into)
        }
    }
    pub(super) struct ScalingPublicly {
        resources: Arc<GaugeResources>,
    }

    impl harmos::operations::Operation for ScalingPublicly {
        type Declaration = ScalePublicly;
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
            _request: &mut ScalePubliclyRequest,
            _endpoints: &mut Self::Endpoints,
        ) -> Result<harmos::operations::Response<Scale>, Refusal> {
            let result: ::core::result::Result<Scale, Self::Error> = async move {
                Ok(Scale {
                    value: self.resources.factor,
                })
            }
            .await;
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
use support::{Bank, Ledger, catalog, streams};
// Local Imports
use harmos::Message;
use harmos::operations::{
    OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, Visibility, WireTable,
};
use harmos::sidecar::{Frame, Notice, SidecarError, Supervised};
use harmos::{
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
#[harmos::message]
#[derive(Clone, PartialEq)]
struct Amplify {
    value: i64,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
struct Amplified {
    value: i64,
}

#[harmos::message]
#[derive(Clone, PartialEq, Eq)]
struct Sample {
    value: u64,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
struct Emit {
    value: u64,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
struct PublishPulse {
    sample: u64,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
struct Collapse {}

#[harmos::message]
#[derive(Clone, PartialEq)]
struct Finish {
    reason: String,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
struct BudgetedRoute {
    payload: Vec<u8>,

    reply_bytes: u32,

    refusal_bytes: u32,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
struct BudgetedReply {
    payload: Vec<u8>,
}

#[harmos::stream(description = "The whole meter sidecar, served in this process.\nCounting attachments is how a restart is observed from outside: the factory runs once per attempt, exactly as a process would be started once per attempt.", id = "meter.pulse", window = 8, row = support::Pulse)]
struct MeterPulse;

struct SampleProjection {
    streams: harmos::streams::StreamManager<Bank>,
}

struct Joinable {
    resources: Arc<()>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
struct Calibration {
    factor: i64,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
pub(crate) struct Measure {
    value: i64,
}

#[harmos::message]
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

#[harmos::message]
#[derive(Clone, PartialEq)]
pub(crate) struct StopSamplingRequest {}

#[harmos::message]
#[derive(Clone, PartialEq)]
pub(crate) struct StopSamplingResponse {}

#[harmos::message]
#[derive(Clone, PartialEq)]
pub(crate) struct GetFactorRequest {}

struct GetMeasure;

struct RunEcho;

struct RunUppercase;

struct StopSampling;

struct GetFactor;

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
#[harmos::lanes]
enum HostLanes {
    #[default]
    Open,
}

#[harmos::catalog]
enum GaugeOperations {
    Measure(GetMeasure),
    Echo(RunEcho),
    Uppercase(RunUppercase),
    Stop(StopSampling),
    Factor(GetFactor),
    Private(visibility::ScalePrivately),
    Public(visibility::ScalePublicly),
}

#[harmos::catalog]
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
impl harmos::operations::WireOperation for Amplify {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for PublishPulse {
    const VERSION: u32 = 1;
}

impl OperationDeclaration for Amplify {
    type Request = Self;
    const ID: &'static str = "meter.amplify";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const VISIBILITY: Visibility = Visibility::Shared;

    type Response = Amplified;
}

impl OperationDeclaration for Sample {
    type Request = Self;
    const ID: &'static str = "meter.sample";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const VISIBILITY: Visibility = Visibility::Host;

    type Response = ();
}

impl OperationDeclaration for Emit {
    type Request = Self;
    const ID: &'static str = "meter.emit";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const VISIBILITY: Visibility = Visibility::Shared;

    type Response = ();
}

impl OperationDeclaration for PublishPulse {
    type Request = Self;
    const ID: &'static str = "meter.publish-pulse";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const VISIBILITY: Visibility = Visibility::Shared;

    type Response = ();
}

impl OperationDeclaration for Collapse {
    type Request = Self;
    const ID: &'static str = "meter.collapse";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const VISIBILITY: Visibility = Visibility::Shared;

    type Response = ();
}

impl OperationDeclaration for Finish {
    type Request = Self;
    const ID: &'static str = "meter.finish";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const VISIBILITY: Visibility = Visibility::Shared;

    type Response = ();
}

impl OperationDeclaration for BudgetedRoute {
    type Request = Self;
    const ID: &'static str = "meter.budgeted";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const VISIBILITY: Visibility = Visibility::Shared;

    type Response = BudgetedReply;
}

impl harmos::operations::lifecycle::Policy for Joinable {
    const INITIALIZATION_POLICY: OperationPolicy = GAUGE_POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = GAUGE_POLICY;
}

#[harmos::plugin(id = "joined", provides = (), requires = ())]
impl harmos::Plugin for Joinable {
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
    ) -> Result<(), harmos::operations::ServingError> {
        Ok(())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl harmos::operations::lifecycle::Policy for Gauge {
    const INITIALIZATION_POLICY: OperationPolicy = GAUGE_POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = GAUGE_POLICY;
}

#[harmos::plugin(description = "Measures Scaled Gauge Values With One SDK Resource Owner.", id = "gauge", provides = GaugeOperations, requires = (),
    restart(credit = 2, sustained_health_ms = 250, backoff_base_ms = 10, backoff_cap_ms = 5_000))]
impl harmos::Plugin for Gauge {
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
    ) -> Result<(), harmos::operations::ServingError> {
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

impl OperationDeclaration for GetMeasure {
    type Request = Measure;
    type Response = Measured;
    const ID: &'static str = "gauge.get-measure";
    const POLICY: OperationPolicy = GAUGE_POLICY;
}

impl harmos::operations::WireOperation for GetMeasure {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for RunEcho {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for RunUppercase {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for StopSampling {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for GetFactor {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for visibility::ScalePrivately {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for visibility::ScalePublicly {
    const VERSION: u32 = 1;
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

impl harmos::operations::Operation for Measuring {
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
        request: &mut Measure,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Measured>, MeasureError> {
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

impl OperationDeclaration for RunEcho {
    type Request = String;
    type Response = String;
    const ID: &'static str = "gauge.run-echo";
    const POLICY: OperationPolicy = GAUGE_POLICY;
}

struct Echoing {
    _resources: Arc<GaugeResources>,
}

impl harmos::operations::Operation for Echoing {
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
        request: &mut String,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<String>, Refusal> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            let _ = &request;
            Ok(request.clone())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl OperationDeclaration for RunUppercase {
    type Request = String;
    type Response = String;
    const ID: &'static str = "gauge.run-uppercase";
    const POLICY: OperationPolicy = GAUGE_POLICY;
}

struct Uppercasing {
    _resources: Arc<GaugeResources>,
}

impl harmos::operations::Operation for Uppercasing {
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
        request: &mut String,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<String>, Refusal> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            let _ = &request;
            Ok(request.to_uppercase())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl OperationDeclaration for StopSampling {
    type Request = StopSamplingRequest;
    type Response = StopSamplingResponse;
    const ID: &'static str = "gauge.stop-sampling";
    const POLICY: OperationPolicy = GAUGE_POLICY;
}

struct StoppingSampling {
    resources: Arc<GaugeResources>,
}

impl harmos::operations::Operation for StoppingSampling {
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
        request: &mut StopSamplingRequest,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<StopSamplingResponse>, Refusal> {
        let result: ::core::result::Result<StopSamplingResponse, Self::Error> = async move {
            let _ = &request;
            self.resources.stopped.fetch_add(1, Ordering::SeqCst);
            Ok(StopSamplingResponse {})
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl OperationDeclaration for GetFactor {
    type Request = GetFactorRequest;
    type Response = Measured;
    const ID: &'static str = "gauge.get-factor";
    const POLICY: OperationPolicy = GAUGE_POLICY;
}

struct ReadingFactor {
    resources: Arc<GaugeResources>,
}

impl harmos::operations::Operation for ReadingFactor {
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
        request: &mut GetFactorRequest,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Measured>, Refusal> {
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

impl harmos::operations::Operation for Amplifying {
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
    ) -> Result<harmos::operations::Response<Amplified>, Refusal> {
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

impl harmos::operations::Operation for Emitting {
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
    ) -> Result<harmos::operations::Response<()>, Refusal> {
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

impl harmos::operations::Operation for PublishingPulse {
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
    ) -> Result<harmos::operations::Response<()>, Refusal> {
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

impl harmos::operations::Operation for Collapsing {
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
    ) -> Result<harmos::operations::Response<()>, Refusal> {
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

impl harmos::operations::Operation for Finishing {
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
    ) -> Result<harmos::operations::Response<()>, Refusal> {
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

impl harmos::operations::Operation for BudgetedCall {
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
    ) -> Result<harmos::operations::Response<BudgetedReply>, Refusal> {
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

impl harmos::operations::WireOperation for Sample {
    const VERSION: u32 = 1;
}

struct Sampling {
    resources: Arc<SampleProjection>,
}

impl harmos::operations::Operation for Sampling {
    type Declaration = Sample;
    type Resources = SampleProjection;
    type Error = harmos::Error;
    type Endpoints = ();
    type Context<'a> = harmos::operations::Context<'a>;
    async fn initialize<'ctx>(
        resources: Arc<SampleProjection>,
        _: Self::Context<'ctx>,
    ) -> harmos::Result<Self> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Sample,
        _endpoints: &mut Self::Endpoints,
    ) -> harmos::Result<harmos::operations::Response<()>> {
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

impl harmos::operations::lifecycle::Policy for Meter {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos::plugin(id = "meter", provides = MeterOperations, requires(harmos::operations::streams::Publish),  streams(MeterPulse))]
impl harmos::Plugin for Meter {
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
    ) -> Result<(), harmos::operations::ServingError> {
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

impl harmos::operations::WireOperation for Emit {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for Collapse {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for Finish {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for BudgetedRoute {
    const VERSION: u32 = 1;
}

impl Attach for MeterSource {
    type Error = std::io::Error;
    fn owner(&self) -> &str {
        self.source.owner()
    }
    fn operations(&self) -> harmos_domain::Result<Vec<harmos::operations::Definition>> {
        self.source.operations()
    }
    fn operation_contracts(
        &self,
    ) -> harmos_domain::Result<Option<Vec<harmos::operations::Contract>>> {
        self.source.operation_contracts()
    }
    fn required_contracts(
        &self,
    ) -> harmos_domain::Result<Option<Vec<harmos_domain::inventory::contracts::Required>>> {
        self.source.required_contracts()
    }
    fn sdk_lifecycle(
        &self,
    ) -> harmos_domain::Result<Option<[harmos::operations::lifecycle::Descriptor; 2]>> {
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
    fn observer_control(&self) -> harmos_domain::Result<Option<harmos::operations::Contract>> {
        self.source.observer_control()
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
    MeterSource {
        source: harmos_sidecar::PluginNative::new(NonZeroUsize::new(16).unwrap()).unwrap(),
        attachments,
        publications: None,
    }
}

fn assembly() -> harmos::Builder<Bank> {
    Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(streams())
        .register(HostLanes::catalog())
}

fn assert_transport_refusal(error: harmos::Error, code: i64, detail: &str) {
    let harmos::Error::OperationFailed { source, .. } = error else {
        panic!("transport refusal preserves its operation cause: {error}")
    };
    assert!(
        matches!(source.downcast_ref::<harmos::operations::NativeError>(),
        Some(harmos::operations::NativeError::Transport(SidecarError::Refused { code: actual, failure }))
            if *actual == code && failure.cause().contains(detail)),
        "{source}"
    );
}

// Await The Resident Status Change The Story Names; The Lane Deadline Ends A Hang.
async fn service_status(
    mut services: tokio::sync::watch::Receiver<
        BTreeMap<harmos::operations::resident::Key, harmos::operations::resident::Snapshot>,
    >,
    name: &str,
    wanted: impl Fn(&ServiceStatus) -> bool,
) -> ServiceStatus {
    let key = harmos::operations::resident::Key::root(name);
    let snapshot = services
        .wait_for(|items| {
            items
                .get(&key)
                .and_then(|item| item.health.as_ref())
                .is_some_and(&wanted)
        })
        .await
        .expect("the runtime keeps the status sender open");
    snapshot
        .get(&key)
        .and_then(|item| item.health.clone())
        .expect("the awaited status is present")
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
            Arc::new(SampleProjection {
                streams: fixture.runtime.streams.clone(),
            }),
        )
        .unwrap();
    fixture.initialize().await.unwrap();
    publications::ready(fixture.ready().clone()).await;
    let runtime = &fixture.runtime;
    fixture
        .call_as::<PublishPulse>(PublishPulse { sample: 42 })
        .await
        .expect("the sidecar publishes the requested sample");

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
    let definition = operations::inspected(&source, serde_json::to_vec(&()).unwrap())
        .definitions()
        .next()
        .unwrap()
        .clone();
    let calls = Supervised::new(
        source,
        (|_: &Service<Bank>, _: Notice| {}) as fn(&Service<Bank>, Notice),
    );
    let policy = harmos::operations::OperationPolicy {
        restart: harmos::operations::RestartPolicy::OnTransient {
            credit: 2,
            sustained_health: Duration::from_secs(30),
            backoff: harmos::operations::Backoff::Exponential {
                initial: Duration::from_millis(1),
                maximum: Duration::from_millis(2),
                jitter: harmos::operations::Jitter::Deterministic,
            },
        },
        ..harmos::services::POLICY
    };
    let runtime = assembly()
        .register(operations::Lanes::catalog())
        .operation_settings(harmos::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_wire_operation::<harmos::sidecar::Publish<_, _, _>>(
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
            Arc::new(SampleProjection {
                streams: runtime.streams.clone(),
            }),
        )
        .unwrap();
    let resident = calls.clone();
    runtime
        .operations
        .register_service(
            harmos::services::ServiceDefinition {
                id: "meter".into(),
                policy,
            },
            move |_: Arc<()>| {
                harmos::sidecar::Managed::new(
                    resident.clone(),
                    definition.clone(),
                    operations::Lanes::NativeOpen,
                    NonZeroUsize::new(16).unwrap(),
                    NonZeroUsize::new(4096).unwrap(),
                )
                .unwrap()
            },
        )
        .unwrap();
    let lease = runtime.operations.submit_resident("meter").await.unwrap();

    let statuses = runtime.operations.watch_residents();
    service_status(statuses.clone(), "meter", |status| {
        *status == ServiceStatus::Running { credit: 2 }
    })
    .await;
    let original = calls.operations().unwrap();
    let generation = original.scope().unwrap().generation();
    let outcome = runtime
        .operations
        .call_encoded(Collapse::ID, Collapse {}.encode_to_vec())
        .await;
    let disconnected = match &outcome {
        Err(harmos::Error::Closing {
            domain: "operation" | "operation plugin",
        }) => true,
        Err(harmos::Error::OperationFailed { operation, source }) if operation == Collapse::ID => {
            matches!(
                source.downcast_ref::<harmos::operations::NativeError>(),
                Some(harmos::operations::NativeError::Transport(
                    SidecarError::Unavailable
                ))
            )
        }
        _ => false,
    };
    assert!(
        disconnected,
        "the abandoned generation cannot answer: {outcome:?}"
    );

    service_status(statuses.clone(), "meter", |status| {
        *status == ServiceStatus::Running { credit: 1 }
    })
    .await;
    assert_eq!(
        attachments.load(Ordering::SeqCst),
        2,
        "a fresh sidecar was attached for the restart"
    );
    assert!(
        work_facts(&runtime).await.is_empty(),
        "an environmental restart is silent"
    );

    assert!(
        matches!(
            original.scope().unwrap().ready().await,
            Err(harmos::Error::Closing {
                domain: "plugin scope"
            })
        ),
        "the retired generation cannot admit calls"
    );
    assert_ne!(
        calls.operations().unwrap().scope().unwrap().generation(),
        generation
    );
    let amplified = runtime
        .operations
        .call_encoded(Amplify::ID, Amplify { value: 5 }.encode_to_vec())
        .await
        .expect("the restarted sidecar serves the same routes");
    assert_eq!(
        Amplified::decode(&amplified).unwrap(),
        Amplified { value: 10 }
    );
    lease.release().await.unwrap();
    runtime
        .operations
        .unregister_resident("meter")
        .await
        .unwrap();
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
            Arc::new(SampleProjection {
                streams: fixture.runtime.streams.clone(),
            }),
        )
        .unwrap();
    fixture.initialize().await.unwrap();
    publications::ready(fixture.ready().clone()).await;
    let request = BudgetedRoute {
        payload: vec![0; Frame::LIMIT],
        reply_bytes: 0,
        refusal_bytes: 0,
    };
    assert!(!Frame::request_fits::<BudgetedRoute>(&request));
    let oversized = fixture.call_as::<BudgetedRoute>(request).await.unwrap_err();
    assert_transport_refusal(oversized, Frame::OVERSIZED, "request");

    for request in [
        BudgetedRoute {
            payload: Vec::new(),
            reply_bytes: Frame::LIMIT as u32,
            refusal_bytes: 0,
        },
        BudgetedRoute {
            payload: Vec::new(),
            reply_bytes: 0,
            refusal_bytes: Frame::LIMIT as u32,
        },
    ] {
        let refused = fixture.call_as::<BudgetedRoute>(request).await.unwrap_err();
        assert_transport_refusal(refused, Frame::OVERSIZED, "reply");
    }

    assert_eq!(
        fixture
            .call_as::<Amplify>(Amplify { value: 5 })
            .await
            .unwrap(),
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
    let resident = Supervised::new(
        source,
        (|_: &Service<Bank>, _: Notice| {}) as fn(&Service<Bank>, Notice),
    );
    let runtime = assembly()
        .operation_settings(harmos::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_wire_operation::<harmos::sidecar::Publish<_, _, _>>(
            "meter-publication-core",
            HostLanes::Open,
            Arc::new(resident.clone()),
        )
        .unwrap();
    runtime
        .operations
        .register_service(
            harmos::services::ServiceDefinition {
                id: "meter".into(),
                policy: harmos::services::POLICY,
            },
            move |_: Arc<()>| {
                harmos::sidecar::Managed::new(
                    resident.clone(),
                    definition.clone(),
                    HostLanes::Open,
                    NonZeroUsize::new(16).unwrap(),
                    NonZeroUsize::new(4096).unwrap(),
                )
                .unwrap()
            },
        )
        .unwrap();
    let _lease = runtime.operations.submit_resident("meter").await.unwrap();
    service_status(runtime.operations.watch_residents(), "meter", |status| {
        matches!(status, ServiceStatus::Running { .. })
    })
    .await;
    let outcome = runtime
        .operations
        .call_encoded(
            Finish::ID,
            Finish {
                reason: "the recording is complete".to_owned(),
            }
            .encode_to_vec(),
        )
        .await;
    assert!(
        matches!(
            outcome,
            Err(harmos::Error::Closing {
                domain: "operation" | "operation plugin"
            })
        ),
        "a reply from the closed generation is revoked: {outcome:?}"
    );

    let status = service_status(runtime.operations.watch_residents(), "meter", |status| {
        matches!(status, ServiceStatus::Inactive { .. })
    })
    .await;

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
    assert_eq!(
        attachments.load(Ordering::SeqCst),
        1,
        "a sidecar that meant to leave is not started again"
    );

    let facts = work_facts(&runtime).await;
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].name, "deactivated");
    assert!(matches!(
        runtime.stop().await,
        Err(harmos::Error::TaskFailed {
            domain: "service",
            ..
        })
    ));
}

#[tokio::test]
async fn retired_control_refused() {
    let attached = meter(Arc::new(AtomicUsize::new(0)))
        .attach()
        .await
        .expect("the in-process sidecar attaches");
    let (write, read, control, _diagnostics, end) = attached.into_parts();
    let harmos_sidecar::transport::Connection {
        sender: outgoing,
        receiver: mut incoming,
        mut driver,
    } = harmos_sidecar::transport::Connection::new(
        (read, write),
        control.expect("the in-process attachment carries a control channel"),
    );
    let driving = tokio::spawn(async move { driver.run().await });

    // Activation Answers On The Same Exchange
    outgoing
        .send(harmos_sidecar::Frame::Call {
            id: 1,
            method: harmos_sidecar::Frame::REGISTER.to_owned(),
            params: Vec::new(),
        })
        .await
        .expect("the activation is sent");
    let answer = received(incoming.receive().await.expect("the exchange remains valid"));
    assert!(matches!(
        answer,
        Some(harmos_sidecar::Frame::Answer { id: 1, .. })
    ));

    // The Retired Control Method Is Refused As Unknown
    outgoing
        .send(harmos_sidecar::Frame::Call {
            id: 2,
            method: "harmos.initialize".to_owned(),
            params: b"{}".to_vec(),
        })
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

fn received(
    received: Option<harmos_sidecar::transport::Received>,
) -> Option<harmos_sidecar::Frame> {
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
        fixture
            .call_as::<visibility::ScalePrivately>(visibility::Scale { value: 4 })
            .await
            .unwrap(),
        visibility::Scale { value: 20 },
        "a private method's route is still named and dispatched from outside          the module it was written in"
    );
    assert_eq!(
        fixture
            .call_as::<visibility::ScalePublicly>(visibility::ScalePubliclyRequest {})
            .await
            .unwrap(),
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
        fixture
            .call_as::<GetMeasure>(Measure { value: 7 })
            .await
            .unwrap(),
        Measured { value: 21 },
        "the route answers under the configuration the entry was activated with"
    );
    let error = fixture
        .call_as::<GetMeasure>(Measure { value: -1 })
        .await
        .unwrap_err();
    let failure = error
        .failure()
        .expect("handler refusal preserves its structured failure");
    assert_eq!(failure.code(), harmos::operations::FailureKind::Permanent);
    assert_eq!(failure.cause(), "this gauge measures no negative value");
    assert_eq!(failure.context().operation(), Some(GetMeasure::ID));
    assert_eq!(
        error.to_string(),
        format!("operation `{}` failed", GetMeasure::ID),
        "the operation frame prints its own line, never the cause"
    );

    // A Nested Transport Failure Arrives As One Cause And One Operation Frame
    let error = fixture
        .call_as::<GetMeasure>(Measure { value: i64::MIN })
        .await
        .unwrap_err();
    let failure = error
        .failure()
        .expect("the handler failure crosses as a structured value");
    assert_eq!(failure.cause(), "connection reset");
    assert_eq!(failure.context().operation(), Some(GetMeasure::ID));
    assert_eq!(failure.context().step(), None);
    let diagnostics = failure.diagnostics().expect("origin diagnostics");
    assert_eq!(diagnostics.origin(), harmos::failure::Origin::Plugin);
    assert_eq!(
        diagnostics.chain().len(),
        3,
        "the full origin chain stays in diagnostics"
    );
    let rendered = std::iter::successors(
        Some(&error as &(dyn std::error::Error + 'static)),
        |error| error.source(),
    )
    .map(ToString::to_string)
    .collect::<Vec<_>>()
    .join(": ");
    assert_eq!(
        rendered,
        format!("operation `{}` failed: connection reset", GetMeasure::ID),
        "no carrier repeats the cause or adds a frame"
    );

    assert_eq!(
        fixture
            .call_as::<RunEcho>("hello".to_owned())
            .await
            .unwrap(),
        "hello".to_owned()
    );
    assert_eq!(
        fixture
            .call_as::<RunUppercase>("hello".to_owned())
            .await
            .unwrap(),
        "HELLO".to_owned()
    );

    assert_eq!(
        fixture
            .call_as::<StopSampling>(StopSamplingRequest {})
            .await
            .unwrap(),
        StopSamplingResponse {},
        "a route carrying nothing either way still names one message each way"
    );
    assert_eq!(
        fixture
            .call_as::<GetFactor>(GetFactorRequest {})
            .await
            .unwrap(),
        Measured { value: 3 },
        "a reply written through the crate's own alias dispatches like any other"
    );

    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
    assert!(
        GAUGE_DEACTIVATED.load(Ordering::SeqCst),
        "the local attachment runs the generated entry's awaited deactivation"
    );
}

#[tokio::test]
async fn ingestion_authority_bounded() {
    for grants in [
        Vec::new(),
        vec![harmos_sidecar::StreamDeclaration {
            metadata: <MeterPulse as harmos::StreamSpec>::metadata(),
            row_metadata: Vec::new(),
            id: <MeterPulse as harmos::StreamSpec>::ID.into(),
            schema: support::Pulse::VERSION + 1,
            window: support::Pulse::WINDOW,
            row: harmos::schema::graph::Schema::from_declaration(
                <support::Pulse as harmos::schema::Described>::DEFINITION,
                &harmos::config::SchemaSettings::default(),
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
                Arc::new(SampleProjection {
                    streams: fixture.runtime.streams.clone(),
                }),
            )
            .unwrap();
        fixture.initialize().await.unwrap();
        publications::ready(fixture.ready().clone()).await;
        let refused = fixture
            .call_as::<PublishPulse>(PublishPulse { sample: 42 })
            .await
            .unwrap_err();
        assert!(
            matches!(refused, harmos::Error::OperationFailed { source, .. }
            if matches!(source.downcast_ref::<harmos::operations::NativeError>(),
                Some(harmos::operations::NativeError::Remote(failure))
                if failure.cause().contains("stream or schema differs from the frozen generation inventory")))
        );
        assert_eq!(
            fixture
                .runtime
                .streams
                .query::<support::Pulse>(..)
                .await
                .unwrap()
                .count(),
            0
        );
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
        fn settle(
            &mut self,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + Send + '_>>
        {
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
        fn operations(&self) -> harmos_domain::Result<Vec<harmos::operations::Definition>> {
            self.source.operations()
        }
        fn operation_contracts(
            &self,
        ) -> harmos_domain::Result<Option<Vec<harmos::operations::Contract>>> {
            self.source.operation_contracts()
        }
        fn required_contracts(
            &self,
        ) -> harmos_domain::Result<Option<Vec<harmos_domain::inventory::contracts::Required>>>
        {
            self.source.required_contracts()
        }
        fn sdk_lifecycle(
            &self,
        ) -> harmos_domain::Result<Option<[harmos::operations::lifecycle::Descriptor; 2]>> {
            self.source.sdk_lifecycle()
        }
        fn publications(&self) -> Vec<harmos_sidecar::StreamDeclaration> {
            self.source.publications()
        }
        fn observers(&self) -> Vec<harmos_sidecar::inventory::observers::Declaration> {
            self.source.observers()
        }
        fn observer_control(&self) -> harmos_domain::Result<Option<harmos::operations::Contract>> {
            self.source.observer_control()
        }
        fn listener_declarations(
            &self,
        ) -> harmos_domain::Result<Option<Vec<harmos_domain::listeners::Definition>>> {
            self.source.listener_declarations()
        }
        async fn attach(&self) -> Result<harmos_sidecar::Attached, Self::Error> {
            let (write, read, control, diagnostics, original) =
                self.source.attach().await?.into_parts();
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
