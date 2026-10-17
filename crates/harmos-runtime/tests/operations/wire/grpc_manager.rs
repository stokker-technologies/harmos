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

/* === Re-Exports === */

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
use super::{Lanes, app::App};
use harmos_runtime::grpc::authentication::{Authenticate, Authentication, Authenticator};
use harmos_runtime::grpc::middleware::{Enter, Invocation, Middleware, Observation};
use harmos_runtime::grpc::{Codec, DecodeSettings, EncodeSettings};
use harmos_runtime::operations::{Context, Operation, OperationPolicy, RestartPolicy, RetryPolicy, StreamSettings};
use harmos_runtime::{Error, Runtime};

/* === Constants === */
const WHO_POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};
// A google.rpc.Status With One Typed Domain Rejection In An Any Envelope.
const REJECTION_DETAILS: &[u8] = &[
    8, 5, 18, 13, 110, 111, 32, 115, 117, 99, 104, 32, 97, 99, 116, 111, 114, 26, 49, 10, 37, 116, 121, 112, 101, 46, 103, 111, 111, 103,
    108, 101, 97, 112, 105, 115, 46, 99, 111, 109, 47, 102, 105, 120, 116, 117, 114, 101, 46, 82, 101, 106, 101, 99, 116, 105, 111, 110,
    18, 8, 10, 6, 97, 98, 115, 101, 110, 116,
];

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Principal(String);
#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "grpc.guarded",
    version = 1,
    response = Reply,
    policy = WHO_POLICY,
    grant = |_: &Guarded| Some(harmos_domain::plugin::Grant::of(Guarded::ID, "reading")),
    key = |_: &Guarded| Some(harmos_runtime::operations::CacheKey::new(b"same-request".to_vec()).unwrap()),
)]
struct Guarded {}
struct Guard;
#[harmos_runtime::message]
#[harmos_runtime::operation(id = "grpc.descendant", version = 1, response = Reply, policy = WHO_POLICY)]
struct Descendant {}
struct Descend;
struct ExternalAccess {
    operations: std::sync::Weak<harmos_runtime::operations::OperationManager>,
    mode: Arc<std::sync::atomic::AtomicUsize>,
}
struct Sessions;
struct PendingAuthentication;
struct DiscoveryAccess;
#[harmos_runtime::message]
#[harmos_runtime::operation(id = "grpc.who", version = 1, response = Reply, policy = WHO_POLICY)]
struct Who {}
struct Read;
#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "grpc.download",
    version = 1,
    response = Reply,
    output = Reply,
    policy = WHO_POLICY
)]
struct Download {}
struct Downloading;
struct Reject;
struct Crash;
struct RejectStream;
struct CrashStream;
struct RejectUpload;
struct RejectExchange;
struct CrashUpload;
struct CrashExchange;
#[harmos_runtime::message]
#[harmos_runtime::operation(id = "grpc.upload", version = 1, response = Reply, input = Reply, policy = WHO_POLICY)]
struct Upload {}
struct Uploading {
    started: Arc<tokio::sync::Notify>,
}
#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "grpc.exchange",
    version = 1,
    response = Reply,
    input = Reply,
    output = Reply,
    policy = WHO_POLICY
)]
struct Exchange {}
struct Exchanging;
#[harmos_runtime::message]
#[harmos_runtime::operation(id = "grpc.hold", version = 1, response = Reply, output = Reply, policy = WHO_POLICY)]
struct Hold {}
struct Holding {
    stopped: Arc<tokio::sync::Semaphore>,
}
#[harmos_runtime::message]
struct Reply {
    actor: String,
}
#[harmos_runtime::message]
struct Ask {}
#[derive(Clone)]
struct Endpoint {
    authority: harmos_runtime::grpc::server::Authority,
}
struct Trace(Arc<Mutex<Vec<tonic::Code>>>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */
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
        Box::pin(std::future::pending())
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
impl Operation for Read {
    type Declaration = Who;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> harmos_runtime::Result<Self> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self, context: Self::Context<'a>, _request: &mut Who, _: &mut Self::Endpoints,
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
        let result: ::core::result::Result<Reply, Self::Error> =
            async move { Ok(Reply { actor: context.context().require::<Principal>()?.0.clone() }) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl harmos_runtime::operations::authorization::Authorizer for ExternalAccess {
    fn required(
        &self, context: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Required>> {
        use harmos_domain::plugin::Grant;
        use harmos_runtime::operations::authorization::Required;
        // Reentrant Read Proves The Callback Does Not Hold The Original Registry Lock.
        assert_eq!(self.operations.upgrade().unwrap().owner(provider.definition.id.as_str())?, *provider.owner);
        assert!(provider.generation.value() > 0);
        let Some(principal) = context.get::<Principal>() else {
            return Ok(None);
        };
        let mut grants = Vec::new();
        match provider.definition.id.as_str() {
            "host.operations" | "grpc.descendant" => (),
            "grpc.guarded" => {
                match self.mode.load(std::sync::atomic::Ordering::Acquire) {
                    1 => {
                        return Ok(Some(Required {
                            contract: harmos_runtime::operations::WireDeclaration::of::<Descendant>().contract().unwrap(),
                            grants,
                        }));
                    }
                    2 => panic!("authorizer panic fixture"),
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
        Ok(Some(Required { contract: provider.contract.clone(), grants }))
    }
}

impl Operation for Guard {
    type Declaration = Guarded;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> harmos_runtime::Result<Self> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self, context: Self::Context<'a>, _: &mut Guarded, _: &mut (),
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            let principal = context.context().get::<Principal>().map_or("anonymous", |value| value.0.as_str());
            Ok(Reply { actor: format!("{}:{principal}", if context.is_host_call() { "host" } else { "external" }) })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for Descend {
    type Declaration = Descendant;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> harmos_runtime::Result<Self> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self, context: Self::Context<'a>, _: &mut Descendant, _: &mut (),
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            let response = context.call(Guarded {}).await?;
            Ok(Reply { actor: response.actor.clone() })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
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
                    harmos_runtime::grpc::server::download::<Hold, harmos_runtime::grpc::Transcoded>(authority, wire_settings(), request)
                        .await
                }
                "/fixture.Identity/Download" => {
                    harmos_runtime::grpc::server::download::<Download, harmos_runtime::grpc::Transcoded>(
                        authority,
                        wire_settings(),
                        request,
                    )
                    .await
                }
                "/fixture.Identity/Upload" => {
                    harmos_runtime::grpc::server::upload::<Upload, harmos_runtime::grpc::Transcoded>(authority, wire_settings(), request)
                        .await
                }
                "/fixture.Identity/Exchange" => {
                    harmos_runtime::grpc::server::exchange::<Exchange, harmos_runtime::grpc::Transcoded>(
                        authority,
                        wire_settings(),
                        request,
                    )
                    .await
                }
                "/fixture.Identity/Guarded" => {
                    harmos_runtime::grpc::server::unary::<Guarded, harmos_runtime::grpc::Transcoded>(authority, wire_settings(), request)
                        .await
                }
                "/fixture.Identity/Descendant" => {
                    harmos_runtime::grpc::server::unary::<Descendant, harmos_runtime::grpc::Transcoded>(authority, wire_settings(), request)
                        .await
                }
                _ => {
                    harmos_runtime::grpc::server::unary::<Who, harmos_runtime::grpc::Transcoded>(authority, wire_settings(), request).await
                }
            })
        })
    }
}

impl Operation for Downloading {
    type Declaration = Download;
    type Resources = ();
    type Error = Error;
    type Endpoints = harmos_runtime::operations::StreamOutput<Reply>;
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> harmos_runtime::Result<Self> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self, context: Self::Context<'a>, _request: &mut Download, output: &mut Self::Endpoints,
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            let actor = context.context().require::<Principal>()?.0.clone();
            output.send_with(128, || Ok(Reply { actor: actor.clone() })).await?;
            Ok(Reply { actor })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for Reject {
    type Declaration = Who;
    type Resources = ();
    type Error = tonic::Status;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, tonic::Status> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self, _: Self::Context<'a>, _: &mut Who, _: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Reply>, tonic::Status> {
        let result: ::core::result::Result<Reply, Self::Error> = async move { Err(authored_rejection()) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for Crash {
    type Declaration = Who;
    type Resources = ();
    type Error = std::io::Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, std::io::Error> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self, _: Self::Context<'a>, _: &mut Who, _: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Reply>, std::io::Error> {
        let result: ::core::result::Result<Reply, Self::Error> = async move { Err(std::io::Error::other("disk detail")) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for RejectStream {
    type Declaration = Download;
    type Resources = ();
    type Error = tonic::Status;
    type Endpoints = harmos_runtime::operations::StreamOutput<Reply>;
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, tonic::Status> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self, _: Self::Context<'a>, _: &mut Download, _: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Reply>, tonic::Status> {
        let result: ::core::result::Result<Reply, Self::Error> = async move { Err(authored_rejection()) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for CrashStream {
    type Declaration = Download;
    type Resources = ();
    type Error = std::io::Error;
    type Endpoints = harmos_runtime::operations::StreamOutput<Reply>;
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, std::io::Error> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self, _: Self::Context<'a>, _: &mut Download, _: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Reply>, std::io::Error> {
        let result: ::core::result::Result<Reply, Self::Error> = async move { Err(std::io::Error::other("disk detail")) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for RejectUpload {
    type Declaration = Upload;
    type Resources = ();
    type Error = tonic::Status;
    type Endpoints = harmos_runtime::operations::StreamInput<Reply>;
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, tonic::Status> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self, _: Self::Context<'a>, _: &mut Upload, _: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Reply>, tonic::Status> {
        let result: ::core::result::Result<Reply, Self::Error> = async move { Err(authored_rejection()) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for RejectExchange {
    type Declaration = Exchange;
    type Resources = ();
    type Error = Error;
    type Endpoints = (harmos_runtime::operations::StreamInput<Reply>, harmos_runtime::operations::StreamOutput<Reply>);
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, Error> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self, _: Self::Context<'a>, _: &mut Exchange, _: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Reply>, Error> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            Err(Error::TaskFailed {
                domain: "fixture response",
                source: Arc::<Error>::new(Error::OperationFailed {
                    operation: "fixture rejection".into(),
                    source: Arc::new(authored_rejection()),
                }),
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for CrashUpload {
    type Declaration = Upload;
    type Resources = ();
    type Error = std::io::Error;
    type Endpoints = harmos_runtime::operations::StreamInput<Reply>;
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, std::io::Error> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self, _: Self::Context<'a>, _: &mut Upload, _: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Reply>, std::io::Error> {
        let result: ::core::result::Result<Reply, Self::Error> = async move { Err(std::io::Error::other("disk detail")) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for CrashExchange {
    type Declaration = Exchange;
    type Resources = ();
    type Error = std::io::Error;
    type Endpoints = (harmos_runtime::operations::StreamInput<Reply>, harmos_runtime::operations::StreamOutput<Reply>);
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, std::io::Error> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self, _: Self::Context<'a>, _: &mut Exchange, _: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Reply>, std::io::Error> {
        let result: ::core::result::Result<Reply, Self::Error> = async move { Err(std::io::Error::other("disk detail")) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for Uploading {
    type Declaration = Upload;
    type Resources = tokio::sync::Notify;
    type Error = Error;
    type Endpoints = harmos_runtime::operations::StreamInput<Reply>;
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(started: Arc<Self::Resources>, _: Self::Context<'ctx>) -> harmos_runtime::Result<Self> {
        Ok(Self { started })
    }
    async fn run<'a>(
        &mut self, context: Self::Context<'a>, _request: &mut Upload, input: &mut Self::Endpoints,
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            self.started.notify_one();
            while input.recv().await.is_some() {}
            Ok(Reply { actor: context.context().require::<Principal>()?.0.clone() })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for Exchanging {
    type Declaration = Exchange;
    type Resources = ();
    type Error = Error;
    type Endpoints = (harmos_runtime::operations::StreamInput<Reply>, harmos_runtime::operations::StreamOutput<Reply>);
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> harmos_runtime::Result<Self> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self, context: Self::Context<'a>, _request: &mut Exchange, endpoints: &mut Self::Endpoints,
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            let actor = context.context().require::<Principal>()?.0.clone();
            while endpoints.0.recv().await.is_some() {
                endpoints.1.send_with(128, || Ok(Reply { actor: actor.clone() })).await?;
            }
            Ok(Reply { actor })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Operation for Holding {
    type Declaration = Hold;
    type Resources = tokio::sync::Semaphore;
    type Error = Error;
    type Endpoints = harmos_runtime::operations::StreamOutput<Reply>;
    type Context<'a> = Context<'a>;
    async fn initialize<'ctx>(stopped: Arc<Self::Resources>, _: Self::Context<'ctx>) -> harmos_runtime::Result<Self> {
        Ok(Self { stopped })
    }
    async fn run<'a>(
        &mut self, context: Self::Context<'a>, _request: &mut Hold, output: &mut Self::Endpoints,
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            let actor = context.context().require::<Principal>()?.0.clone();
            output.send_with(128, || Ok(Reply { actor })).await?;
            std::future::pending().await
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn stop(&mut self) -> harmos_runtime::Result<()> {
        self.stopped.add_permits(1);
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn authored_rejection() -> tonic::Status {
    let mut metadata = tonic::metadata::MetadataMap::new();
    metadata.insert("x-fixture-rejection", "absent".parse().unwrap());
    metadata.insert_bin("x-fixture-context-bin", tonic::metadata::MetadataValue::from_bytes(&[0, 255, 1]));
    tonic::Status::with_details_and_metadata(tonic::Code::NotFound, "no such actor", bytes::Bytes::from_static(REJECTION_DETAILS), metadata)
}

fn limits() -> (DecodeSettings, EncodeSettings) {
    let bytes = NonZeroUsize::new(4096).unwrap();
    (DecodeSettings { message_bytes: bytes, retained_bytes: bytes }, EncodeSettings { message_bytes: bytes, buffer_bytes: bytes })
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
        web: false,
        allowed_origins: Vec::new(),
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

// One unary call to a built-in operation over its declared gRPC route.
async fn builtin_call<Q, R>(channel: tonic::transport::Channel, path: &'static str, request: Q) -> Result<R, tonic::Status>
where
    Q: harmos_domain::Portable,
    R: harmos_domain::Message + harmos_domain::schema::Described + harmos_runtime::MemorySize + Clone + Send + Sync + 'static,
{
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.map_err(|_| tonic::Status::unavailable("channel"))?;
    let request = tonic::Request::new(harmos_runtime::operations::Response::new(request));
    let (decode, encode) = limits();
    let response: tonic::Response<harmos_runtime::operations::StreamItem<R>> = client
        .unary(
            request,
            http::uri::PathAndQuery::from_static(path),
            Codec::<harmos_runtime::grpc::Transcoded, R, harmos_runtime::operations::Response<Q>>::new(decode, encode)?,
        )
        .await?;
    Ok((*response.into_inner()).clone())
}

async fn external_catalog(
    channel: tonic::transport::Channel, token: Option<&'static str>,
) -> Result<Vec<harmos_runtime::operations::host::DiscoveredOperation>, tonic::Status> {
    use harmos_runtime::operations::host::{Catalog, CatalogKind, CatalogPage};
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.map_err(|_| tonic::Status::unavailable("channel"))?;
    let mut request = tonic::Request::new(harmos_runtime::operations::Response::new(Catalog {
        kind: CatalogKind::Operations,
        schema: None,
        after: None,
        rows: None,
        bytes: None,
    }));
    if let Some(token) = token {
        request.metadata_mut().insert("authorization", token.parse().unwrap());
    }
    let (decode, encode) = limits();
    let response: tonic::Response<harmos_runtime::operations::StreamItem<CatalogPage>> = client
        .unary(
            request,
            http::uri::PathAndQuery::from_static("/harmos.host.v1.Host/Catalog"),
            Codec::<harmos_runtime::grpc::Transcoded, CatalogPage, harmos_runtime::operations::Response<Catalog>>::new(decode, encode)?,
        )
        .await?;
    Ok(serde_json::from_slice(&response.into_inner().entries).unwrap())
}

async fn web(address: std::net::SocketAddr, method: &str, path: &str, headers: &[(&str, &str)]) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: harmos\r\nConnection: close\r\n");
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    let body: &[u8] = if method == "POST" { b"\0\0\0\0\0" } else { b"" };
    request.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
    stream.write_all(request.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();
    let mut response = Vec::new();
    (stream.read_to_end(&mut response)).await.unwrap();
    String::from_utf8_lossy(&response).to_ascii_lowercase()
}

async fn failures(register: impl FnOnce(&harmos_runtime::operations::OperationManager)) -> [tonic::Status; 4] {
    use harmos_runtime::grpc::{InputEnvelope, OutputEnvelope};
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    register(&runtime.operations);
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint { authority: harmos_runtime::grpc::server::Authority::TrustedHost(runtime.operations.clone()) })
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
    runtime.stop().await.unwrap();
    [unary, streaming, upload, exchange]
}

/* === Tests === */
