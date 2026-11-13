/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "grpc_network.rs"]
mod grpc_network;

/* === Imports === */

// Standard Library Imports
use std::{
    convert::Infallible,
    future::Future,
    num::NonZeroUsize,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context as TaskContext, Poll},
    time::Duration,
};

// Third Party Imports
use tonic::{
    body::Body,
    codegen::{Service, http},
    server::NamedService,
};

// Local Imports
use super::app::App;
use harmos_runtime::Error;
use harmos_runtime::grpc::authentication::{Authenticate, Authentication, Authenticator};
use harmos_runtime::grpc::middleware::{Enter, Invocation, Middleware, Observation};
use harmos_runtime::grpc::{Codec, DecodeSettings, EncodeSettings};
use harmos_runtime::operations::{OperationPolicy, RetryPolicy, StreamSettings};

/* === Constants === */

const WHO_POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::Never,
};
// A google.rpc.Status With One Typed Domain Rejection In An Any Envelope.
const REJECTION_DETAILS: &[u8] = &[
    8, 5, 18, 13, 110, 111, 32, 115, 117, 99, 104, 32, 97, 99, 116, 111, 114, 26, 49, 10, 37, 116, 121, 112, 101, 46, 103, 111, 111, 103,
    108, 101, 97, 112, 105, 115, 46, 99, 111, 109, 47, 102, 105, 120, 116, 117, 114, 101, 46, 82, 101, 106, 101, 99, 116, 105, 111, 110,
    18, 8, 10, 6, 97, 98, 115, 101, 110, 116,
];

/* === Structs === */

struct Principal(String);
#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Reply)]
#[derive(Clone)]
struct GuardedDeclaration {}

static GUARDED_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<GuardedDeclaration>("grpc.guarded", WHO_POLICY)
        .visibility(harmos_runtime::operations::Visibility::Public);

const GUARDED_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<GuardedDeclaration> =
    harmos_runtime::operations::OperationSpec::<GuardedDeclaration>::new(&GUARDED_DECLARATION_DESCRIPTION)
        .grant(|_: &GuardedDeclaration| Some(harmos_operations::plugin::Grant::of(GUARDED_DECLARATION_OPERATION.entry().id, "reading")));

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Reply)]
#[derive(Clone)]
struct DescendantDeclaration {}

static DESCENDANT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<DescendantDeclaration>("grpc.descendant", WHO_POLICY)
        .visibility(harmos_runtime::operations::Visibility::Public);

const DESCENDANT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<DescendantDeclaration> =
    harmos_runtime::operations::OperationSpec::<DescendantDeclaration>::new(&DESCENDANT_DECLARATION_DESCRIPTION);

struct ExternalAccess {
    operations: std::sync::Weak<harmos_runtime::operations::OperationManager>,
    mode: Arc<std::sync::atomic::AtomicUsize>,
}
struct Sessions;
struct PendingAuthentication(Arc<tokio::sync::Notify>);
struct DiscoveryAccess;
#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Reply)]
#[derive(Clone)]
struct WhoDeclaration {}

static WHO_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<WhoDeclaration>("grpc.who", WHO_POLICY)
        .visibility(harmos_runtime::operations::Visibility::Public);

const WHO_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<WhoDeclaration> =
    harmos_runtime::operations::OperationSpec::<WhoDeclaration>::new(&WHO_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Reply, output = Reply)]
#[derive(Clone)]
struct DownloadDeclaration {}

static DOWNLOAD_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<DownloadDeclaration>("grpc.download", WHO_POLICY)
        .visibility(harmos_runtime::operations::Visibility::Public);

const DOWNLOAD_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<DownloadDeclaration> =
    harmos_runtime::operations::OperationSpec::<DownloadDeclaration>::new(&DOWNLOAD_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Reply, input = Reply)]
#[derive(Clone)]
struct UploadDeclaration {}

static UPLOAD_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<UploadDeclaration>("grpc.upload", WHO_POLICY)
        .visibility(harmos_runtime::operations::Visibility::Public);

const UPLOAD_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<UploadDeclaration> =
    harmos_runtime::operations::OperationSpec::<UploadDeclaration>::new(&UPLOAD_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Reply, input = Reply, output = Reply)]
#[derive(Clone)]
struct ExchangeDeclaration {}

static EXCHANGE_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<ExchangeDeclaration>("grpc.exchange", WHO_POLICY)
        .visibility(harmos_runtime::operations::Visibility::Public);

const EXCHANGE_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<ExchangeDeclaration> =
    harmos_runtime::operations::OperationSpec::<ExchangeDeclaration>::new(&EXCHANGE_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Reply, output = Reply)]
#[derive(Clone)]
struct HoldDeclaration {}

static HOLD_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<HoldDeclaration>("grpc.hold", WHO_POLICY)
        .visibility(harmos_runtime::operations::Visibility::Public);

const HOLD_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<HoldDeclaration> =
    harmos_runtime::operations::OperationSpec::<HoldDeclaration>::new(&HOLD_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Clone, Debug, Default)]
struct Reply {
    actor: String,
}
#[derive(harmos_runtime::Message, Clone, Debug, Default)]
struct Ask {}
#[derive(Clone)]
struct Endpoint {
    authority: harmos_runtime::grpc::server::Authority,
}
struct Trace(Arc<Mutex<Vec<tonic::Code>>>);

struct HoldingGuard(Arc<Behavior>);

/* === Enums === */

#[derive(Clone, Copy)]
enum FailureBehavior {
    Reject,
    Crash,
}

struct Behavior {
    failure: Option<FailureBehavior>,
    uploading: Option<Arc<tokio::sync::Notify>>,
    stopped: Option<Arc<tokio::sync::Semaphore>>,
}

/* === Implementations === */

impl Behavior {
    fn read() -> Self {
        Self {
            failure: None,
            uploading: None,
            stopped: None,
        }
    }
    fn reject() -> Self {
        Self {
            failure: Some(FailureBehavior::Reject),
            uploading: None,
            stopped: None,
        }
    }
    fn crash() -> Self {
        Self {
            failure: Some(FailureBehavior::Crash),
            uploading: None,
            stopped: None,
        }
    }
    fn uploading(notify: Arc<tokio::sync::Notify>) -> Self {
        Self {
            failure: None,
            uploading: Some(notify),
            stopped: None,
        }
    }
    fn holding(stopped: Arc<tokio::sync::Semaphore>) -> Self {
        Self {
            failure: None,
            uploading: None,
            stopped: Some(stopped),
        }
    }
    fn holding_and_uploading(stopped: Arc<tokio::sync::Semaphore>, uploading: Arc<tokio::sync::Notify>) -> Self {
        Self {
            failure: None,
            uploading: Some(uploading),
            stopped: Some(stopped),
        }
    }
    fn check(&self, exchange: bool) -> harmos_runtime::Result<()> {
        let source: Arc<dyn std::error::Error + Send + Sync> = match self.failure {
            Some(FailureBehavior::Reject) => Arc::new(authored_rejection()),
            Some(FailureBehavior::Crash) => Arc::new(std::io::Error::other("disk detail")),
            None => return Ok(()),
        };
        let failure = Error::OperationFailed {
            operation: "fixture.response".into(),
            source,
        };
        if exchange && matches!(self.failure, Some(FailureBehavior::Reject)) {
            return Err(Error::TaskFailed {
                domain: "fixture response",
                source: Arc::new(failure),
            });
        }
        Err(failure)
    }
}
impl Drop for HoldingGuard {
    fn drop(&mut self) {
        if let Some(stopped) = &self.0.stopped {
            stopped.add_permits(1);
        }
    }
}

impl Authenticator for Sessions {
    type Principal = Principal;
    fn authenticate<'a>(&'a self, call: &'a Invocation) -> Authentication<'a, Principal> {
        Box::pin(async move {
            tokio::task::yield_now().await;
            match call.metadata().get("authorization").and_then(|value| value.to_str().ok()) {
                Some("Bearer alice") => Ok(Principal("alice".into())),
                Some("Bearer bob") => Ok(Principal("bob".into())),
                Some("Bearer limited") => Ok(Principal("limited".into())),
                Some("Bearer carol") => Ok(Principal("carol".into())),
                _ => Err(Error::Unauthenticated),
            }
        })
    }
}
impl Middleware for DiscoveryAccess {
    fn enter<'a>(&'a self, call: &'a mut Invocation) -> Enter<'a> {
        Box::pin(async move {
            let principal = call.context().require::<Principal>()?;
            if principal.0 == "bob" && !call.path().starts_with("/grpc.reflection.v1.ServerReflection/") {
                return Err(Error::PermissionDenied);
            }
            Ok(())
        })
    }
}
impl Authenticator for PendingAuthentication {
    type Principal = Principal;
    fn authenticate<'a>(&'a self, _: &'a Invocation) -> Authentication<'a, Principal> {
        Box::pin(async move {
            self.0.notify_one();
            std::future::pending().await
        })
    }
}
impl Middleware for Trace {
    fn enter<'a>(&'a self, _: &'a mut Invocation) -> Enter<'a> {
        Box::pin(async { Ok(()) })
    }
    fn observe(&self, _: &Invocation, observation: Observation) {
        self.0.lock().unwrap().push(observation.code);
    }
}
async fn who_declaration_handler(
    _request: WhoDeclaration, ctx: harmos_runtime::operations::FunctionContext<WhoDeclaration>,
) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
    let behavior = ctx.resource::<Behavior>().expect("declared resource is registered");
    behavior.check(false)?;
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let result: ::core::result::Result<Reply, Error> = async move {
        Ok(Reply {
            actor: context.context().require::<Principal>()?.0.clone(),
        })
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn who_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        WHO_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Behavior>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        who_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}
impl harmos_runtime::operations::authorization::Authorizer for ExternalAccess {
    fn authorize(
        &self, context: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Authorization>> {
        use harmos_operations::plugin::Grant;
        use harmos_runtime::operations::authorization::{Authorization, Required};
        // Reentrant Read Proves The Callback Does Not Hold The Original Registry Lock.
        assert_eq!(self.operations.upgrade().unwrap().owner(provider.definition.id.as_str())?, *provider.owner);
        assert!(provider.generation.value() > 0);
        assert!(!provider.plugin, "host registration cannot impersonate a plugin provider");
        let Some(principal) = context.get::<Principal>() else {
            return Ok(None);
        };
        let mut grants = Vec::new();
        match provider.definition.id.as_str() {
            "harmos.v1.CatalogEntries/List" | "grpc.descendant" => (),
            "grpc.guarded" => {
                match self.mode.load(std::sync::atomic::Ordering::Acquire) {
                    1 => {
                        return Ok(Some(Authorization::Restricted(Required {
                            contract: DESCENDANT_DECLARATION_OPERATION.wire().contract().unwrap(),
                            grants,
                        })));
                    }
                    2 => panic!("authorizer panic fixture"),
                    3 => {
                        let contract = DESCENDANT_DECLARATION_OPERATION.wire().contract().unwrap();
                        return Ok(Some(Authorization::Full(contract)));
                    }
                    4 => return Ok(Some(Authorization::Full(provider.contract.clone()))),
                    _ => (),
                }
                match principal.0.as_str() {
                    "alice" | "carol" => grants.push(Grant::of("grpc.guarded", "reading")),
                    "limited" => (),
                    _ => return Ok(None),
                }
            }
            _ => return Ok(None),
        }
        Ok(Some(Authorization::Restricted(Required {
            contract: provider.contract.clone(),
            grants,
        })))
    }
}

async fn guarded_declaration_handler(
    _request: GuardedDeclaration, context: harmos_runtime::operations::FunctionContext<GuardedDeclaration>,
) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
    let host = context.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let result: ::core::result::Result<Reply, Error> = async move {
        let principal = context.context().get::<Principal>().map_or("anonymous", |value| value.0.as_str());
        Ok(Reply {
            actor: format!("{}:{principal}", if context.is_host_call() { "host" } else { "external" }),
        })
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn guarded_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        GUARDED_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        guarded_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}
async fn descendant_declaration_handler(
    _request: DescendantDeclaration, context: harmos_runtime::operations::FunctionContext<DescendantDeclaration>,
) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
    let host = context.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let result: ::core::result::Result<Reply, Error> = async move {
        let response = context.operation::<GuardedDeclaration>(GuardedDeclaration {}).await?;
        Ok(Reply { actor: response.actor.clone() })
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn descendant_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        DESCENDANT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        descendant_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

impl NamedService for Endpoint {
    const NAME: &'static str = "fixture.Identity";
}
impl Service<http::Request<Body>> for Endpoint {
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Infallible>> + Send>>;
    fn poll_ready(&mut self, _: &mut TaskContext<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let authority = self.authority.clone();
        Box::pin(async move {
            Ok(match request.uri().path() {
                "/fixture.Identity/Hold" => {
                    harmos_runtime::grpc::server::download::<HoldDeclaration, harmos_runtime::grpc::Transcoded>(
                        authority,
                        wire_settings(),
                        request,
                    )
                    .await
                }
                "/fixture.Identity/Download" => {
                    harmos_runtime::grpc::server::download::<DownloadDeclaration, harmos_runtime::grpc::Transcoded>(
                        authority,
                        wire_settings(),
                        request,
                    )
                    .await
                }
                "/fixture.Identity/Upload" => {
                    harmos_runtime::grpc::server::upload::<UploadDeclaration, harmos_runtime::grpc::Transcoded>(
                        authority,
                        wire_settings(),
                        request,
                    )
                    .await
                }
                "/fixture.Identity/Exchange" => {
                    harmos_runtime::grpc::server::exchange::<ExchangeDeclaration, harmos_runtime::grpc::Transcoded>(
                        authority,
                        wire_settings(),
                        request,
                    )
                    .await
                }
                "/fixture.Identity/Guarded" => {
                    harmos_runtime::grpc::server::unary::<GuardedDeclaration, harmos_runtime::grpc::Transcoded>(
                        authority,
                        wire_settings(),
                        request,
                    )
                    .await
                }
                "/fixture.Identity/Descendant" => {
                    harmos_runtime::grpc::server::unary::<DescendantDeclaration, harmos_runtime::grpc::Transcoded>(
                        authority,
                        wire_settings(),
                        request,
                    )
                    .await
                }
                _ => {
                    harmos_runtime::grpc::server::unary::<WhoDeclaration, harmos_runtime::grpc::Transcoded>(
                        authority,
                        wire_settings(),
                        request,
                    )
                    .await
                }
            })
        })
    }
}

async fn download_declaration_handler(
    _request: DownloadDeclaration, ctx: harmos_runtime::operations::FunctionContext<DownloadDeclaration>,
) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
    let behavior = ctx.resource::<Behavior>().expect("declared resource is registered");
    behavior.check(false)?;
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let output = ctx.streams();
    let result: ::core::result::Result<Reply, Error> = async move {
        let actor = context.context().require::<Principal>()?.0.clone();
        output.send_with(128, || Ok(Reply { actor: actor.clone() })).await?;
        Ok(Reply { actor })
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn download_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        DOWNLOAD_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Behavior>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        download_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}
async fn upload_declaration_handler(
    _request: UploadDeclaration, ctx: harmos_runtime::operations::FunctionContext<UploadDeclaration>,
) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
    let behavior = ctx.resource::<Behavior>().expect("declared resource is registered");
    behavior.check(false)?;
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let input = ctx.streams();
    let result: ::core::result::Result<Reply, Error> = async move {
        if let Some(started) = &behavior.uploading {
            started.notify_one();
        }
        while input.recv().await.is_some() {}
        Ok(Reply {
            actor: context.context().require::<Principal>()?.0.clone(),
        })
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn upload_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        UPLOAD_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Behavior>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        upload_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}
async fn exchange_declaration_handler(
    _request: ExchangeDeclaration, ctx: harmos_runtime::operations::FunctionContext<ExchangeDeclaration>,
) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
    let behavior = ctx.resource::<Behavior>().expect("declared resource is registered");
    behavior.check(true)?;
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let endpoints = ctx.streams();
    let result: ::core::result::Result<Reply, Error> = async move {
        let actor = context.context().require::<Principal>()?.0.clone();
        while endpoints.0.recv().await.is_some() {
            endpoints.1.send_with(128, || Ok(Reply { actor: actor.clone() })).await?;
        }
        Ok(Reply { actor })
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn exchange_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        EXCHANGE_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Behavior>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        exchange_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

async fn hold_declaration_handler(
    _request: HoldDeclaration, ctx: harmos_runtime::operations::FunctionContext<HoldDeclaration>,
) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
    let behavior = ctx.resource::<Behavior>().expect("declared resource is registered");
    behavior.check(false)?;
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let output = ctx.streams();
    let _guard = HoldingGuard(behavior);
    let result: ::core::result::Result<Reply, Error> = async move {
        let actor = context.context().require::<Principal>()?.0.clone();
        output.send_with(128, || Ok(Reply { actor })).await?;
        std::future::pending().await
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn hold_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        HOLD_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Behavior>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        hold_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

/* === Private Functions === */

fn authored_rejection() -> tonic::Status {
    let mut metadata = tonic::metadata::MetadataMap::new();
    metadata.insert("x-fixture-rejection", "absent".parse().unwrap());
    metadata.insert_bin("x-fixture-context-bin", tonic::metadata::MetadataValue::from_bytes(&[0, 255, 1]));
    tonic::Status::with_details_and_metadata(tonic::Code::NotFound, "no such actor", bytes::Bytes::from_static(REJECTION_DETAILS), metadata)
}

fn limits() -> (DecodeSettings, EncodeSettings) {
    let bytes = NonZeroUsize::new(4096).unwrap();
    (
        DecodeSettings {
            message_bytes: bytes,
            retained_bytes: bytes,
        },
        EncodeSettings {
            message_bytes: bytes,
            buffer_bytes: bytes,
        },
    )
}
fn wire_settings() -> harmos_runtime::grpc::server::Settings {
    let (decode, encode) = limits();
    let capacity = NonZeroUsize::new(4096).unwrap();
    harmos_runtime::grpc::server::Settings {
        calls: NonZeroUsize::new(8).unwrap(),
        decode,
        encode,
        input: StreamSettings { items: capacity, bytes: capacity },
        output: StreamSettings { items: capacity, bytes: capacity },
        opening_timeout: Duration::from_secs(1),
    }
}
fn settings() -> harmos_runtime::grpc::Settings {
    harmos_runtime::grpc::Settings {
        operations: harmos_runtime::grpc::server::Settings::default(),
        reflection: false,
        address: "127.0.0.1:0".parse().unwrap(),
        tls: None,
        connections: NonZeroUsize::new(4).unwrap(),
        calls_per_connection: NonZeroUsize::new(8).unwrap(),
        request_timeout: Duration::from_secs(2),
        shutdown_timeout: Duration::from_secs(2),
    }
}
async fn call(channel: tonic::transport::Channel, token: Option<&'static str>) -> Result<String, tonic::Status> {
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.map_err(|_| tonic::Status::unavailable("channel"))?;
    let mut request = tonic::Request::new(harmos_runtime::operations::Response::new(Ask {}));
    if let Some(token) = token {
        request.metadata_mut().insert("authorization", token.parse().unwrap());
    }
    let (decode, encode) = limits();
    let response = client
        .unary(
            request,
            http::uri::PathAndQuery::from_static("/fixture.Identity/Who"),
            Codec::<harmos_runtime::grpc::Transcoded, Reply, harmos_runtime::operations::Response<Ask>>::new(decode, encode)?,
        )
        .await?;
    Ok(response.into_inner().actor.clone())
}

async fn external_call(
    channel: tonic::transport::Channel, token: Option<&'static str>, path: &'static str,
) -> Result<String, tonic::Status> {
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.map_err(|_| tonic::Status::unavailable("channel"))?;
    let mut request = tonic::Request::new(harmos_runtime::operations::Response::new(Ask {}));
    if let Some(token) = token {
        request.metadata_mut().insert("authorization", token.parse().unwrap());
    }
    let (decode, encode) = limits();
    let response = client
        .unary(
            request,
            http::uri::PathAndQuery::from_static(path),
            Codec::<harmos_runtime::grpc::Transcoded, Reply, harmos_runtime::operations::Response<Ask>>::new(decode, encode)?,
        )
        .await?;
    Ok(response.into_inner().actor.clone())
}

async fn external_catalog(
    channel: tonic::transport::Channel, token: Option<&'static str>,
) -> Result<Vec<harmos_operations::host::DiscoveredOperation>, tonic::Status> {
    use harmos_protocol::v1::{CatalogEntriesListRequest, CatalogEntriesListResponse, CatalogKind};
    let (decode, encode) = limits();
    let mut client = tonic::client::Grpc::new(channel)
        .max_decoding_message_size(decode.message_bytes.get())
        .max_encoding_message_size(encode.message_bytes.get());
    let mut entries = Vec::new();
    let mut after = None;
    // Bounded Pages Fit The Client's Decode Ceiling; The Catalog Ends With An Empty Cursor.
    loop {
        client.ready().await.map_err(|_| tonic::Status::unavailable("channel"))?;
        let page = CatalogEntriesListRequest {
            kind: CatalogKind::Operations as i32,
            schema: None,
            page: Some(harmos_protocol::v1::PageRequest {
                page_size: 64,
                max_bytes: decode.message_bytes.get() as u64,
                page_token: after.take().unwrap_or_default(),
            }),
        };
        let mut request = tonic::Request::new(page);
        if let Some(token) = token {
            request.metadata_mut().insert("authorization", token.parse().unwrap());
        }
        let response: tonic::Response<CatalogEntriesListResponse> = client
            .unary(request, http::uri::PathAndQuery::from_static("/harmos.v1.CatalogEntries/List"), tonic_prost::ProstCodec::default())
            .await?;
        let page = response.into_inner();
        assert!(prost::Message::encoded_len(&page) <= decode.message_bytes.get());
        entries.extend(page.operations);
        if page.next_page_token.is_empty() {
            return Ok(entries);
        }
        after = Some(page.next_page_token);
    }
}

async fn failures<F: Future<Output = ()>>(
    behavior: Behavior, register: impl FnOnce(harmos_runtime::operations::OperationManager) -> F,
) -> [tonic::Status; 4] {
    use harmos_runtime::grpc::{InputEnvelope, OutputEnvelope};
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource(behavior);
    })
    .await
    .unwrap();
    register(runtime.operations.clone()).await;
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint {
            authority: harmos_runtime::grpc::server::Authority::trusted_host(&runtime.operations),
        })
        .unwrap();
    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    let unary = call(channel.clone(), Some("Bearer alice")).await.unwrap_err();
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();
    let (decode, encode) = limits();
    let mut request = tonic::Request::new(harmos_runtime::operations::Response::new(Ask {}));
    request.metadata_mut().insert("authorization", "Bearer alice".parse().unwrap());
    let streaming = match client
        .server_streaming(
            request,
            http::uri::PathAndQuery::from_static("/fixture.Identity/Download"),
            Codec::<harmos_runtime::grpc::Transcoded, OutputEnvelope<Reply, Reply>, harmos_runtime::operations::Response<Ask>>::new(
                decode, encode,
            )
            .unwrap(),
        )
        .await
    {
        Ok(response) => response.into_inner().message().await.err().expect("stream fails"),
        Err(status) => status,
    };
    client.ready().await.unwrap();
    let mut request = tonic::Request::new(futures_util::stream::iter([harmos_runtime::operations::Response::new(InputEnvelope::<
        Ask,
        Reply,
    >::Request(Ask {}))]));
    request.metadata_mut().insert("authorization", "Bearer alice".parse().unwrap());
    let upload = client
        .client_streaming(
            request,
            http::uri::PathAndQuery::from_static("/fixture.Identity/Upload"),
            Codec::<harmos_runtime::grpc::Transcoded, Reply, harmos_runtime::operations::Response<InputEnvelope<Ask, Reply>>>::new(
                decode, encode,
            )
            .unwrap(),
        )
        .await
        .err()
        .expect("upload fails");
    client.ready().await.unwrap();
    let mut request = tonic::Request::new(futures_util::stream::iter([harmos_runtime::operations::Response::new(InputEnvelope::<
        Ask,
        Reply,
    >::Request(Ask {}))]));
    request.metadata_mut().insert("authorization", "Bearer alice".parse().unwrap());
    let exchange = match client
        .streaming(
            request,
            http::uri::PathAndQuery::from_static("/fixture.Identity/Exchange"),
            Codec::<
                harmos_runtime::grpc::Transcoded,
                OutputEnvelope<Reply, Reply>,
                harmos_runtime::operations::Response<InputEnvelope<Ask, Reply>>,
            >::new(decode, encode)
            .unwrap(),
        )
        .await
    {
        Ok(response) => response.into_inner().message().await.err().expect("exchange fails"),
        Err(status) => status,
    };
    drop(attached);
    crate::support::app::shutdown(&runtime).await.unwrap();
    [unary, streaming, upload, exchange]
}
