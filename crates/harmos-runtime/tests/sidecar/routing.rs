/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "dependency_routing.rs"]
mod dependency_routing;

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

#[derive(Default)]
struct NoticeLog {
    values: Mutex<Vec<Notice>>,
    changed: tokio::sync::Notify,
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
    notices: Arc<NoticeLog>,
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
                    notices.values.lock().unwrap().push(notice);
                    notices.changed.notify_one();
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
                    notices.values.lock().unwrap().push(notice);
                    notices.changed.notify_one();
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
        let native = resources.resident.as_ref().unwrap().staged_operations().await
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
            assert!(self.notices.values.lock().unwrap().is_empty());
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
