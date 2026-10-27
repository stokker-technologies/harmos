/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Enforce Request Grants For External Callers Across Sidecar And In-Process Bindings

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::operations::{Lanes, Managed};
use harmos_domain::plugin::Grant;
use harmos_runtime::grpc::authentication::{Authenticate, Authentication, Authenticator};
use harmos_runtime::grpc::middleware::Invocation;
use harmos_runtime::operations::{Handler, OperationPolicy, RetryPolicy, Visibility, WireTable};
use harmos_sidecar::{Refusal, Supervisor};

/* === Constants === */

const ALLOWED: &str = "fixture.remote.allowed";
const REFUSED: &str = "fixture.remote.refused";
const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    retries: RetryPolicy::Never,
};
static NEXT_SEED: AtomicUsize = AtomicUsize::new(1);
static SEEDS: Mutex<std::collections::BTreeMap<usize, Weak<Runs>>> = Mutex::new(std::collections::BTreeMap::new());

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Runs(AtomicUsize);

struct Seed {
    id: usize,
}

struct Principal(&'static str);

struct Bearers;

// Selects The Scoped Grants Each Bearer Holds On The Called Handler; Unknown
// Bearers Are Denied.
struct Holdings;

struct Provider {
    runs: Arc<Runs>,
}

#[harmos_runtime::message]
#[derive(Clone)]
struct Fetch {
    value: u32,
}

#[harmos_runtime::operation(id = "gated_remote",
    policy = POLICY,
    response = Fetch,
    visibility = Visibility::Host,
    grpc(package = "fixture.gated.v1", service = "Remote", method = "Fetch"),
    grant = |request: &RemoteFetchDeclaration| scoped("gated_remote", request.value),
)]
#[derive(Clone)]
struct RemoteFetchDeclaration {
    value: u32,
}

struct RemoteFetching {
    runs: Arc<Runs>,
}

#[harmos_runtime::operation(id = "gated_local",
    policy = POLICY,
    response = Fetch,
    visibility = Visibility::Host,
    grpc(package = "fixture.gated.v1", service = "Local", method = "Fetch"),
    grant = |request: &LocalFetchDeclaration| scoped("gated_local", request.value),
)]
#[derive(Clone)]
struct LocalFetchDeclaration {
    value: u32,
}

struct LocalFetching {
    runs: Arc<Runs>,
}

/* === Enums === */

#[harmos_runtime::catalog]
enum Gated {
    Fetch(RemoteFetchDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl Seed {
    fn new(runs: &Arc<Runs>) -> Self {
        let id = NEXT_SEED.try_update(Ordering::SeqCst, Ordering::SeqCst, |id| id.checked_add(1)).unwrap();
        let mut seeds = SEEDS.lock().unwrap();
        assert!(seeds.len() < 64, "fixture handoff stays bounded");
        assert!(seeds.insert(id, Arc::downgrade(runs)).is_none());
        Self { id }
    }
}

impl Drop for Seed {
    fn drop(&mut self) {
        SEEDS.lock().unwrap().remove(&self.id);
    }
}

impl Authenticator for Bearers {
    type Principal = Principal;
    fn authenticate<'a>(&'a self, call: &'a Invocation) -> Authentication<'a, Principal> {
        Box::pin(async move {
            match call.metadata().get("authorization").and_then(|value| value.to_str().ok()) {
                Some("Bearer holder") => Ok(Principal("holder")),
                Some("Bearer lacking") => Ok(Principal("lacking")),
                Some("Bearer full") => Ok(Principal("full")),
                _ => Err(harmos_runtime::Error::Unauthenticated),
            }
        })
    }
}

impl harmos_runtime::operations::authorization::Authorizer for Holdings {
    fn authorize(
        &self, context: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Authorization>> {
        use harmos_runtime::operations::authorization::{Authorization, Required};
        assert_eq!(provider.plugin, provider.owner.as_str() == "gated", "provenance comes from the original plugin scope");
        let grants = match context.get::<Principal>().map(|principal| principal.0) {
            Some("full") => return Ok(Some(Authorization::Full(provider.contract.clone()))),
            Some("holder") => vec![Grant::of(provider.contract.operation().id.as_str().to_owned(), ALLOWED)],
            Some("lacking") => vec![],
            _ => return Ok(None),
        };
        Ok(Some(Authorization::Restricted(Required {
            contract: provider.contract.clone(),
            grants,
        })))
    }
}

impl harmos_operations::lifecycle::Policy for Provider {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos_runtime::plugin(id = "gated", provides = Gated, requires = ())]
impl harmos_runtime::Plugin for Provider {
    type Config = usize;
    type Context = Supervisor;
    type Resources = Runs;
    type Error = Refusal;
    fn prepare(id: usize) -> Result<Self, Self::Error> {
        let runs = SEEDS
            .lock()
            .unwrap()
            .remove(&id)
            .and_then(|runs| runs.upgrade())
            .ok_or_else(|| Refusal::of("original gated resources unavailable"))?;
        Ok(Self { runs })
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Self::Error> {
        Ok(())
    }
    fn resources(&self) -> Arc<Runs> {
        self.runs.clone()
    }
    fn bind_operations(&self, table: &mut WireTable) -> Result<(), harmos_runtime::operations::ServingError> {
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .implementation::<RemoteFetchDeclaration, RemoteFetching>(),
        )
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Handler for RemoteFetching {
    type Request = <RemoteFetchDeclaration as harmos_runtime::OperationDeclaration>::Request;
    type Response = <RemoteFetchDeclaration as harmos_runtime::OperationDeclaration>::Response;

    type Error = Refusal;
    type StreamEndpoints = ();
    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Refusal> {
        let runs = context.resource::<Runs>().expect("fixture dependency");
        Ok(Self { runs })
    }
    async fn run(
        &mut self, _: harmos_runtime::OperationContext, request: &mut RemoteFetchDeclaration, _: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<Fetch>, Refusal> {
        self.runs.0.fetch_add(1, Ordering::SeqCst);
        Ok(Fetch { value: request.value + 1 }.into())
    }
}

impl Handler for LocalFetching {
    type Request = <LocalFetchDeclaration as harmos_runtime::OperationDeclaration>::Request;
    type Response = <LocalFetchDeclaration as harmos_runtime::OperationDeclaration>::Response;

    type Error = harmos_runtime::Error;
    type StreamEndpoints = ();
    async fn initialize(context: harmos_runtime::OperationContext) -> harmos_runtime::Result<Self> {
        let runs = context.resource::<Runs>().expect("fixture dependency");
        Ok(Self { runs })
    }
    async fn run(
        &mut self, _: harmos_runtime::OperationContext, request: &mut LocalFetchDeclaration, _: &mut Self::StreamEndpoints,
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<Fetch>> {
        self.runs.0.fetch_add(1, Ordering::SeqCst);
        Ok(Fetch { value: request.value + 1 }.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

// Value 20 needs the held scope, 21 one nobody holds, 22 the held scope on
// another operation, anything else no grant.
fn scoped(operation: &'static str, value: u32) -> Option<Grant> {
    match value {
        20 => Some(Grant::of(operation, ALLOWED)),
        21 => Some(Grant::of(operation, REFUSED)),
        22 => Some(Grant::of("gated.elsewhere", ALLOWED)),
        _ => None,
    }
}

async fn fetch(channel: tonic::transport::Channel, bearer: &'static str, path: &'static str, value: u32) -> Result<u32, tonic::Status> {
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.map_err(|_| tonic::Status::unavailable("channel"))?;
    let mut request = tonic::Request::new(harmos_runtime::operations::Response::new(Fetch { value }));
    request.metadata_mut().insert("authorization", bearer.parse().unwrap());
    let settings = harmos_runtime::grpc::server::Settings::default();
    let codec = harmos_runtime::grpc::Codec::<harmos_runtime::grpc::Transcoded, Fetch, harmos_runtime::operations::Response<Fetch>>::new(
        settings.decode,
        settings.encode,
    )?;
    let response = client.unary(request, tonic::codegen::http::uri::PathAndQuery::from_static(path), codec).await?;
    Ok(response.into_inner().value)
}

// Each binding refuses a caller lacking the request's grant before it runs.
async fn enforced(channel: &tonic::transport::Channel, path: &'static str, runs: &Runs) {
    let denied = |result: Result<u32, tonic::Status>| {
        assert_eq!(result.unwrap_err().code(), tonic::Code::PermissionDenied);
    };
    assert_eq!(fetch(channel.clone(), "Bearer lacking", path, 1).await.unwrap(), 2);
    denied(fetch(channel.clone(), "Bearer lacking", path, 20).await);
    denied(fetch(channel.clone(), "Bearer lacking", path, 21).await);
    assert_eq!(runs.0.load(Ordering::SeqCst), 1, "refusals never run");
    assert_eq!(fetch(channel.clone(), "Bearer holder", path, 20).await.unwrap(), 21);
    denied(fetch(channel.clone(), "Bearer holder", path, 21).await);
    // A grant is permission to call one operation: the held scope does not
    // carry over to a grant the request resolves on another operation.
    denied(fetch(channel.clone(), "Bearer holder", path, 22).await);
    assert_eq!(runs.0.load(Ordering::SeqCst), 2, "refusals never run");
    assert_eq!(fetch(channel.clone(), "Bearer full", path, 20).await.unwrap(), 21);
    assert_eq!(fetch(channel.clone(), "Bearer full", path, 21).await.unwrap(), 22);
    denied(fetch(channel.clone(), "Bearer full", path, 22).await);
    assert_eq!(runs.0.load(Ordering::SeqCst), 4, "full control still refuses a foreign-operation grant");
}

#[tokio::test]
async fn external_request_grants() {
    let remote = Arc::new(Runs::default());
    let local = Arc::new(Runs::default());
    let seed = Seed::new(&remote);
    let capacity = std::num::NonZeroUsize::new(2).unwrap();
    let source = harmos_sidecar::PluginNative::<Provider>::new(capacity).unwrap();
    let mut fixture =
        Managed::attach(source, serde_json::to_vec(&seed.id).unwrap(), Lanes::Open, capacity, vec![], std::convert::identity).await;
    fixture.initialize().await.unwrap();
    fixture
        .runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(local.clone()).unwrap();
                dependencies
            })
            .implementation::<LocalFetchDeclaration, LocalFetching>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    fixture.runtime.operations.register_authorizer(Arc::new(Holdings)).unwrap();
    fixture.runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Bearers)))).unwrap();
    let settings = harmos_runtime::grpc::Settings {
        address: "127.0.0.1:0".parse().unwrap(),
        operations: harmos_runtime::grpc::server::Settings::default(),
        reflection: false,
        web: false,
        allowed_origins: Vec::new(),
        tls: None,
        connections: std::num::NonZeroUsize::new(4).unwrap(),
        calls_per_connection: std::num::NonZeroUsize::new(8).unwrap(),
        request_timeout: Duration::from_secs(2),
        shutdown_timeout: Duration::from_secs(2),
    };
    let attached = fixture.runtime.grpc().attach(settings).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    enforced(&channel, "/fixture.gated.v1.Remote/Fetch", &remote).await;
    enforced(&channel, "/fixture.gated.v1.Local/Fetch", &local).await;
    drop(channel);
    attached.detach().await.unwrap();
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

/* === Tests === */
