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
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::operations::{Lanes, Managed};
use harmos::grpc::authentication::{Authenticate, Authentication, Authenticator};
use harmos::grpc::middleware::Invocation;
use harmos::operations::{
    Operation, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, Visibility,
    WireOperation, WireTable,
};
use harmos_contract::plugin::Grant;
use harmos_sidecar::{Refusal, Supervisor};

/* === Constants === */
const ALLOWED: &str = "fixture.remote.allowed";
const REFUSED: &str = "fixture.remote.refused";
const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};
static NEXT_SEED: AtomicUsize = AtomicUsize::new(1);
static SEEDS: Mutex<std::collections::BTreeMap<usize, Weak<Runs>>> =
    Mutex::new(std::collections::BTreeMap::new());

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

// Selects The Egress Grants Each Bearer Holds; Unknown Bearers Are Denied.
struct Holdings;

struct Provider {
    runs: Arc<Runs>,
}

#[harmos::message]
#[derive(Clone)]
struct Fetch {
    value: u32,
}

struct RemoteFetch;

struct RemoteFetching {
    runs: Arc<Runs>,
}

struct LocalFetch;

struct LocalFetching {
    runs: Arc<Runs>,
}

/* === Enums === */
#[harmos::catalog]
enum Gated {
    Fetch(RemoteFetch),
}

/* === Traits === */

/* === Implementations === */
impl Seed {
    fn new(runs: &Arc<Runs>) -> Self {
        let id = NEXT_SEED
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |id| id.checked_add(1))
            .unwrap();
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
            match call
                .metadata()
                .get("authorization")
                .and_then(|value| value.to_str().ok())
            {
                Some("Bearer holder") => Ok(Principal("holder")),
                Some("Bearer lacking") => Ok(Principal("lacking")),
                _ => Err(harmos::Error::Unauthenticated),
            }
        })
    }
}

impl harmos::operations::authorization::Authorizer for Holdings {
    fn required(
        &self,
        context: &harmos::operations::CallContext,
        provider: harmos::operations::authorization::Provider<'_>,
    ) -> harmos::Result<Option<harmos::operations::authorization::Required>> {
        let grants = match context.get::<Principal>().map(|principal| principal.0) {
            Some("holder") => vec![Grant::Egress(ALLOWED.into())],
            Some("lacking") => vec![],
            _ => return Ok(None),
        };
        Ok(Some(harmos::operations::authorization::Required {
            contract: provider.contract.clone(),
            grants,
        }))
    }
}

impl harmos::operations::lifecycle::Policy for Provider {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos::plugin(id = "gated", provides = Gated, requires = ())]
impl harmos::Plugin for Provider {
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
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos::operations::ServingError> {
        table.bind_operation::<RemoteFetching>(self.resources())
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl OperationDeclaration for RemoteFetch {
    type Request = Fetch;
    type Response = Fetch;
    const ID: &'static str = "gated.remote";
    const VISIBILITY: Visibility = Visibility::Host;
    const POLICY: OperationPolicy = POLICY;
    fn visit_wire<V: harmos::operations::WireVisitor>(visitor: V) -> Option<V::Entry> {
        Some(visitor.unary::<Self>())
    }
    fn grant(request: &Fetch) -> Option<Grant> {
        egress(request)
    }
}

impl WireOperation for RemoteFetch {
    const VERSION: u32 = 1;
    const GRPC: Option<harmos::operations::Grpc> = Some(harmos::operations::Grpc::new(
        "fixture.gated.v1",
        "Remote",
        "Fetch",
    ));
    fn visit_grpc<V: harmos::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
        Some(visitor.unary::<Self>())
    }
}

impl Operation for RemoteFetching {
    type Declaration = RemoteFetch;
    type Resources = Runs;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(runs: Arc<Runs>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { runs })
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut Fetch,
        _: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Fetch>, Refusal> {
        self.runs.0.fetch_add(1, Ordering::SeqCst);
        Ok(Fetch {
            value: request.value + 1,
        }
        .into())
    }
}

impl OperationDeclaration for LocalFetch {
    type Request = Fetch;
    type Response = Fetch;
    const ID: &'static str = "gated.local";
    const VISIBILITY: Visibility = Visibility::Host;
    const POLICY: OperationPolicy = POLICY;
    fn visit_wire<V: harmos::operations::WireVisitor>(visitor: V) -> Option<V::Entry> {
        Some(visitor.unary::<Self>())
    }
    fn grant(request: &Fetch) -> Option<Grant> {
        egress(request)
    }
}

impl WireOperation for LocalFetch {
    const VERSION: u32 = 1;
    const GRPC: Option<harmos::operations::Grpc> = Some(harmos::operations::Grpc::new(
        "fixture.gated.v1",
        "Local",
        "Fetch",
    ));
    fn visit_grpc<V: harmos::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
        Some(visitor.unary::<Self>())
    }
}

impl Operation for LocalFetching {
    type Declaration = LocalFetch;
    type Resources = Runs;
    type Error = harmos::Error;
    type Endpoints = ();
    type Context<'a> = harmos::operations::Context<'a>;
    async fn initialize<'ctx>(runs: Arc<Runs>, _: Self::Context<'ctx>) -> harmos::Result<Self> {
        Ok(Self { runs })
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut Fetch,
        _: &mut Self::Endpoints,
    ) -> harmos::Result<harmos::operations::Response<Fetch>> {
        self.runs.0.fetch_add(1, Ordering::SeqCst);
        Ok(Fetch {
            value: request.value + 1,
        }
        .into())
    }
}

/* === Public Functions === */

/* === Private Functions === */
// Value 20 needs the held destination, 21 one nobody holds, anything else none.
fn egress(request: &Fetch) -> Option<Grant> {
    match request.value {
        20 => Some(Grant::Egress(ALLOWED.into())),
        21 => Some(Grant::Egress(REFUSED.into())),
        _ => None,
    }
}

async fn fetch(
    channel: tonic::transport::Channel,
    bearer: &'static str,
    path: &'static str,
    value: u32,
) -> Result<u32, tonic::Status> {
    let mut client = tonic::client::Grpc::new(channel);
    client
        .ready()
        .await
        .map_err(|_| tonic::Status::unavailable("channel"))?;
    let mut request = tonic::Request::new(harmos::operations::Response::new(Fetch { value }));
    request
        .metadata_mut()
        .insert("authorization", bearer.parse().unwrap());
    let settings = harmos::grpc::server::Settings::default();
    let codec = harmos::grpc::Codec::<Fetch, harmos::operations::Response<Fetch>>::new(
        settings.decode,
        settings.encode,
    )?;
    let response = client
        .unary(
            request,
            tonic::codegen::http::uri::PathAndQuery::from_static(path),
            codec,
        )
        .await?;
    Ok(response.into_inner().value)
}

// Each binding refuses a caller lacking the request's grant before it runs.
async fn enforced(channel: &tonic::transport::Channel, path: &'static str, runs: &Runs) {
    let denied = |result: Result<u32, tonic::Status>| {
        assert_eq!(result.unwrap_err().code(), tonic::Code::PermissionDenied);
    };
    assert_eq!(
        fetch(channel.clone(), "Bearer lacking", path, 1)
            .await
            .unwrap(),
        2
    );
    denied(fetch(channel.clone(), "Bearer lacking", path, 20).await);
    denied(fetch(channel.clone(), "Bearer lacking", path, 21).await);
    assert_eq!(runs.0.load(Ordering::SeqCst), 1, "refusals never run");
    assert_eq!(
        fetch(channel.clone(), "Bearer holder", path, 20)
            .await
            .unwrap(),
        21
    );
    denied(fetch(channel.clone(), "Bearer holder", path, 21).await);
    assert_eq!(runs.0.load(Ordering::SeqCst), 2, "refusals never run");
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("external_request_grants", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let remote = Arc::new(Runs::default());
                    let local = Arc::new(Runs::default());
                    let seed = Seed::new(&remote);
                    let capacity = std::num::NonZeroUsize::new(2).unwrap();
                    let source = harmos_sidecar::PluginNative::<Provider>::new(capacity).unwrap();
                    let mut fixture = Managed::attach(
                        source,
                        serde_json::to_vec(&seed.id).unwrap(),
                        Lanes::NativeOpen,
                        capacity,
                        vec![],
                        std::convert::identity,
                    )
                    .await;
                    fixture.initialize().await.unwrap();
                    fixture
                        .runtime
                        .operations
                        .register_wire_operation::<LocalFetching>(
                            "app",
                            Lanes::NativeOpen,
                            local.clone(),
                        )
                        .unwrap();
                    fixture
                        .runtime
                        .operations
                        .register_authorizer(Arc::new(Holdings))
                        .unwrap();
                    fixture
                        .runtime
                        .grpc()
                        .register_middleware(Arc::new(Authenticate::new(Arc::new(Bearers))))
                        .unwrap();
                    let settings = harmos::grpc::Settings {
                        address: "127.0.0.1:0".parse().unwrap(),
                        operations: harmos::grpc::server::Settings::default(),
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
                    let channel = tonic::transport::Endpoint::from_shared(format!(
                        "http://{}",
                        attached.local_addr()
                    ))
                    .unwrap()
                    .connect()
                    .await
                    .unwrap();
                    enforced(&channel, "/fixture.gated.v1.Remote/Fetch", &remote).await;
                    enforced(&channel, "/fixture.gated.v1.Local/Fetch", &local).await;
                    drop(channel);
                    attached.detach().await.unwrap();
                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();
                });
        });
    }
}

/* === Tests === */
