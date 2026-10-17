/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;
use harmos_runtime::operations::Host as _;
use harmos_runtime::operations::{Declarations, NativeError, PluginRegistration, PluginSdkRegistration};
use harmos_runtime::plugin::{
    PluginBackend, PluginDefinition, PluginDefinitions, PluginLifecycle, PluginManager,
    PluginTransition, Requirement, Router,
};

/* === Constants === */
static NEXT_PROVIDER_SEED: AtomicUsize = AtomicUsize::new(1);
static PROVIDER_SEEDS: Mutex<std::collections::BTreeMap<usize, std::sync::Weak<Provider>>> =
    Mutex::new(std::collections::BTreeMap::new());

/* === Macros === */

/* === Type Aliases === */
type NoticeHandler = Box<dyn Fn(&Service<Bank>, Notice) + Send + Sync>;
type Resident = Supervised<Bank, Attachment, NoticeHandler>;
type Backends = PluginBackend<(), Resources, ()>;

/* === Structs === */
macro_rules! forward {
    ($name:ident, $id:literal) => {
        #[harmos_runtime::message]
        #[harmos_runtime::operation(id = $id, version = 1, policy = Amplify::POLICY, response = Amplified)]
        #[derive(Clone, PartialEq)]
        struct $name {
            value: i64,
        }

        impl Forwarded for $name {
            fn value(&self) -> i64 {
                self.value
            }
        }
    };
}

forward!(Forward, "caller.forward");

forward!(UnauthorizedForward, "unauthorized.forward");

struct Dependent {
    resources: Arc<()>,
    notify: bool,
}

struct Publishing {
    resources: Arc<()>,
    publish_on_cleanup: bool,
}

struct PublishingCall {
    _resources: Arc<()>,
}

struct DroppedHandler(Arc<tokio::sync::Notify>);

#[derive(Default)]
struct Provider {
    multiplier: i64,
    observed: Mutex<Vec<i64>>,
    started: tokio::sync::Notify,
    dropped: Arc<tokio::sync::Notify>,
    prepared: AtomicUsize,
    attempts: AtomicUsize,
    finalized: AtomicUsize,
    fail_cleanup: AtomicBool,
}

struct ProviderSeed {
    id: usize,
}

struct ProviderEndpoint {
    resources: Arc<Provider>,
}

struct CallerEndpoint {
    resources: Arc<Forwarder>,
}

struct UnauthorizedEndpoint {
    resources: Arc<Forwarder>,
}

struct Forwarder;

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "provider.retry",
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_millis(500),
        retries: RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: harmos_runtime::operations::Backoff::Fixed {
                delay: Duration::from_millis(1),
                jitter: harmos_runtime::operations::Jitter::None,
            },
        },
        restart: RestartPolicy::Never,
    },
    response = Amplified,
)]
#[derive(Clone, PartialEq)]
struct Retry {
    value: i64,
}

#[derive(Clone)]
struct Attachment {
    id: String,
    lifecycle: bool,
    publishing: bool,
    canonical: bool,
}

#[derive(Default)]
struct Resources {
    services: Option<harmos_runtime::operations::OperationManager>,
    resident: Option<Resident>,
    lease: Option<harmos_runtime::operations::OperationInstance<harmos_runtime::operations::resident::Activation>>,
    registration: Option<PluginRegistration>,
    sdk: Option<PluginSdkRegistration>,
    initializing: Option<harmos_runtime::operations::EncodedInvocation>,
    shutting_down: Option<harmos_runtime::operations::EncodedInvocation>,
    initialized: bool,
    shutdown_attempted: bool,
    registered: bool,
}

struct Host {
    operations: harmos_runtime::operations::OperationManager,
    streams: harmos_runtime::streams::StreamManager<Bank>,
    // Keeps The Original Resources Alive For The Weak Configuration Seed.
    _provider: Arc<Provider>,
    seed: ProviderSeed,
    lifecycle: bool,
    dynamic: bool,
    canonical: bool,
    capacity: NonZeroUsize,
    previous_native: Option<Arc<harmos_runtime::operations::Native>>,
    publication_resident: Option<Resident>,
    publication_start: Option<PublicationStart>,
    project_publications: bool,
    notices: Arc<Mutex<Vec<Notice>>>,
}

/* === Enums === */
enum PublicationStart {
    Commit,
    Refuse,
    Pause(Arc<tokio::sync::Notify>),
}

#[harmos_runtime::catalog]
enum PublishingOperations {
    Publish(PublishPulse),
}

#[harmos_runtime::catalog]
enum DependentRequirements {
    Amplify(Amplify),
}

#[harmos_runtime::catalog]
enum ProviderOperations {
    Amplify(Amplify),
    Retry(Retry),
}

#[harmos_runtime::catalog]
enum CallerOperations {
    Forward(Forward),
}

#[harmos_runtime::catalog]
enum UnauthorizedOperations {
    Forward(UnauthorizedForward),
}

/* === Traits === */
// The Two Forwarding Operations Share One Handler Body.
trait Forwarded: OperationDeclaration<Response = Amplified> {
    fn value(&self) -> i64;
}

/* === Implementations === */
impl Drop for ProviderSeed {
    fn drop(&mut self) {
        PROVIDER_SEEDS.lock().unwrap().remove(&self.id);
    }
}

impl harmos_runtime::operations::lifecycle::Policy for ProviderEndpoint {
    const INITIALIZATION_POLICY: OperationPolicy =
        <Dependent as harmos_runtime::operations::lifecycle::Policy>::INITIALIZATION_POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_runtime::plugin(id = "provider", provides = ProviderOperations, requires = ())]
impl harmos_runtime::Plugin for ProviderEndpoint {
    type Context = Supervisor;
    type Config = usize;
    type Error = Refusal;
    type Resources = Provider;
    fn prepare(configuration: usize) -> Result<Self, Refusal> {
        let resources = PROVIDER_SEEDS
            .lock()
            .unwrap()
            .get(&configuration)
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| Refusal::of("original provider resources unavailable"))?;
        Ok(Self { resources })
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Refusal> {
        Ok(())
    }
    fn resources(&self) -> Arc<Provider> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        table.bind_operation::<Amplifying>(self.resources.clone())?;
        table.bind_operation::<Retrying>(self.resources.clone())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Refusal> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        Ok(())
    }
}

impl harmos_runtime::operations::lifecycle::Policy for CallerEndpoint {
    const INITIALIZATION_POLICY: OperationPolicy =
        <Dependent as harmos_runtime::operations::lifecycle::Policy>::INITIALIZATION_POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_runtime::plugin(id = "caller", provides = CallerOperations, requires = DependentRequirements)]
impl harmos_runtime::Plugin for CallerEndpoint {
    type Context = Supervisor;
    type Config = ();
    type Error = Refusal;
    type Resources = Forwarder;
    fn prepare(_: ()) -> Result<Self, Refusal> {
        Ok(Self {
            resources: Arc::new(Forwarder),
        })
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Refusal> {
        Ok(())
    }
    fn resources(&self) -> Arc<Forwarder> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        table.bind_operation::<Forwarding<Forward>>(self.resources.clone())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Refusal> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        Ok(())
    }
}

impl harmos_runtime::operations::lifecycle::Policy for UnauthorizedEndpoint {
    const INITIALIZATION_POLICY: OperationPolicy =
        <Dependent as harmos_runtime::operations::lifecycle::Policy>::INITIALIZATION_POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_runtime::plugin(id = "unauthorized", provides = UnauthorizedOperations, requires = ())]
impl harmos_runtime::Plugin for UnauthorizedEndpoint {
    type Context = Supervisor;
    type Config = ();
    type Error = Refusal;
    type Resources = Forwarder;
    fn prepare(_: ()) -> Result<Self, Refusal> {
        Ok(Self {
            resources: Arc::new(Forwarder),
        })
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Refusal> {
        Ok(())
    }
    fn resources(&self) -> Arc<Forwarder> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        table.bind_operation::<Forwarding<UnauthorizedForward>>(self.resources.clone())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Refusal> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        Ok(())
    }
}

impl harmos_runtime::operations::lifecycle::Policy for Publishing {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(5),
        attempt_timeout: Duration::from_secs(5),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

impl harmos_runtime::operations::Operation for PublishingCall {
    type Declaration = PublishPulse;
    type Resources = ();
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(resources: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
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
            host.publisher::<Publishing>()
                .publish_on::<MeterPulse>(&support::pulse(request.sample))
                .await
                .map_err(Refusal::from)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

#[harmos_runtime::plugin(description = "Publishes Before Its Generation Commits.", id = "caller", streams(MeterPulse), provides = PublishingOperations, requires(harmos_runtime::operations::streams::Publish))]
impl harmos_runtime::Plugin for Publishing {
    type Context = Supervisor;
    type Config = bool;
    type Error = Refusal;
    type Resources = ();
    fn prepare(publish_on_cleanup: bool) -> Result<Self, Refusal> {
        Ok(Self {
            resources: Arc::new(()),
            publish_on_cleanup,
        })
    }
    async fn initialize(&mut self, context: &Self::Context) -> Result<(), Refusal> {
        for sample in [41, 42] {
            context
                .publisher::<Self>()
                .publish_on_context::<MeterPulse>(&support::pulse(sample), Some("initialization"))
                .await
                .map_err(Refusal::from)?;
        }
        Ok(())
    }
    fn resources(&self) -> Arc<()> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        table.bind_operation::<PublishingCall>(self.resources())
    }
    async fn stop(&mut self, context: &Self::Context) -> Result<(), Refusal> {
        if self.publish_on_cleanup {
            context
                .publisher::<Self>()
                .publish_on_context::<MeterPulse>(&support::pulse(44), Some("cleanup"))
                .await
                .expect("cleanup publication admitted");
        }
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        Ok(())
    }
}

impl harmos_runtime::operations::lifecycle::Policy for Dependent {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(5),
        attempt_timeout: Duration::from_secs(5),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_runtime::plugin(description = "Calls A Dependency During Current Lifecycle Hooks.", id = "caller", provides = (), requires = DependentRequirements)]
impl harmos_runtime::Plugin for Dependent {
    type Context = Supervisor;
    type Config = bool;
    type Error = Refusal;
    type Resources = ();
    fn prepare(notify: bool) -> Result<Self, Refusal> {
        Ok(Self {
            resources: Arc::new(()),
            notify,
        })
    }
    async fn initialize(&mut self, context: &Self::Context) -> Result<(), Refusal> {
        context
            .call(Amplify { value: 11 })
            .await
            .map_err(Refusal::from)?;
        if self.notify {
            context
                .call(Sample { value: 99 })
                .await
                .map_err(Refusal::from)?;
        }
        Ok(())
    }
    fn resources(&self) -> Arc<()> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        _: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        Ok(())
    }
    async fn stop(&mut self, context: &Self::Context) -> Result<(), Refusal> {
        context
            .call(Amplify { value: 22 })
            .await
            .expect("provider remains available through cleanup");
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        Ok(())
    }
}

impl Drop for DroppedHandler {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

struct Amplifying {
    resources: Arc<Provider>,
}

impl harmos_runtime::operations::Operation for Amplifying {
    type Declaration = Amplify;
    type Resources = Provider;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<Provider>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Amplify,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Amplified>, Refusal> {
        let result: ::core::result::Result<Amplified, Self::Error> = async move {
            if request.value == 0 {
                let _drop = DroppedHandler(self.resources.dropped.clone());
                self.resources.started.notify_one();
                std::future::pending::<()>().await;
            }
            self.resources.observed.lock().unwrap().push(request.value);
            Ok(Amplified {
                value: request.value * self.resources.multiplier,
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

struct Retrying {
    resources: Arc<Provider>,
}

impl harmos_runtime::operations::Operation for Retrying {
    type Declaration = Retry;
    type Resources = Provider;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<Provider>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Retry,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Amplified>, Refusal> {
        let result: ::core::result::Result<Amplified, Self::Error> = async move {
            self.resources.attempts.fetch_add(1, Ordering::SeqCst);
            request.value += 1;
            if request.value == 2 {
                return Err(Refusal::of("one transient failure"));
            }
            Ok(Amplified {
                value: request.value,
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        if self.resources.fail_cleanup.load(Ordering::SeqCst) {
            return Err(Refusal::of("retained cleanup"));
        }
        self.resources.finalized.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn classify(_: &Refusal) -> harmos_runtime::operations::FailureKind {
        harmos_runtime::operations::FailureKind::Transient
    }
}

struct Forwarding<F> {
    _resources: Arc<Forwarder>,
    forwarded: std::marker::PhantomData<fn() -> F>,
}

impl<F: Forwarded> harmos_runtime::operations::Operation for Forwarding<F> {
    type Declaration = F;
    type Resources = Forwarder;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<Forwarder>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            _resources: resources,
            forwarded: std::marker::PhantomData,
        })
    }
    async fn run<'ctx>(
        &mut self,
        host: Self::Context<'ctx>,
        request: &mut F,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Amplified>, Refusal> {
        let result: ::core::result::Result<Amplified, Self::Error> = async move {
            host.call(Amplify {
                value: request.value(),
            })
                .await
                .map(harmos_runtime::operations::Response::into_owned)
                .map_err(|error| match error {
                    harmos_sidecar::CallError::Refused { code, failure } => {
                        Refusal::failed(code, failure)
                    }
                    error => Refusal::of(error),
                })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Attach for Attachment {
    type Error = std::io::Error;
    fn owner(&self) -> &str {
        &self.id
    }
    fn operation_contracts(
        &self,
    ) -> harmos_domain::Result<Option<Vec<harmos_runtime::operations::Contract>>> {
        if !self.canonical {
            return Ok(None);
        }
        let contracts = match self.id.as_str() {
            "provider" => harmos_runtime::operations::Contracts::of::<ProviderOperations>().unwrap(),
            "caller" if self.publishing => {
                harmos_runtime::operations::Contracts::of::<PublishingOperations>().unwrap()
            }
            "caller" if self.lifecycle => Vec::new(),
            "caller" => harmos_runtime::operations::Contracts::of::<CallerOperations>().unwrap(),
            _ => harmos_runtime::operations::Contracts::of::<UnauthorizedOperations>().unwrap(),
        };
        Ok(Some(contracts))
    }
    fn operations(&self) -> harmos_domain::Result<Vec<harmos_runtime::operations::Definition>> {
        match self.id.as_str() {
            "provider" => Declarations::of::<ProviderOperations>(),
            "caller" if self.publishing => Declarations::of::<PublishingOperations>(),
            "caller" if self.lifecycle => Declarations::of::<()>(),
            "caller" => Declarations::of::<CallerOperations>(),
            _ => Declarations::of::<UnauthorizedOperations>(),
        }
    }
    fn publications(&self) -> Vec<harmos_sidecar::StreamDeclaration> {
        if self.publishing && self.id == "caller" {
            harmos_sidecar::PluginNative::<Publishing>::new(NonZeroUsize::new(1).unwrap())
                .unwrap()
                .publications()
        } else {
            vec![]
        }
    }
    fn sdk_lifecycle(
        &self,
    ) -> harmos_domain::Result<Option<[harmos_runtime::operations::lifecycle::Descriptor; 2]>> {
        use harmos_runtime::operations::lifecycle::Descriptor;
        let pair = match self.id.as_str() {
            "provider" => [
                Descriptor::of_initialize::<ProviderEndpoint>()
                    .expect("valid fixture lifecycle contract"),
                Descriptor::of_shutdown::<ProviderEndpoint>()
                    .expect("valid fixture lifecycle contract"),
            ],
            "caller" if self.publishing => [
                Descriptor::of_initialize::<Publishing>()
                    .expect("valid fixture lifecycle contract"),
                Descriptor::of_shutdown::<Publishing>().expect("valid fixture lifecycle contract"),
            ],
            "caller" if self.lifecycle => [
                Descriptor::of_initialize::<Dependent>().expect("valid fixture lifecycle contract"),
                Descriptor::of_shutdown::<Dependent>().expect("valid fixture lifecycle contract"),
            ],
            "caller" => [
                Descriptor::of_initialize::<CallerEndpoint>()
                    .expect("valid fixture lifecycle contract"),
                Descriptor::of_shutdown::<CallerEndpoint>()
                    .expect("valid fixture lifecycle contract"),
            ],
            _ => [
                Descriptor::of_initialize::<UnauthorizedEndpoint>()
                    .expect("valid fixture lifecycle contract"),
                Descriptor::of_shutdown::<UnauthorizedEndpoint>()
                    .expect("valid fixture lifecycle contract"),
            ],
        };
        Ok(Some(pair))
    }
    fn required_contracts(
        &self,
    ) -> harmos_domain::Result<Option<Vec<harmos_domain::inventory::contracts::Required>>> {
        let requirements = if self.id == "caller" && !self.publishing {
            harmos_runtime::operations::Contracts::of::<DependentRequirements>()
                .expect("valid fixture requirements")
                .into_iter()
                .map(|contract| harmos_domain::inventory::contracts::Required {
                    contract,
                    grants: Vec::new(),
                })
                .collect()
        } else if self.id == "caller" {
            harmos_domain::inventory::contracts::Required::from_tree(
                <Publishing as harmos_domain::plugin::Declared>::REQUIRES,
            )
            .expect("valid fixture requirements")
        } else {
            Vec::new()
        };
        Ok(Some(requirements))
    }
    async fn attach(&self) -> Result<harmos_sidecar::Attached, Self::Error> {
        if self.publishing && self.id == "caller" {
            return harmos_sidecar::PluginNative::<Publishing>::new(NonZeroUsize::new(1).unwrap())?
                .attach()
                .await;
        }
        if self.lifecycle && self.id == "caller" {
            return harmos_sidecar::PluginNative::<Dependent>::new(NonZeroUsize::new(1).unwrap())?
                .attach()
                .await;
        }
        let capacity = NonZeroUsize::new(64).unwrap();
        match self.id.as_str() {
            "provider" => {
                harmos_sidecar::PluginNative::<ProviderEndpoint>::new(capacity)?
                    .attach()
                    .await
            }
            "caller" => {
                harmos_sidecar::PluginNative::<CallerEndpoint>::new(capacity)?
                    .attach()
                    .await
            }
            _ => {
                harmos_sidecar::PluginNative::<UnauthorizedEndpoint>::new(capacity)?
                    .attach()
                    .await
            }
        }
    }
}

impl PluginLifecycle for Host {
    type Guest = ();
    type Sidecar = Resources;
    type Embedded = ();
    fn preflight(&mut self, candidate: &PluginDefinitions) -> harmos_runtime::Result<()> {
        if self.publication_start.is_some() && self.publication_resident.is_none() {
            let notices = self.notices.clone();
            let handler: NoticeHandler = Box::new(move |_, notice| {
                if matches!(notice, Notice::Published { .. }) {
                    notices.lock().unwrap().push(notice);
                }
            });
            let mut resident: Resident = Supervised::new(
                Attachment {
                    id: "caller".into(),
                    lifecycle: self.lifecycle,
                    publishing: true,
                    canonical: self.canonical,
                },
                handler,
            );
            if self.project_publications {
                resident = resident.ingest::<MeterPulse>();
            }
            self.operations
                .register_wire_operation::<harmos_runtime::sidecar::Publish<_, _, _>>(
                    "routing-publication-core",
                    HostLanes::Open,
                    Arc::new(resident.clone()),
                )?;
            self.publication_resident = Some(resident);
        }
        self.operations.validate_requirements(candidate)
    }
    fn prepare(&mut self, _: &PluginDefinition) -> harmos_runtime::Result<Backends> {
        Ok(PluginBackend::Sidecar(Resources {
            services: Some(self.operations.clone()),
            ..Resources::default()
        }))
    }
    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> harmos_runtime::Result<()> {
        Ok(())
    }
    async fn start(
        &mut self,
        definition: &PluginDefinition,
        router: &Router,
        authority: &harmos_runtime::plugin::PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> harmos_runtime::Result<()> {
        let PluginBackend::Sidecar(resources) = backend else {
            unreachable!()
        };
        let resident = if definition.id == "caller" && self.publication_start.is_some() {
            self.publication_resident
                .as_ref()
                .expect("publication provider preflighted")
                .clone()
                .routed(router)
        } else {
            let notices = self.notices.clone();
            let handler: NoticeHandler = Box::new(move |_, notice| {
                if matches!(notice, Notice::Published { .. }) {
                    notices.lock().unwrap().push(notice);
                }
            });
            Supervised::new(
                Attachment {
                    id: definition.id.clone(),
                    lifecycle: self.lifecycle,
                    publishing: false,
                    canonical: self.canonical,
                },
                handler,
            )
            .routed(router)
        };
        resources.resident = Some(resident.clone());
        let policy = harmos_runtime::services::POLICY;
        self.operations.register_service(
            harmos_runtime::services::ServiceDefinition {
                id: definition.id.clone(),
                policy,
            },
            move |_: Arc<()>| resident.clone(),
        )?;
        resources.registered = true;
        resources.lease = Some(self.operations.submit_resident(&definition.id).await?);
        let native = tokio::time::timeout(
            Duration::from_secs(2),
            resources.resident.as_ref().unwrap().staged_operations(),
        )
        .await
        .map_err(|source| harmos_runtime::Error::TaskFailed {
            domain: "fixture staged endpoint",
            source: Arc::new(source),
        })?
        .map_err(|source| harmos_runtime::Error::OperationFailed {
            operation: definition.id.clone(),
            source: Arc::new(source),
        })?;
        let handle = Arc::new(native);
        resources.registration = Some(self.operations.stage_plugin(
            handle.scope().unwrap(),
            |bindings| {
                if self.dynamic {
                    if let Some(previous) = &self.previous_native {
                        assert!(matches!(
                            bindings.register_native(
                                HostLanes::Open,
                                previous.clone(),
                                self.capacity
                            ),
                            Err(harmos_runtime::Error::InvalidInput {
                                field: "native operation scope",
                                ..
                            })
                        ));
                    }
                    bindings.register_native(HostLanes::Open, handle.clone(), self.capacity)?;
                    // A Handled Duplicate Must Preserve The First Complete Binding Set.
                    if !definition.contributions.operations.is_empty() {
                        assert!(matches!(
                            bindings.register_native(
                                HostLanes::Open,
                                handle.clone(),
                                self.capacity
                            ),
                            Err(harmos_runtime::Error::AlreadyRegistered { .. })
                        ));
                    }
                    return Ok(());
                }
                match definition.id.as_str() {
                    "provider" => bindings.register_native_catalog::<ProviderOperations>(
                        HostLanes::Open,
                        handle.clone(),
                        self.capacity,
                    ),
                    "caller" if self.publication_start.is_some() => bindings
                        .register_native_catalog::<PublishingOperations>(
                            HostLanes::Open,
                            handle.clone(),
                            self.capacity,
                        ),
                    "caller" if self.lifecycle => Ok(()),
                    "caller" => bindings.register_native_catalog::<CallerOperations>(
                        HostLanes::Open,
                        handle.clone(),
                        self.capacity,
                    ),
                    _ => bindings.register_native_catalog::<UnauthorizedOperations>(
                        HostLanes::Open,
                        handle.clone(),
                        self.capacity,
                    ),
                }
            },
        )?);
        if definition.contributions.lifecycle.is_some() {
            let sdk = handle.stage_sdk(&self.operations, HostLanes::Open)?;
            resources.sdk = Some(sdk);
            resources.initializing = Some(resources.sdk.as_ref().unwrap().submit_initialize(
                authority,
                &definition.configuration,
                NonZeroUsize::new(192).unwrap(),
            )?);
            let result = resources.initializing.as_mut().unwrap().await;
            resources.initializing.take();
            result?;
            resources.initialized = true;
        }
        if self.dynamic {
            assert!(definition.contributions.operations.iter().all(|operation| {
                matches!(
                    self.operations.definition(operation.id.as_str()),
                    Err(harmos_runtime::Error::NotReady { .. })
                )
            }));
        }
        self.previous_native = Some(handle);
        if definition.id == "caller"
            && let Some(start) = &self.publication_start
        {
            assert!(self.notices.lock().unwrap().is_empty());
            assert!(self.streams.publication("meter.pulse", 1).is_none());
            assert!(
                self.streams
                    .query::<support::Pulse>(..)
                    .await
                    .unwrap()
                    .next()
                    .is_none()
            );
            match start {
                PublicationStart::Commit => {}
                PublicationStart::Refuse => {
                    return Err(harmos_runtime::Error::InvalidInput {
                        field: "fixture activation",
                        reason: "startup refused",
                    });
                }
                PublicationStart::Pause(started) => {
                    started.notify_one();
                    std::future::pending::<()>().await;
                }
            }
        }
        Ok(())
    }
    async fn stop(
        definition: &PluginDefinition,
        authority: &harmos_runtime::plugin::PluginLifecycleAuthority<'_>,

        backend: &mut Backends,
    ) -> harmos_runtime::Result<()> {
        let PluginBackend::Sidecar(resources) = backend else {
            unreachable!()
        };
        if let Some(sdk) = &resources.sdk {
            sdk.settle().await?;
            resources.initializing.take();
            if resources.initialized && !resources.shutdown_attempted {
                resources.shutting_down = Some(sdk.submit_shutdown(authority)?);
                resources.shutdown_attempted = true;
            }
            if let Some(shutdown) = &mut resources.shutting_down {
                let outcome = shutdown.await;
                resources.shutting_down.take();
                outcome?;
                resources.initialized = false;
            }
        }
        if let Some(registration) = &mut resources.registration {
            registration.shutdown().await?;
        }
        if let Some(lease) = resources.lease.take() {
            lease.release().await?;
        }
        if resources.registered {
            let retired = resources
                .services
                .as_ref()
                .unwrap()
                .unregister_resident(&definition.id)
                .await;
            resources.registered = false;
            retired?;
        }
        resources.registered = false;
        if let Some(sdk) = &mut resources.sdk {
            sdk.close().await?;
        }
        resources.sdk.take();
        resources.registration.take();
        resources.resident.take();
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn candidate(
    host: &Host,
    lifecycle: bool,
    unauthorized: bool,
    publishing: bool,
    configuration: bool,
) -> PluginDefinitions {
    let mut owners = vec![("provider", vec![]), ("caller", vec!["provider"])];
    if unauthorized {
        owners.push(("unauthorized", vec![]));
    }
    let definitions = owners.into_iter().map(|(id, dependencies)| {
        let source = Attachment {
            id: id.into(),
            lifecycle,
            publishing,
            canonical: true,
        };
        let configuration = if id == "provider" {
            serde_json::to_vec(&host.seed.id).unwrap()
        } else if id == "caller" && (lifecycle || publishing) {
            serde_json::to_vec(&configuration).unwrap()
        } else {
            serde_json::to_vec(&()).unwrap()
        };
        let mut definition = operations::definition(&source, configuration);
        definition.requires = dependencies
            .into_iter()
            .map(|id| Requirement {
                id: id.into(),
                version: "*".parse().unwrap(),
            })
            .collect();
        definition
    });
    PluginDefinitions::new(definitions, []).unwrap()
}

fn host(runtime: &Runtime<Bank>, provider: Arc<Provider>, lifecycle: bool) -> Host {
    let id = NEXT_PROVIDER_SEED.fetch_add(1, Ordering::SeqCst);
    PROVIDER_SEEDS
        .lock()
        .unwrap()
        .insert(id, Arc::downgrade(&provider));
    Host {
        operations: runtime.operations.clone(),
        streams: runtime.streams.clone(),
        _provider: provider,
        seed: ProviderSeed { id },
        lifecycle,
        dynamic: false,
        canonical: true,
        capacity: NonZeroUsize::new(4).unwrap(),
        previous_native: None,
        publication_resident: None,
        publication_start: None,
        project_publications: true,
        notices: Arc::new(Mutex::new(Vec::new())),
    }
}

async fn published_notices(notices: &Mutex<Vec<Notice>>, count: usize) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while notices.lock().unwrap().len() < count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("initialization_rows_commit", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly()
                        .operation_settings(harmos_runtime::operations::Settings { preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()), ..Default::default() })
                        .runtime()
                        .await
                        .unwrap();
                    let mut projected = runtime.streams.watch::<support::Pulse>();
                    let mut lifecycle = host(&runtime, Arc::new(Provider::default()), true);
                    lifecycle.publication_start = Some(PublicationStart::Commit);
                    let mut manager =
                        PluginManager::new(candidate(&lifecycle, true, false, true, false))
                            .unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    for sample in [41, 42] {
                        let next = std::future::poll_fn(|context| {
                            futures_core::Stream::poll_next(
                                std::pin::Pin::new(&mut projected),
                                context,
                            )
                        });
                        let row = tokio::time::timeout(Duration::from_secs(2), next)
                            .await
                            .unwrap()
                            .unwrap();
                        assert!(
                            matches!(row, harmos_runtime::streams::Item::Row(row) if row.value.sample == sample)
                        );
                    }
                    let source = runtime.streams.publication("meter.pulse", 1).unwrap();
                    let rows: Vec<_> = source.query(..).await.unwrap().collect();
                    assert_eq!(rows.len(), 2);
                    for (row, sample) in rows.iter().zip([41, 42]) {
                        assert_eq!(
                            support::Pulse::decode(row.value.payload.as_slice())
                                .unwrap()
                                .sample,
                            sample
                        );
                    }
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    drop((manager, lifecycle, projected, source));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("notice_wire_order", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly()
                        .operation_settings(harmos_runtime::operations::Settings {
                            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                            ..Default::default()
                        })
                        .runtime()
                        .await
                        .unwrap();
                    let mut lifecycle = host(&runtime, Arc::new(Provider::default()), true);
                    lifecycle.publication_start = Some(PublicationStart::Commit);
                    lifecycle.project_publications = false;
                    lifecycle.dynamic = true;
                    let notices = lifecycle.notices.clone();
                    let mut manager =
                        PluginManager::new(candidate(&lifecycle, true, false, true, false))
                            .unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    runtime
                        .operations
                        .call_encoded(
                            PublishPulse::ID,
                            PublishPulse { sample: 43 }.encode_to_vec(),
                        )
                        .await
                        .unwrap();
                    published_notices(&notices, 3).await;
                    let retained = notices.lock().unwrap().clone();
                    assert_eq!(retained.len(), 3);
                    let expected_generation = lifecycle
                        .previous_native
                        .as_ref()
                        .unwrap()
                        .scope()
                        .unwrap()
                        .generation()
                        .get();
                    let source = runtime.streams.publication("meter.pulse", 1).unwrap();
                    let rows: Vec<_> = source.query(..).await.unwrap().collect();
                    assert_eq!(rows.len(), 3);
                    for (index, (notice, sample)) in retained.iter().zip([41, 42, 43]).enumerate() {
                        let Notice::Published { publication } = notice else {
                            panic!("only committed publications were retained")
                        };
                        let cloned = notice.clone();
                        let Notice::Published {
                            publication: shared,
                        } = &cloned
                        else {
                            unreachable!()
                        };
                        assert_eq!(publication.stream(), "meter.pulse");
                        assert_eq!(publication.owner(), "caller");
                        assert_eq!(publication.schema(), 1);
                        assert_eq!(publication.generation(), Some(expected_generation));
                        assert_eq!(publication.sequence(), index as u64 + 1);
                        assert_eq!(publication.wire_sequence(), index as u64 + 1);
                        assert_eq!(publication.missed(), 0);
                        assert_eq!(
                            support::Pulse::decode(publication.payload())
                                .unwrap()
                                .sample,
                            sample
                        );
                        assert_eq!(publication.payload(), rows[index].value.payload);
                        assert_eq!(publication.payload().as_ptr(), shared.payload().as_ptr());
                        assert_eq!(publication.owner().as_ptr(), shared.owner().as_ptr());
                        assert_eq!(
                            publication.published_unix_micros(),
                            shared.published_unix_micros()
                        );
                        assert!(publication.published_unix_micros().is_some());
                        assert_eq!(
                            publication.context(),
                            if index < 2 {
                                Some("initialization")
                            } else {
                                None
                            }
                        );
                        assert_eq!(publication.context(), shared.context());
                    }
                    assert!(
                        runtime
                            .streams
                            .query::<support::Pulse>(..)
                            .await
                            .unwrap()
                            .next()
                            .is_none()
                    );
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    drop((manager, lifecycle, source));
                    runtime.stop().await.unwrap();
                    let Notice::Published { publication } = &retained[0] else {
                        unreachable!()
                    };
                    assert_eq!(
                        support::Pulse::decode(publication.payload())
                            .unwrap()
                            .sample,
                        41
                    );
                });
        });
    }
    {
        scenario_runtime::check("shutdown_delivery", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let notices = Arc::new(Mutex::new(Vec::new()));
                    let runtime = assembly()
                        .operation_settings(harmos_runtime::operations::Settings {
                            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                            ..Default::default()
                        })
                        .runtime()
                        .await
                        .unwrap();
                    let mut lifecycle = host(&runtime, Arc::new(Provider::default()), true);
                    lifecycle.publication_start = Some(PublicationStart::Commit);
                    lifecycle.project_publications = false;
                    lifecycle.dynamic = true;
                    lifecycle.notices = notices.clone();
                    let mut manager =
                        PluginManager::new(candidate(&lifecycle, true, false, true, true)).unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    let generation = lifecycle
                        .previous_native
                        .as_ref()
                        .unwrap()
                        .scope()
                        .unwrap()
                        .generation()
                        .get();
                    published_notices(&notices, 2).await;
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                    let retained = notices.lock().unwrap();
                    assert_eq!(retained.len(), 3);
                    for (index, (notice, sample)) in retained.iter().zip([41, 42, 44]).enumerate() {
                        let Notice::Published { publication } = notice else {
                            unreachable!()
                        };
                        assert_eq!(publication.owner(), "caller");
                        assert_eq!(publication.generation(), Some(generation));
                        assert_eq!(publication.schema(), 1);
                        assert_eq!(publication.sequence(), index as u64 + 1);
                        assert_eq!(publication.wire_sequence(), index as u64 + 1);
                        assert_eq!(
                            support::Pulse::decode(publication.payload())
                                .unwrap()
                                .sample,
                            sample
                        );
                        assert_eq!(
                            publication.context(),
                            Some(if sample == 44 {
                                "cleanup"
                            } else {
                                "initialization"
                            })
                        );
                    }
                });
        });
    }
    {
        scenario_runtime::check("failed_activation_discards", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly()
                        .operation_settings(harmos_runtime::operations::Settings {
                            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                            ..Default::default()
                        })
                        .runtime()
                        .await
                        .unwrap();
                    let mut lifecycle = host(&runtime, Arc::new(Provider::default()), true);
                    lifecycle.publication_start = Some(PublicationStart::Refuse);
                    let mut manager =
                        PluginManager::new(candidate(&lifecycle, true, false, true, false))
                            .unwrap();
                    assert!(manager.initialize(&mut lifecycle).await.is_err());
                    assert!(runtime.streams.publication("meter.pulse", 1).is_none());
                    assert!(
                        runtime
                            .streams
                            .query::<support::Pulse>(..)
                            .await
                            .unwrap()
                            .next()
                            .is_none()
                    );
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancelled_activation_discards", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings { preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()), ..Default::default() })
        .runtime()
        .await
        .unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let mut lifecycle = host(&runtime, Arc::new(Provider::default()), true);
    lifecycle.publication_start = Some(PublicationStart::Pause(started.clone()));
    let mut manager = PluginManager::new(candidate(&lifecycle, true, false, true, false)).unwrap();
    let mut initialization = Box::pin(manager.initialize(&mut lifecycle));
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            result = &mut initialization => panic!("initialization ended before cancellation: {result:?}"),
            () = started.notified() => {}
        }
    }).await.unwrap();
    drop(initialization);
    assert!(runtime.streams.publication("meter.pulse", 1).is_none());
    assert!(
        runtime
            .streams
            .query::<support::Pulse>(..)
            .await
            .unwrap()
            .next()
            .is_none()
    );
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("undeclared_calls_refused", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly()
                        .operation_settings(harmos_runtime::operations::Settings {
                            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                            ..Default::default()
                        })
                        .runtime()
                        .await
                        .unwrap();
                    let provider = Arc::new(Provider::default());
                    let mut lifecycle = host(&runtime, provider.clone(), true);
                    let mut manager =
                        PluginManager::new(candidate(&lifecycle, true, false, false, true))
                            .unwrap();
                    let initialized = manager.initialize(&mut lifecycle).await;
                    assert_eq!(*provider.observed.lock().unwrap(), [11]);
                    assert!(
                        runtime
                            .streams
                            .query::<support::Pulse>(..)
                            .await
                            .unwrap()
                            .next()
                            .is_none()
                    );
                    assert!(initialized.is_err());
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    assert_eq!(*provider.observed.lock().unwrap(), [11]);
                    assert!(manager.status("caller").unwrap().generation.is_none());
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("dependency_original_manager", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings { preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()), ..Default::default() })
        .runtime()
        .await
        .unwrap();
    let provider = Arc::new(Provider {
        multiplier: 3,
        ..Default::default()
    });
    let mut lifecycle = host(&runtime, provider, false);
    let mut manager = PluginManager::new(candidate(&lifecycle, false, true, false, false)).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    assert_eq!(
        runtime
            .operations
            .call(Forward { value: 7 })
            .await
            .unwrap()
            .value,
        21
    );
    let denied = runtime
        .operations
        .call(UnauthorizedForward { value: 7 })
        .await
        .unwrap_err();
    let harmos_runtime::Error::OperationFailed { source, .. } = denied else {
        panic!("native source must survive")
    };
    assert!(
        matches!(source.downcast_ref::<NativeError>(), Some(NativeError::Remote(failure)) if failure.cause().contains("may not invoke"))
    );
    assert_eq!(
        runtime
            .operations
            .call(Forward { value: 8 })
            .await
            .unwrap()
            .value,
        24
    );
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("dependency_lifecycle_calls", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly()
                        .operation_settings(harmos_runtime::operations::Settings {
                            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                            ..Default::default()
                        })
                        .runtime()
                        .await
                        .unwrap();
                    let provider = Arc::new(Provider {
                        multiplier: 1,
                        ..Default::default()
                    });
                    let mut lifecycle = host(&runtime, provider.clone(), true);
                    let mut manager =
                        PluginManager::new(candidate(&lifecycle, true, false, false, false))
                            .unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    assert_eq!(*provider.observed.lock().unwrap(), [11]);
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    assert_eq!(*provider.observed.lock().unwrap(), [11, 22]);
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancellation_settles_handlers", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly()
                        .operation_settings(harmos_runtime::operations::Settings {
                            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                            ..Default::default()
                        })
                        .runtime()
                        .await
                        .unwrap();
                    let provider = Arc::new(Provider {
                        multiplier: 1,
                        ..Default::default()
                    });
                    let mut lifecycle = host(&runtime, provider.clone(), false);
                    let mut manager =
                        PluginManager::new(candidate(&lifecycle, false, false, false, false))
                            .unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    let invocation = runtime.operations.submit(Forward { value: 0 }).unwrap();
                    tokio::time::timeout(Duration::from_secs(1), provider.started.notified())
                        .await
                        .unwrap();
                    invocation.cancel();
                    assert!(matches!(
                        invocation.await,
                        Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Cancelled))
                    ));
                    tokio::time::timeout(Duration::from_secs(1), provider.dropped.notified())
                        .await
                        .unwrap();
                    assert_eq!(
                        runtime
                            .operations
                            .call(Forward { value: 9 })
                            .await
                            .unwrap()
                            .value,
                        9
                    );

                    let timed = runtime.operations.call(Amplify { value: 0 }).await;
                    assert!(matches!(
                        timed,
                        Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::TimedOut))
                    ));
                    tokio::time::timeout(Duration::from_secs(1), provider.dropped.notified())
                        .await
                        .unwrap();
                    assert_eq!(
                        runtime
                            .operations
                            .call(Amplify { value: 10 })
                            .await
                            .unwrap()
                            .value,
                        10
                    );
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("dependency_admission_revocation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings { preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()), ..Default::default() })
        .runtime()
        .await
        .unwrap();
    let provider = Arc::new(Provider {
        multiplier: 3,
        ..Default::default()
    });
    let mut lifecycle = host(&runtime, provider, false);
    lifecycle.dynamic = true;
    let mut manager = PluginManager::new(candidate(&lifecycle, false, true, false, false)).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    for (value, expected) in [(7, 21), (8, 24)] {
        let bytes = runtime
            .operations
            .call_encoded(Forward::ID, Message::encode_to_vec(&Forward { value }))
            .await
            .unwrap();
        assert_eq!(Amplified::decode(bytes.as_slice()).unwrap().value, expected);
    }
    let denied = runtime
        .operations
        .call_encoded(UnauthorizedForward::ID, Message::encode_to_vec(&UnauthorizedForward { value: 7 }))
        .await
        .unwrap_err();
    let harmos_runtime::Error::OperationFailed { operation, source } = denied else {
        panic!("the original native error must survive")
    };
    assert_eq!(operation, UnauthorizedForward::ID);
    assert!(
        matches!(source.downcast_ref::<NativeError>(), Some(NativeError::Remote(failure)) if failure.cause().contains("may not invoke"))
    );
    assert!(
        runtime
            .operations
            .call_encoded(Amplify::ID, vec![0xff])
            .await
            .is_err()
    );
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert!(
        runtime
            .operations
            .submit_encoded(Amplify::ID, Amplify { value: 1 }.encode_to_vec())
            .is_err()
    );
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("cancelled_dependency_capacity", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings { preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()), ..Default::default() })
        .runtime()
        .await
        .unwrap();
    let provider = Arc::new(Provider {
        multiplier: 1,
        ..Default::default()
    });
    let mut lifecycle = host(&runtime, provider.clone(), false);
    lifecycle.dynamic = true;
    lifecycle.capacity = NonZeroUsize::new(1).unwrap();
    let mut manager =
        PluginManager::new(candidate(&lifecycle, false, false, false, false)).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let invocation = runtime
        .operations
        .submit_encoded(Forward::ID, Message::encode_to_vec(&Forward { value: 0 }))
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), provider.started.notified())
        .await
        .unwrap();
    let busy = runtime
        .operations
        .submit_encoded(Amplify::ID, Amplify { value: 1 }.encode_to_vec());
    assert!(
        matches!(&busy, Err(harmos_runtime::Error::OperationFailed { source, .. }) if matches!(source.downcast_ref::<NativeError>(), Some(NativeError::Capacity))),
        "expected native capacity refusal: {:?}",
        busy.as_ref().err()
    );
    invocation.cancel();
    assert!(matches!(
        invocation.await,
        Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Cancelled))
    ));
    tokio::time::timeout(Duration::from_secs(1), provider.dropped.notified())
        .await
        .unwrap();
    let bytes = runtime
        .operations
        .call_encoded(Forward::ID, Message::encode_to_vec(&Forward { value: 9 }))
        .await
        .unwrap();
    assert_eq!(Amplified::decode(bytes.as_slice()).unwrap().value, 9);
    let timed = runtime
        .operations
        .call_encoded(Amplify::ID, Amplify { value: 0 }.encode_to_vec())
        .await;
    assert!(matches!(
        timed,
        Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::TimedOut))
    ));
    tokio::time::timeout(Duration::from_secs(1), provider.dropped.notified())
        .await
        .unwrap();
    let bytes = runtime
        .operations
        .call_encoded(Amplify::ID, Amplify { value: 10 }.encode_to_vec())
        .await
        .unwrap();
    assert_eq!(Amplified::decode(bytes.as_slice()).unwrap().value, 10);
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("empty_catalog_dependencies", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly()
                        .operation_settings(harmos_runtime::operations::Settings {
                            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                            ..Default::default()
                        })
                        .runtime()
                        .await
                        .unwrap();
                    let provider = Arc::new(Provider {
                        multiplier: 1,
                        ..Default::default()
                    });
                    let mut lifecycle = host(&runtime, provider.clone(), true);
                    lifecycle.dynamic = true;
                    let mut manager =
                        PluginManager::new(candidate(&lifecycle, true, false, false, false))
                            .unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    assert_eq!(*provider.observed.lock().unwrap(), [11]);
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    assert_eq!(*provider.observed.lock().unwrap(), [11, 22]);
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("retry_retained_finalization", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings { preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()), ..Default::default() })
        .runtime()
        .await
        .unwrap();
    let provider = Arc::new(Provider {
        multiplier: 1,
        ..Default::default()
    });
    let mut lifecycle = host(&runtime, provider.clone(), false);
    lifecycle.dynamic = true;
    lifecycle.capacity = NonZeroUsize::new(1).unwrap();
    let mut manager =
        PluginManager::new(candidate(&lifecycle, false, false, false, false)).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let result = runtime
        .operations
        .call_encoded(Retry::ID, Message::encode_to_vec(&Retry { value: 1 }))
        .await
        .unwrap();
    assert_eq!(Amplified::decode(result.as_slice()).unwrap().value, 3);
    assert_eq!(provider.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(provider.attempts.load(Ordering::SeqCst), 2);
    assert_eq!(provider.finalized.load(Ordering::SeqCst), 1);
    let oversized = runtime.operations.submit_encoded(
        Retry::ID,
        Vec::with_capacity(harmos_sidecar::Frame::LIMIT + 1),
    );
    assert!(
        matches!(oversized, Err(harmos_runtime::Error::OperationFailed { operation, source })
        if operation == Retry::ID && matches!(source.downcast_ref::<NativeError>(), Some(NativeError::Transport(harmos_runtime::services::SidecarError::Refused { code, .. })) if *code == harmos_sidecar::Frame::OVERSIZED))
    );
    assert_eq!(provider.prepared.load(Ordering::SeqCst), 1);
    provider.fail_cleanup.store(true, Ordering::SeqCst);
    let failed = runtime
        .operations
        .call_encoded(Retry::ID, Message::encode_to_vec(&Retry { value: 1 }))
        .await;
    assert!(
        matches!(failed, Err(harmos_runtime::Error::OperationFinalization { operation, .. }) if operation == Retry::ID)
    );
    assert!(runtime.operations.unregister(Retry::ID).await.is_err());
    assert!(
        matches!(runtime.operations.submit_encoded(Amplify::ID, Amplify { value: 1 }.encode_to_vec()),
        Err(harmos_runtime::Error::OperationFailed { source, .. }) if matches!(source.downcast_ref::<NativeError>(), Some(NativeError::Capacity)))
    );
    provider.fail_cleanup.store(false, Ordering::SeqCst);
    runtime.operations.unregister(Retry::ID).await.unwrap();
    assert_eq!(provider.prepared.load(Ordering::SeqCst), 2);
    assert_eq!(provider.attempts.load(Ordering::SeqCst), 4);
    assert_eq!(provider.finalized.load(Ordering::SeqCst), 2);
    runtime
        .operations
        .call_encoded(Amplify::ID, Amplify { value: 1 }.encode_to_vec())
        .await
        .unwrap();
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("capacity_precedes_encoding", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    for (dynamic, canonical) in [(true, false), (true, true), (false, true)] {
        let runtime = assembly()
            .operation_settings(harmos_runtime::operations::Settings { preparation_bytes: Some(NonZeroUsize::new(256).unwrap()), ..Default::default() })
            .runtime()
            .await
            .unwrap();
        let provider = Arc::new(Provider {
            multiplier: 1,
            ..Default::default()
        });
        let mut lifecycle = host(&runtime, provider.clone(), false);
        lifecycle.dynamic = dynamic;
        lifecycle.canonical = canonical;
        lifecycle.capacity = NonZeroUsize::new(1).unwrap();
        let mut manager =
            PluginManager::new(candidate(&lifecycle, false, false, false, false)).unwrap();
        let initialized = manager.initialize(&mut lifecycle).await;
        if !canonical {
            assert!(
                initialized.is_err(),
                "missing canonical wire contracts must fail activation"
            );
            assert!(provider.observed.lock().unwrap().is_empty());
            assert_eq!(provider.prepared.load(Ordering::SeqCst), 0);
            manager.shutdown(&mut lifecycle).await.unwrap();
            assert!(manager.status("provider").unwrap().generation.is_none());
            drop((manager, lifecycle));
            runtime.stop().await.unwrap();
            continue;
        }
        initialized.unwrap();
        let native = lifecycle.previous_native.as_ref().unwrap();
        let contract = harmos_runtime::operations::WireDeclaration::of::<Forward>()
            .contract()
            .unwrap();
        let target = runtime
            .operations
            .capture_callback(native.scope().unwrap(), &contract);
        let target = target.unwrap();
        let invocation = target
            .submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                Ok(Amplify { value: 0 }.encode_to_vec())
            })
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), provider.started.notified())
            .await
            .unwrap();
        let encoded = AtomicBool::new(false);
        let busy = target.submit_with(NonZeroUsize::new(64).unwrap(), |_| {
            encoded.store(true, Ordering::SeqCst);
            Ok(Amplify { value: 1 }.encode_to_vec())
        });
        assert!(
            matches!(&busy, Err(harmos_runtime::Error::OperationFailed { source, .. }) if matches!(source.downcast_ref::<NativeError>(), Some(NativeError::Capacity))),
            "expected native capacity refusal: {:?}",
            busy.as_ref().err()
        );
        assert!(!encoded.load(Ordering::SeqCst));
        if !dynamic {
            let busy = runtime
                .operations
                .submit(Forward { value: 1 })
                .unwrap()
                .await;
            assert!(
                matches!(busy, Err(harmos_runtime::Error::OperationFailed { source, .. })
                if matches!(source.downcast_ref::<NativeError>(), Some(NativeError::Capacity)))
            );
        }
        invocation.cancel();
        assert!(matches!(
            invocation.await,
            Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Cancelled))
        ));
        let bytes = target
            .submit_with(NonZeroUsize::new(64).unwrap(), |_| {
                Ok(Amplify { value: 9 }.encode_to_vec())
            })
            .unwrap()
            .await
            .unwrap();
        assert_eq!(Amplified::decode(&bytes).unwrap().value, 9);
        if !dynamic {
            let response = runtime
                .operations
                .call(Forward { value: 11 })
                .await
                .unwrap();
            assert_eq!(response.value, 11);
        }
        manager.shutdown(&mut lifecycle).await.unwrap();
        drop((manager, lifecycle));
        runtime.stop().await.unwrap();
    }

});
        });
    }
}

/* === Tests === */
