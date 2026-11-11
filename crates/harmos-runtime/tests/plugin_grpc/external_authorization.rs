/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// External Client Authorization Remains Enforced

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos_domain::plugin::Grant;
use harmos_runtime::grpc::authentication::{Authenticate, Authentication, Authenticator};
use harmos_runtime::grpc::middleware::Invocation;
use harmos_runtime::operations::{OperationPolicy, RetryPolicy};

/* === Constants === */

const ALLOWED: &str = "fixture.remote.allowed";
const REFUSED: &str = "fixture.remote.refused";
const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Runs(AtomicUsize);

struct Principal(&'static str);

struct Bearers;

// Selects The Scoped Grants Each Bearer Holds On The Called Handler; Unknown
// Bearers Are Denied.
struct Holdings;

#[harmos_runtime::message]
#[derive(Clone)]
struct Fetch {
    value: u32,
}

#[harmos_runtime::operation(resources = [Runs], id = "gated.local",
    policy = POLICY,
    response = Fetch,
    visibility = Public,
    grpc(package = "fixture.gated.v1", service = "Local", method = "Fetch"),
    grant = |request: &LocalFetchDeclaration| scoped("gated.local", request.value),
)]
#[derive(Clone)]
struct LocalFetchDeclaration {
    value: u32,
}

/* === Enums === */

#[harmos_runtime::catalog]
enum LocalOperations {
    Fetch(LocalFetchDeclaration),
}

/* === Traits === */

/* === Implementations === */

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
        assert!(!provider.plugin);
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

impl harmos_runtime::Operation for LocalFetchDeclaration {
    type Error = harmos_runtime::Error;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::operations::Response<Fetch>> {
        let runs = ctx.resource::<Runs>();
        runs.0.fetch_add(1, Ordering::SeqCst);
        Ok(Fetch { value: self.value + 1 }.into())
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

#[tokio::test]
async fn external_request_grants() {
    // Register A Public Host Operation And The Product Authorization Hook
    let local = Arc::new(Runs::default());
    let (clock, _) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let runtime = crate::support::booted(Default::default(), |setup| {
        setup.settings().clock(clock);
        setup.resource::<Runs>(local.clone());
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        });
    })
    .await;
    runtime.operations.register::<LocalOperations>(crate::support::Lanes::Open).await.unwrap();
    runtime.operations.register_authorizer(Arc::new(Holdings)).unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Bearers)))).unwrap();

    // Authenticate Real gRPC Requests Before Authorizing Their Request Scope
    let settings = harmos_runtime::grpc::Settings {
        address: "127.0.0.1:0".parse().unwrap(),
        operations: harmos_runtime::grpc::server::Settings::default(),
        reflection: false,
        tls: None,
        connections: std::num::NonZeroUsize::new(4).unwrap(),
        calls_per_connection: std::num::NonZeroUsize::new(8).unwrap(),
        request_timeout: Duration::from_secs(2),
        shutdown_timeout: Duration::from_secs(2),
    };
    let attached = runtime.grpc().attach(settings).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    let denied = |result: Result<u32, tonic::Status>| {
        assert_eq!(result.unwrap_err().code(), tonic::Code::PermissionDenied);
    };
    assert_eq!(fetch(channel.clone(), "Bearer lacking", "/fixture.gated.v1.Local/Fetch", 1).await.unwrap(), 2);
    denied(fetch(channel.clone(), "Bearer lacking", "/fixture.gated.v1.Local/Fetch", 20).await);
    denied(fetch(channel.clone(), "Bearer lacking", "/fixture.gated.v1.Local/Fetch", 21).await);
    assert_eq!(local.0.load(Ordering::SeqCst), 1, "refusals never run");
    assert_eq!(fetch(channel.clone(), "Bearer holder", "/fixture.gated.v1.Local/Fetch", 20).await.unwrap(), 21);
    denied(fetch(channel.clone(), "Bearer holder", "/fixture.gated.v1.Local/Fetch", 21).await);
    // A grant is permission to call one operation: the held scope does not
    // carry over to a grant the request resolves on another operation.
    denied(fetch(channel.clone(), "Bearer holder", "/fixture.gated.v1.Local/Fetch", 22).await);
    assert_eq!(local.0.load(Ordering::SeqCst), 2, "refusals never run");
    assert_eq!(fetch(channel.clone(), "Bearer full", "/fixture.gated.v1.Local/Fetch", 20).await.unwrap(), 21);
    assert_eq!(fetch(channel.clone(), "Bearer full", "/fixture.gated.v1.Local/Fetch", 21).await.unwrap(), 22);
    denied(fetch(channel.clone(), "Bearer full", "/fixture.gated.v1.Local/Fetch", 22).await);
    assert_eq!(local.0.load(Ordering::SeqCst), 4, "full control still refuses a foreign-operation grant");

    // Shut Down The External Interface And Host
    drop(channel);
    attached.detach().await.unwrap();
    runtime.shutdown().await.unwrap();
}

/* === Tests === */
