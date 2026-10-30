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
    PluginBackend, PluginDefinition, PluginDefinitions, PluginLifecycle, PluginManager, PluginTransition, Requirement, Router,
};

/* === Constants === */

static NEXT_PROVIDER_SEED: AtomicUsize = AtomicUsize::new(1);
static PROVIDER_SEEDS: Mutex<std::collections::BTreeMap<usize, std::sync::Weak<Provider>>> = Mutex::new(std::collections::BTreeMap::new());

/* === Macros === */

macro_rules! forward {
    ($name:ident, $id:literal, [$($callee:ty),*]) => {
        #[harmos_runtime::operation(resources = [Forwarder, Supervisor], operations = [$($callee),*], id = $id, policy = AmplifyDeclaration::POLICY, response = Amplified)]
        #[derive(Clone, PartialEq)]
        struct $name {
            value: i64,
        }

        impl harmos_runtime::Operation for $name {
            type Error = Refusal;
            async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Amplified>, Self::Error> {
                ctx.resource::<Supervisor>().call::<AmplifyDeclaration>(AmplifyDeclaration { value: self.value }).await
                    .map_err(|error| match error {
                        harmos_sidecar::CallError::Refused { code, failure } => Refusal::failed(code, failure),
                        error => Refusal::of(error),
                    })
            }
        }
    };
}

forward!(Forward, "caller.forward", [AmplifyDeclaration]);

forward!(UnauthorizedForward, "unauthorized.forward", []);

/* === Type Aliases === */

type NoticeHandler = Box<dyn Fn(&ServiceContext<Bank>, Notice) + Send + Sync>;
type Resident = Supervised<Bank, Attachment, NoticeHandler>;
type Backends = PluginBackend<(), Resources, ()>;

/* === Structs === */

struct Dependent {
    notify: bool,
}

struct Publishing {
    publish_on_cleanup: bool,
}

#[harmos_runtime::operation(codec = manual, resources = [Supervisor], id = PublishPulseDeclaration::ID, response = (),
    policy = PublishPulseDeclaration::POLICY, visibility = Public)]
#[derive(Clone)]
struct PublishingCall(PublishPulseDeclaration);

impl harmos_runtime::Message for PublishingCall {
    const CODEC: &'static str = <PublishPulseDeclaration as harmos_runtime::Message>::CODEC;
    fn encode(&self) -> Vec<u8> {
        harmos_runtime::Message::encode(&self.0)
    }
    fn encoded_size(&self) -> usize {
        harmos_runtime::Message::encoded_size(&self.0)
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        <PublishPulseDeclaration as harmos_runtime::Message>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        <PublishPulseDeclaration as harmos_runtime::Message>::decode(bytes).map(Self)
    }
}
impl harmos_runtime::MemorySize for PublishingCall {
    fn memory_size(&self) -> usize {
        harmos_runtime::MemorySize::memory_size(&self.0)
    }
}
impl harmos_runtime::schema::Described for PublishingCall {
    const NAME: &'static str = <PublishPulseDeclaration as harmos_runtime::schema::Described>::NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] = <PublishPulseDeclaration as harmos_runtime::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = <PublishPulseDeclaration as harmos_runtime::schema::Described>::MARKER;
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

#[harmos_runtime::operation(resources = [Provider], id = "provider.retry",
    policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_millis(500),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: harmos_runtime::operations::Backoff::Fixed {
                delay: Duration::from_millis(1),
                jitter: harmos_runtime::operations::Jitter::None,
            },
        },
    },
    response = Amplified,
)]
#[derive(Clone, PartialEq)]
struct RetryDeclaration {
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
    services: Option<harmos_runtime::operations::OperationManager<super::support::Lanes>>,
    resident: Option<Resident>,
    lease: Option<harmos_runtime::operations::Running<harmos_runtime::operations::resident::Activation>>,
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
    operations: harmos_runtime::operations::OperationManager<super::support::Lanes>,
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

#[harmos_runtime::operation(codec = manual, resources = [Provider], id = AmplifyDeclaration::ID, response = Amplified,
    policy = AmplifyDeclaration::POLICY, visibility = Public)]
#[derive(Clone)]
struct Amplifying(AmplifyDeclaration);

impl harmos_runtime::Message for Amplifying {
    const CODEC: &'static str = <AmplifyDeclaration as harmos_runtime::Message>::CODEC;
    fn encode(&self) -> Vec<u8> {
        harmos_runtime::Message::encode(&self.0)
    }
    fn encoded_size(&self) -> usize {
        harmos_runtime::Message::encoded_size(&self.0)
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        <AmplifyDeclaration as harmos_runtime::Message>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        <AmplifyDeclaration as harmos_runtime::Message>::decode(bytes).map(Self)
    }
}
impl harmos_runtime::MemorySize for Amplifying {
    fn memory_size(&self) -> usize {
        harmos_runtime::MemorySize::memory_size(&self.0)
    }
}
impl harmos_runtime::schema::Described for Amplifying {
    const NAME: &'static str = <AmplifyDeclaration as harmos_runtime::schema::Described>::NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] = <AmplifyDeclaration as harmos_runtime::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = <AmplifyDeclaration as harmos_runtime::schema::Described>::MARKER;
}

struct RetryGuard(Arc<Provider>);

/* === Enums === */

enum PublicationStart {
    Commit,
    Refuse,
    Pause(Arc<tokio::sync::Notify>),
}

#[harmos_runtime::catalog]
enum PublishingOperations {
    Publish(PublishingCall),
}

#[harmos_runtime::catalog]
enum DependentRequirements {
    AmplifyDeclaration(AmplifyDeclaration),
}

#[harmos_runtime::catalog]
enum ProviderOperations {
    AmplifyDeclaration(Amplifying),
    RetryDeclaration(RetryDeclaration),
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

/* === Implementations === */

impl Drop for ProviderSeed {
    fn drop(&mut self) {
        PROVIDER_SEEDS.lock().unwrap().remove(&self.id);
    }
}

impl harmos_operations::lifecycle::Policy for ProviderEndpoint {
    const INITIALIZATION_POLICY: OperationPolicy = <Dependent as harmos_operations::lifecycle::Policy>::INITIALIZATION_POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

impl harmos_runtime::Lifecycle for ProviderEndpoint {
    type Context = Supervisor;
    type Config = usize;
    type Error = Refusal;

    async fn prepare(configuration: usize, setup: &mut harmos_sdk::Setup<Self>) -> Result<Self, Refusal> {
        let resources = PROVIDER_SEEDS
            .lock()
            .unwrap()
            .get(&configuration)
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| Refusal::of("original provider resources unavailable"))?;
        let plugin = Self { resources };
        setup.resource::<Provider>(plugin.resources.clone());
        Ok(plugin)
    }
}

#[harmos_macros::plugin(harmos_path = ::harmos_runtime, id = "provider", requires = ())]
impl harmos_runtime::Plugin for ProviderEndpoint {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[ProviderOperations::CATALOG];
}

impl harmos_operations::lifecycle::Policy for CallerEndpoint {
    const INITIALIZATION_POLICY: OperationPolicy = <Dependent as harmos_operations::lifecycle::Policy>::INITIALIZATION_POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

impl harmos_runtime::Lifecycle for CallerEndpoint {
    type Context = Supervisor;
    type Config = ();
    type Error = Refusal;

    async fn prepare(_: (), setup: &mut harmos_sdk::Setup<Self>) -> Result<Self, Refusal> {
        let plugin = Self { resources: Arc::new(Forwarder) };
        setup.resource::<Forwarder>(plugin.resources.clone());
        Ok(plugin)
    }
}

#[harmos_macros::plugin(harmos_path = ::harmos_runtime, id = "caller", requires = DependentRequirements)]
impl harmos_runtime::Plugin for CallerEndpoint {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[CallerOperations::CATALOG];
}

impl harmos_operations::lifecycle::Policy for UnauthorizedEndpoint {
    const INITIALIZATION_POLICY: OperationPolicy = <Dependent as harmos_operations::lifecycle::Policy>::INITIALIZATION_POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

impl harmos_runtime::Lifecycle for UnauthorizedEndpoint {
    type Context = Supervisor;
    type Config = ();
    type Error = Refusal;

    async fn prepare(_: (), setup: &mut harmos_sdk::Setup<Self>) -> Result<Self, Refusal> {
        let plugin = Self { resources: Arc::new(Forwarder) };
        setup.resource::<Forwarder>(plugin.resources.clone());
        Ok(plugin)
    }
}

#[harmos_macros::plugin(harmos_path = ::harmos_runtime, id = "unauthorized", requires = ())]
impl harmos_runtime::Plugin for UnauthorizedEndpoint {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[UnauthorizedOperations::CATALOG];
}

impl harmos_operations::lifecycle::Policy for Publishing {
    const INITIALIZATION_POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(5),
        attempt_timeout: Duration::from_secs(5),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: RetryPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

impl harmos_runtime::Operation for PublishingCall {
    type Error = Refusal;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let host = ctx.resource::<Supervisor>();
        let request = self.0;
        let result: ::core::result::Result<(), Self::Error> = async move {
            host.publisher::<Publishing>().publish_on::<MeterPulse>(&support::pulse(request.sample)).await.map_err(Refusal::from)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Lifecycle for Publishing {
    type Context = Supervisor;
    type Config = bool;
    type Error = Refusal;

    async fn prepare(publish_on_cleanup: bool, _setup: &mut harmos_sdk::Setup<Self>) -> Result<Self, Refusal> {
        Ok(Self { publish_on_cleanup })
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
}

#[harmos_macros::plugin(harmos_path = ::harmos_runtime, description = "Publishes Before Its Generation Commits.", id = "caller", streams(MeterPulse), requires(harmos_operations::streams::Publish))]
impl harmos_runtime::Plugin for Publishing {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[PublishingOperations::CATALOG];
}

impl harmos_operations::lifecycle::Policy for Dependent {
    const INITIALIZATION_POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(5),
        attempt_timeout: Duration::from_secs(5),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: RetryPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

impl harmos_runtime::Lifecycle for Dependent {
    type Context = Supervisor;
    type Config = bool;
    type Error = Refusal;

    async fn prepare(notify: bool, _setup: &mut harmos_sdk::Setup<Self>) -> Result<Self, Refusal> {
        Ok(Self { notify })
    }

    async fn initialize(&mut self, context: &Self::Context) -> Result<(), Refusal> {
        context.call::<AmplifyDeclaration>(AmplifyDeclaration { value: 11 }).await.map_err(Refusal::from)?;
        if self.notify {
            context.call::<SampleDeclaration>(SampleDeclaration { value: 99 }).await.map_err(Refusal::from)?;
        }
        Ok(())
    }

    async fn stop(&mut self, context: &Self::Context) -> Result<(), Refusal> {
        context.call::<AmplifyDeclaration>(AmplifyDeclaration { value: 22 }).await.expect("provider remains available through cleanup");
        Ok(())
    }
}

#[harmos_macros::plugin(harmos_path = ::harmos_runtime, description = "Calls A Dependency During Current Lifecycle Hooks.", id = "caller", requires = DependentRequirements)]
impl harmos_runtime::Plugin for Dependent {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[];
}

impl Drop for DroppedHandler {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

impl harmos_runtime::Operation for Amplifying {
    type Error = Refusal;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<Amplified>, Refusal> {
        let request = self.0;
        let resources = ctx.resource::<Provider>();
        let result: ::core::result::Result<Amplified, Self::Error> = async move {
            if request.value == 0 {
                let _drop = DroppedHandler(resources.dropped.clone());
                resources.started.notify_one();
                std::future::pending::<()>().await;
            }
            resources.observed.lock().unwrap().push(request.value);
            Ok(Amplified {
                value: request.value * resources.multiplier,
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for RetryDeclaration {
    type Error = Refusal;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Amplified>, Self::Error> {
        let resources = ctx.resource::<Provider>();
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        let _guard = RetryGuard(resources.clone());
        let attempt = resources.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt.is_multiple_of(2) {
            return Err(Refusal::of("one transient failure"));
        }
        Ok(Amplified { value: self.value + 2 }.into())
    }
    fn classify(_: &Refusal) -> harmos_runtime::operations::FailureKind {
        harmos_runtime::operations::FailureKind::Transient
    }
}
impl Drop for RetryGuard {
    fn drop(&mut self) {
        self.0.finalized.fetch_add(1, Ordering::SeqCst);
    }
}

impl Attach for Attachment {
    type Error = std::io::Error;
    fn owner(&self) -> &str {
        &self.id
    }
    fn operation_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_runtime::operations::Contract>>> {
        if !self.canonical {
            return Ok(None);
        }
        let contracts = match self.id.as_str() {
            "provider" => harmos_runtime::operations::Contracts::of::<ProviderOperations>().unwrap(),
            "caller" if self.publishing => harmos_runtime::operations::Contracts::of::<PublishingOperations>().unwrap(),
            "caller" if self.lifecycle => Vec::new(),
            "caller" => harmos_runtime::operations::Contracts::of::<CallerOperations>().unwrap(),
            _ => harmos_runtime::operations::Contracts::of::<UnauthorizedOperations>().unwrap(),
        };
        Ok(Some(contracts))
    }
    fn operations(&self) -> harmos_domain::Result<Vec<harmos_runtime::operations::OperationDefinition>> {
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
            harmos_sidecar::PluginNative::<Publishing>::new(NonZeroUsize::new(1).unwrap()).unwrap().publications()
        } else {
            vec![]
        }
    }
    fn sdk_lifecycle(&self) -> harmos_domain::Result<Option<[harmos_operations::lifecycle::Descriptor; 2]>> {
        use harmos_operations::lifecycle::Descriptor;
        let pair = match self.id.as_str() {
            "provider" => [
                Descriptor::of_initialize::<ProviderEndpoint>().expect("valid fixture lifecycle contract"),
                Descriptor::of_shutdown::<ProviderEndpoint>().expect("valid fixture lifecycle contract"),
            ],
            "caller" if self.publishing => [
                Descriptor::of_initialize::<Publishing>().expect("valid fixture lifecycle contract"),
                Descriptor::of_shutdown::<Publishing>().expect("valid fixture lifecycle contract"),
            ],
            "caller" if self.lifecycle => [
                Descriptor::of_initialize::<Dependent>().expect("valid fixture lifecycle contract"),
                Descriptor::of_shutdown::<Dependent>().expect("valid fixture lifecycle contract"),
            ],
            "caller" => [
                Descriptor::of_initialize::<CallerEndpoint>().expect("valid fixture lifecycle contract"),
                Descriptor::of_shutdown::<CallerEndpoint>().expect("valid fixture lifecycle contract"),
            ],
            _ => [
                Descriptor::of_initialize::<UnauthorizedEndpoint>().expect("valid fixture lifecycle contract"),
                Descriptor::of_shutdown::<UnauthorizedEndpoint>().expect("valid fixture lifecycle contract"),
            ],
        };
        Ok(Some(pair))
    }
    fn required_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_domain::inventory::contracts::Required>>> {
        let requirements = if self.id == "caller" && !self.publishing {
            harmos_runtime::operations::Contracts::of::<DependentRequirements>()
                .expect("valid fixture requirements")
                .into_iter()
                .map(|contract| harmos_domain::inventory::contracts::Required { contract, grants: Vec::new() })
                .collect()
        } else if self.id == "caller" {
            harmos_domain::inventory::contracts::Required::from_tree(<Publishing as harmos_domain::plugin::Declared>::REQUIRES)
                .expect("valid fixture requirements")
        } else {
            Vec::new()
        };
        Ok(Some(requirements))
    }
    async fn attach(&self) -> Result<harmos_sidecar::Attached, Self::Error> {
        if self.publishing && self.id == "caller" {
            return harmos_sidecar::PluginNative::<Publishing>::new(NonZeroUsize::new(1).unwrap())?.attach().await;
        }
        if self.lifecycle && self.id == "caller" {
            return harmos_sidecar::PluginNative::<Dependent>::new(NonZeroUsize::new(1).unwrap())?.attach().await;
        }
        let capacity = NonZeroUsize::new(64).unwrap();
        match self.id.as_str() {
            "provider" => harmos_sidecar::PluginNative::<ProviderEndpoint>::new(capacity)?.attach().await,
            "caller" => harmos_sidecar::PluginNative::<CallerEndpoint>::new(capacity)?.attach().await,
            _ => harmos_sidecar::PluginNative::<UnauthorizedEndpoint>::new(capacity)?.attach().await,
        }
    }
}

impl Host {
    async fn prepare_publication(&mut self) -> harmos_runtime::Result<()> {
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
            self.operations.register::<crate::sidecar::PublicationOperations>(HostLanes::Open).await?;
            self.publication_resident = Some(resident);
        }
        Ok(())
    }
}

impl PluginLifecycle for Host {
    type Guest = ();
    type Sidecar = Resources;
    type Embedded = ();
    fn preflight(&mut self, candidate: &PluginDefinitions) -> harmos_runtime::Result<()> {
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
        &mut self, definition: &PluginDefinition, router: &Router, authority: &harmos_runtime::plugin::PluginLifecycleAuthority<'_>,
        backend: &mut Backends,
    ) -> harmos_runtime::Result<()> {
        let PluginBackend::Sidecar(resources) = backend else { unreachable!() };
        let resident = if definition.id == "caller" && self.publication_start.is_some() {
            self.publication_resident.as_ref().expect("publication provider preflighted").clone().routed(router)
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
            harmos_runtime::services::ServiceDefinition { id: definition.id.clone(), policy },
            move |_: Arc<harmos_sdk::ExecutionContext>| resident.clone(),
        )?;
        resources.registered = true;
        resources.lease = Some(self.operations.submit_resident(&definition.id).await?);
        let native =
            resources.resident.as_ref().unwrap().staged_operations().await.map_err(|source| harmos_runtime::Error::OperationFailed {
                operation: definition.id.clone(),
                source: Arc::new(source),
            })?;
        let handle = Arc::new(native);
        resources.registration = Some(self.operations.stage_plugin(handle.scope().unwrap(), |bindings| {
            if self.dynamic {
                if let Some(previous) = &self.previous_native {
                    assert!(matches!(
                        bindings.register_native(HostLanes::Open, previous.clone(), self.capacity),
                        Err(harmos_runtime::Error::InvalidInput { field: "native operation scope", .. })
                    ));
                }
                bindings.register_native(HostLanes::Open, handle.clone(), self.capacity)?;
                // A Handled Duplicate Must Preserve The First Complete Binding Set.
                if !definition.contributions.operations.is_empty() {
                    assert!(matches!(
                        bindings.register_native(HostLanes::Open, handle.clone(), self.capacity),
                        Err(harmos_runtime::Error::AlreadyRegistered { .. })
                    ));
                }
                return Ok(());
            }
            match definition.id.as_str() {
                "provider" => bindings.register_native_catalog::<ProviderOperations>(HostLanes::Open, handle.clone(), self.capacity),
                "caller" if self.publication_start.is_some() => {
                    bindings.register_native_catalog::<PublishingOperations>(HostLanes::Open, handle.clone(), self.capacity)
                }
                "caller" if self.lifecycle => Ok(()),
                "caller" => bindings.register_native_catalog::<CallerOperations>(HostLanes::Open, handle.clone(), self.capacity),
                _ => bindings.register_native_catalog::<UnauthorizedOperations>(HostLanes::Open, handle.clone(), self.capacity),
            }
        })?);
        if definition.contributions.lifecycle.is_some() {
            let sdk = handle.stage_sdk(&self.operations, HostLanes::Open)?;
            resources.sdk = Some(sdk);
            resources.initializing = Some(resources.sdk.as_ref().unwrap().submit_initialize(
                authority,
                &definition.configuration,
                NonZeroUsize::new(192 + 3072).unwrap(),
            )?);
            let result = resources.initializing.as_mut().unwrap().await;
            resources.initializing.take();
            result?;
            resources.initialized = true;
        }
        if self.dynamic {
            assert!(definition.contributions.operations.iter().all(|operation| {
                matches!(self.operations.definition(operation.id.as_str()), Err(harmos_runtime::Error::NotReady { .. }))
            }));
        }
        self.previous_native = Some(handle);
        if definition.id == "caller"
            && let Some(start) = &self.publication_start
        {
            assert!(self.notices.values.lock().unwrap().is_empty());
            assert!(self.streams.publication("meter.pulse", 1).is_none());
            assert!(self.streams.query::<support::Pulse>(..).await.unwrap().next().is_none());
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
        definition: &PluginDefinition, authority: &harmos_runtime::plugin::PluginLifecycleAuthority<'_>, backend: &mut Backends,
    ) -> harmos_runtime::Result<()> {
        let PluginBackend::Sidecar(resources) = backend else { unreachable!() };
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
            let retired = resources.services.as_ref().unwrap().unregister_resident(&definition.id).await;
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

/* === Tests === */
