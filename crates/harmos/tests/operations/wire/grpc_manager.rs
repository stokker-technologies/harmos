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
use harmos::grpc::authentication::{Authenticate, Authentication, Authenticator};
use harmos::grpc::middleware::{Enter, Invocation, Middleware, Observation};
use harmos::grpc::{Codec, DecodeSettings, EncodeSettings};
use harmos::operations::{
    Context, Operation, OperationDeclaration, OperationPolicy, RetryPolicy, StreamSettings,
};
use harmos::{Error, Runtime};

/* === Constants === */
// A google.rpc.Status With One Typed Domain Rejection In An Any Envelope.
const REJECTION_DETAILS: &[u8] = &[
    8, 5, 18, 13, 110, 111, 32, 115, 117, 99, 104, 32, 97, 99, 116, 111, 114, 26, 49, 10, 37, 116,
    121, 112, 101, 46, 103, 111, 111, 103, 108, 101, 97, 112, 105, 115, 46, 99, 111, 109, 47, 102,
    105, 120, 116, 117, 114, 101, 46, 82, 101, 106, 101, 99, 116, 105, 111, 110, 18, 8, 10, 6, 97,
    98, 115, 101, 110, 116,
];

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Principal(String);
struct Guarded;
struct Guard;
struct Descendant;
struct Descend;
struct ExternalAccess {
    operations: std::sync::Weak<harmos::operations::OperationManager>,
    mode: Arc<std::sync::atomic::AtomicUsize>,
}
struct Sessions;
struct PendingAuthentication;
struct DiscoveryAccess;
struct Who;
struct Read;
struct Download;
struct Reject;
struct Crash;
struct RejectStream;
struct CrashStream;
struct RejectUpload;
struct RejectExchange;
struct CrashUpload;
struct CrashExchange;
struct Upload;
struct Exchange;
struct Hold {
    stopped: Arc<tokio::sync::Semaphore>,
}
#[harmos::message]
struct Reply {
    actor: String,
}
#[harmos::message]
struct Ask {}
#[derive(Clone)]
struct Endpoint {
    authority: harmos::grpc::server::Authority,
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
            match call
                .metadata()
                .get("authorization")
                .and_then(|value| value.to_str().ok())
            {
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
            if principal.0 == "bob"
                && !call
                    .path()
                    .starts_with("/grpc.reflection.v1.ServerReflection/")
            {
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
impl OperationDeclaration for Who {
    type Request = Ask;
    type Response = Reply;
    const ID: &'static str = "grpc.who";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    };
}
impl Operation for Read {
    type Declaration = Who;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> harmos::Result<Self> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        context: Self::Context<'a>,
        _request: &mut Ask,
        _: &mut Self::Endpoints,
    ) -> harmos::Result<harmos::operations::Response<Reply>> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            Ok(Reply {
                actor: context.context().require::<Principal>()?.0.clone(),
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}
impl harmos::operations::authorization::Authorizer for ExternalAccess {
    fn required(
        &self,
        context: &harmos::operations::CallContext,
        provider: harmos::operations::authorization::Provider<'_>,
    ) -> harmos::Result<Option<harmos::operations::authorization::Required>> {
        use harmos::operations::authorization::Required;
        use harmos_contract::plugin::Grant;
        // Reentrant Read Proves The Callback Does Not Hold The Original Registry Lock.
        assert_eq!(
            self.operations
                .upgrade()
                .unwrap()
                .owner(provider.definition.id.as_str())?,
            *provider.owner
        );
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
                            contract: harmos::operations::WireDeclaration::of::<Descendant>()
                                .contract()
                                .unwrap(),
                            grants,
                        }));
                    }
                    2 => panic!("authorizer panic fixture"),
                    _ => (),
                }
                match principal.0.as_str() {
                    "alice" | "carol" => grants.push(Grant::Survey("reading".into())),
                    "limited" => (),
                    _ => return Ok(None),
                }
            }
            _ => return Ok(None),
        }
        Ok(Some(Required {
            contract: provider.contract.clone(),
            grants,
        }))
    }
}

impl OperationDeclaration for Guarded {
    type Request = Ask;
    type Response = Reply;
    const ID: &'static str = "grpc.guarded";
    const POLICY: OperationPolicy = Who::POLICY;
    fn visit_wire<V: harmos::operations::WireVisitor>(visitor: V) -> Option<V::Entry> {
        Some(visitor.unary::<Self>())
    }
    fn grant(_: &Ask) -> Option<harmos_contract::plugin::Grant> {
        Some(harmos_contract::plugin::Grant::Survey("reading".into()))
    }
    fn key(_: &Ask) -> Option<harmos::operations::CacheKey> {
        Some(harmos::operations::CacheKey::new(b"same-request".to_vec()).unwrap())
    }
}
impl harmos::operations::WireOperation for Guarded {
    const VERSION: u32 = 1;
}
impl Operation for Guard {
    type Declaration = Guarded;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> harmos::Result<Self> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        context: Self::Context<'a>,
        _: &mut Ask,
        _: &mut (),
    ) -> harmos::Result<harmos::operations::Response<Reply>> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            let principal = context
                .context()
                .get::<Principal>()
                .map_or("anonymous", |value| value.0.as_str());
            Ok(Reply {
                actor: format!(
                    "{}:{principal}",
                    if context.is_host_call() {
                        "host"
                    } else {
                        "external"
                    }
                ),
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}
impl OperationDeclaration for Descendant {
    type Request = Ask;
    type Response = Reply;
    const ID: &'static str = "grpc.descendant";
    const POLICY: OperationPolicy = Who::POLICY;
    fn visit_wire<V: harmos::operations::WireVisitor>(visitor: V) -> Option<V::Entry> {
        Some(visitor.unary::<Self>())
    }
}
impl harmos::operations::WireOperation for Descendant {
    const VERSION: u32 = 1;
}
impl Operation for Descend {
    type Declaration = Descendant;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> harmos::Result<Self> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        context: Self::Context<'a>,
        _: &mut Ask,
        _: &mut (),
    ) -> harmos::Result<harmos::operations::Response<Reply>> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            let response = context.call::<Guarded>(Ask {}).await?;
            Ok(Reply {
                actor: response.actor.clone(),
            })
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
                    harmos::grpc::server::download::<Hold>(authority, wire_settings(), request)
                        .await
                }
                "/fixture.Identity/Download" => {
                    harmos::grpc::server::download::<Download>(authority, wire_settings(), request)
                        .await
                }
                "/fixture.Identity/Upload" => {
                    harmos::grpc::server::upload::<Upload>(authority, wire_settings(), request)
                        .await
                }
                "/fixture.Identity/Exchange" => {
                    harmos::grpc::server::exchange::<Exchange>(authority, wire_settings(), request)
                        .await
                }
                "/fixture.Identity/Guarded" => {
                    harmos::grpc::server::unary::<Guarded>(authority, wire_settings(), request)
                        .await
                }
                "/fixture.Identity/Descendant" => {
                    harmos::grpc::server::unary::<Descendant>(authority, wire_settings(), request)
                        .await
                }
                _ => harmos::grpc::server::unary::<Who>(authority, wire_settings(), request).await,
            })
        })
    }
}

impl OperationDeclaration for Download {
    type Request = Ask;
    type Response = Reply;
    const ID: &'static str = "grpc.download";
    const POLICY: OperationPolicy = Who::POLICY;
    fn endpoint_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (None, Some(std::any::TypeId::of::<Reply>()))
    }
}
impl harmos::operations::OutputDeclaration for Download {
    type Item = Reply;
}
impl Operation for Download {
    type Declaration = Self;
    type Resources = ();
    type Error = Error;
    type Endpoints = harmos::operations::StreamOutput<Reply>;
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> harmos::Result<Self> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        context: Self::Context<'a>,
        _request: &mut Ask,
        output: &mut Self::Endpoints,
    ) -> harmos::Result<harmos::operations::Response<Reply>> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            let actor = context.context().require::<Principal>()?.0.clone();
            output
                .send_with(128, || {
                    Ok(Reply {
                        actor: actor.clone(),
                    })
                })
                .await?;
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
    fn new(_: Arc<()>) -> Result<Self, tonic::Status> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        _: &mut Ask,
        _: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Reply>, tonic::Status> {
        let result: ::core::result::Result<Reply, Self::Error> =
            async move { Err(authored_rejection()) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for Crash {
    type Declaration = Who;
    type Resources = ();
    type Error = std::io::Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> Result<Self, std::io::Error> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        _: &mut Ask,
        _: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Reply>, std::io::Error> {
        let result: ::core::result::Result<Reply, Self::Error> =
            async move { Err(std::io::Error::other("disk detail")) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for RejectStream {
    type Declaration = Download;
    type Resources = ();
    type Error = tonic::Status;
    type Endpoints = harmos::operations::StreamOutput<Reply>;
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> Result<Self, tonic::Status> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        _: &mut Ask,
        _: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Reply>, tonic::Status> {
        let result: ::core::result::Result<Reply, Self::Error> =
            async move { Err(authored_rejection()) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for CrashStream {
    type Declaration = Download;
    type Resources = ();
    type Error = std::io::Error;
    type Endpoints = harmos::operations::StreamOutput<Reply>;
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> Result<Self, std::io::Error> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        _: &mut Ask,
        _: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Reply>, std::io::Error> {
        let result: ::core::result::Result<Reply, Self::Error> =
            async move { Err(std::io::Error::other("disk detail")) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for RejectUpload {
    type Declaration = Upload;
    type Resources = ();
    type Error = tonic::Status;
    type Endpoints = harmos::operations::StreamInput<Reply>;
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> Result<Self, tonic::Status> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        _: &mut Ask,
        _: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Reply>, tonic::Status> {
        let result: ::core::result::Result<Reply, Self::Error> =
            async move { Err(authored_rejection()) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for RejectExchange {
    type Declaration = Exchange;
    type Resources = ();
    type Error = Error;
    type Endpoints = (
        harmos::operations::StreamInput<Reply>,
        harmos::operations::StreamOutput<Reply>,
    );
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> Result<Self, Error> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        _: &mut Ask,
        _: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Reply>, Error> {
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
    type Endpoints = harmos::operations::StreamInput<Reply>;
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> Result<Self, std::io::Error> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        _: &mut Ask,
        _: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Reply>, std::io::Error> {
        let result: ::core::result::Result<Reply, Self::Error> =
            async move { Err(std::io::Error::other("disk detail")) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl Operation for CrashExchange {
    type Declaration = Exchange;
    type Resources = ();
    type Error = std::io::Error;
    type Endpoints = (
        harmos::operations::StreamInput<Reply>,
        harmos::operations::StreamOutput<Reply>,
    );
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> Result<Self, std::io::Error> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        _: &mut Ask,
        _: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Reply>, std::io::Error> {
        let result: ::core::result::Result<Reply, Self::Error> =
            async move { Err(std::io::Error::other("disk detail")) }.await;
        result.map(::core::convert::Into::into)
    }
}
impl OperationDeclaration for Upload {
    type Request = Ask;
    type Response = Reply;
    const ID: &'static str = "grpc.upload";
    const POLICY: OperationPolicy = Who::POLICY;
    fn endpoint_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (Some(std::any::TypeId::of::<Reply>()), None)
    }
}
impl harmos::operations::InputDeclaration for Upload {
    type Item = Reply;
}
impl Operation for Upload {
    type Declaration = Self;
    type Resources = ();
    type Error = Error;
    type Endpoints = harmos::operations::StreamInput<Reply>;
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> harmos::Result<Self> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        context: Self::Context<'a>,
        _request: &mut Ask,
        input: &mut Self::Endpoints,
    ) -> harmos::Result<harmos::operations::Response<Reply>> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            while input.recv().await.is_some() {}
            Ok(Reply {
                actor: context.context().require::<Principal>()?.0.clone(),
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}
impl OperationDeclaration for Exchange {
    type Request = Ask;
    type Response = Reply;
    const ID: &'static str = "grpc.exchange";
    const POLICY: OperationPolicy = Who::POLICY;
    fn endpoint_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (
            Some(std::any::TypeId::of::<Reply>()),
            Some(std::any::TypeId::of::<Reply>()),
        )
    }
}
impl harmos::operations::InputDeclaration for Exchange {
    type Item = Reply;
}
impl harmos::operations::OutputDeclaration for Exchange {
    type Item = Reply;
}
impl Operation for Exchange {
    type Declaration = Self;
    type Resources = ();
    type Error = Error;
    type Endpoints = (
        harmos::operations::StreamInput<Reply>,
        harmos::operations::StreamOutput<Reply>,
    );
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> harmos::Result<Self> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        context: Self::Context<'a>,
        _request: &mut Ask,
        endpoints: &mut Self::Endpoints,
    ) -> harmos::Result<harmos::operations::Response<Reply>> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            let actor = context.context().require::<Principal>()?.0.clone();
            while endpoints.0.recv().await.is_some() {
                endpoints
                    .1
                    .send_with(128, || {
                        Ok(Reply {
                            actor: actor.clone(),
                        })
                    })
                    .await?;
            }
            Ok(Reply { actor })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl OperationDeclaration for Hold {
    type Request = Ask;
    type Response = Reply;
    const ID: &'static str = "grpc.hold";
    const POLICY: OperationPolicy = Who::POLICY;
    fn endpoint_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (None, Some(std::any::TypeId::of::<Reply>()))
    }
}
impl harmos::operations::OutputDeclaration for Hold {
    type Item = Reply;
}
impl Operation for Hold {
    type Declaration = Self;
    type Resources = tokio::sync::Semaphore;
    type Error = Error;
    type Endpoints = harmos::operations::StreamOutput<Reply>;
    type Context<'a> = Context<'a>;
    fn new(stopped: Arc<Self::Resources>) -> harmos::Result<Self> {
        Ok(Self { stopped })
    }
    async fn run<'a>(
        &mut self,
        context: Self::Context<'a>,
        _request: &mut Ask,
        output: &mut Self::Endpoints,
    ) -> harmos::Result<harmos::operations::Response<Reply>> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            let actor = context.context().require::<Principal>()?.0.clone();
            output.send_with(128, || Ok(Reply { actor })).await?;
            std::future::pending().await
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn stop(&mut self) -> harmos::Result<()> {
        self.stopped.add_permits(1);
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn authored_rejection() -> tonic::Status {
    let mut metadata = tonic::metadata::MetadataMap::new();
    metadata.insert("x-fixture-rejection", "absent".parse().unwrap());
    metadata.insert_bin(
        "x-fixture-context-bin",
        tonic::metadata::MetadataValue::from_bytes(&[0, 255, 1]),
    );
    tonic::Status::with_details_and_metadata(
        tonic::Code::NotFound,
        "no such actor",
        bytes::Bytes::from_static(REJECTION_DETAILS),
        metadata,
    )
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
fn wire_settings() -> harmos::grpc::server::Settings {
    let (decode, encode) = limits();
    let capacity = NonZeroUsize::new(4096).unwrap();
    harmos::grpc::server::Settings {
        calls: NonZeroUsize::new(8).unwrap(),
        decode,
        encode,
        input: StreamSettings {
            items: capacity,
            bytes: capacity,
        },
        output: StreamSettings {
            items: capacity,
            bytes: capacity,
        },
        opening_timeout: Duration::from_secs(1),
    }
}
fn settings() -> harmos::grpc::Settings {
    harmos::grpc::Settings {
        operations: harmos::grpc::server::Settings::default(),
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
async fn call(
    channel: tonic::transport::Channel,
    token: Option<&'static str>,
) -> Result<String, tonic::Status> {
    let mut client = tonic::client::Grpc::new(channel);
    client
        .ready()
        .await
        .map_err(|_| tonic::Status::unavailable("channel"))?;
    let mut request = tonic::Request::new(harmos::operations::Response::new(Ask {}));
    if let Some(token) = token {
        request
            .metadata_mut()
            .insert("authorization", token.parse().unwrap());
    }
    let (decode, encode) = limits();
    let response = client
        .unary(
            request,
            http::uri::PathAndQuery::from_static("/fixture.Identity/Who"),
            Codec::<Reply, harmos::operations::Response<Ask>>::new(decode, encode)?,
        )
        .await?;
    Ok(response.into_inner().actor.clone())
}

async fn external_call(
    channel: tonic::transport::Channel,
    token: Option<&'static str>,
    path: &'static str,
) -> Result<String, tonic::Status> {
    let mut client = tonic::client::Grpc::new(channel);
    client
        .ready()
        .await
        .map_err(|_| tonic::Status::unavailable("channel"))?;
    let mut request = tonic::Request::new(harmos::operations::Response::new(Ask {}));
    if let Some(token) = token {
        request
            .metadata_mut()
            .insert("authorization", token.parse().unwrap());
    }
    let (decode, encode) = limits();
    let response = client
        .unary(
            request,
            http::uri::PathAndQuery::from_static(path),
            Codec::<Reply, harmos::operations::Response<Ask>>::new(decode, encode)?,
        )
        .await?;
    Ok(response.into_inner().actor.clone())
}

async fn external_catalog(
    channel: tonic::transport::Channel,
    token: Option<&'static str>,
) -> Result<Vec<harmos::operations::host::DiscoveredOperation>, tonic::Status> {
    use harmos::operations::host::{OperationsRequest, OperationsResponse};
    let mut client = tonic::client::Grpc::new(channel);
    client
        .ready()
        .await
        .map_err(|_| tonic::Status::unavailable("channel"))?;
    let mut request = tonic::Request::new(harmos::operations::Response::new(OperationsRequest {
        after: None,
        rows: None,
        bytes: None,
    }));
    if let Some(token) = token {
        request
            .metadata_mut()
            .insert("authorization", token.parse().unwrap());
    }
    let (decode, encode) = limits();
    let response: tonic::Response<harmos::operations::StreamItem<OperationsResponse>> = client
        .unary(
            request,
            http::uri::PathAndQuery::from_static("/harmos.host.v1.Host/Operations"),
            Codec::<OperationsResponse, harmos::operations::Response<OperationsRequest>>::new(
                decode, encode,
            )?,
        )
        .await?;
    Ok(serde_json::from_slice(&response.into_inner().catalog).unwrap())
}

async fn web(
    address: std::net::SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
) -> String {
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
    tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut response))
        .await
        .unwrap()
        .unwrap();
    String::from_utf8_lossy(&response).to_ascii_lowercase()
}

async fn failures(
    register: impl FnOnce(&harmos::operations::OperationManager),
) -> [tonic::Status; 4] {
    use harmos::grpc::{InputEnvelope, OutputEnvelope};
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    register(&runtime.operations);
    runtime
        .grpc()
        .register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions))))
        .unwrap();
    runtime
        .grpc()
        .register_service(Endpoint {
            authority: harmos::grpc::server::Authority::TrustedHost(runtime.operations.clone()),
        })
        .unwrap();
    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let channel =
        tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr()))
            .unwrap()
            .connect()
            .await
            .unwrap();
    let unary = call(channel.clone(), Some("Bearer alice"))
        .await
        .unwrap_err();
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();
    let (decode, encode) = limits();
    let mut request = tonic::Request::new(harmos::operations::Response::new(Ask {}));
    request
        .metadata_mut()
        .insert("authorization", "Bearer alice".parse().unwrap());
    let streaming = match client
        .server_streaming(
            request,
            http::uri::PathAndQuery::from_static("/fixture.Identity/Download"),
            Codec::<OutputEnvelope<Reply, Reply>, harmos::operations::Response<Ask>>::new(
                decode, encode,
            )
            .unwrap(),
        )
        .await
    {
        Ok(response) => response
            .into_inner()
            .message()
            .await
            .err()
            .expect("stream fails"),
        Err(status) => status,
    };
    client.ready().await.unwrap();
    let mut request = tonic::Request::new(futures_util::stream::iter([
        harmos::operations::Response::new(InputEnvelope::<Ask, Reply>::Request(Ask {})),
    ]));
    request
        .metadata_mut()
        .insert("authorization", "Bearer alice".parse().unwrap());
    let upload = client
        .client_streaming(
            request,
            http::uri::PathAndQuery::from_static("/fixture.Identity/Upload"),
            Codec::<Reply, harmos::operations::Response<InputEnvelope<Ask, Reply>>>::new(
                decode, encode,
            )
            .unwrap(),
        )
        .await
        .err()
        .expect("upload fails");
    client.ready().await.unwrap();
    let mut request = tonic::Request::new(futures_util::stream::iter([
        harmos::operations::Response::new(InputEnvelope::<Ask, Reply>::Request(Ask {})),
    ]));
    request
        .metadata_mut()
        .insert("authorization", "Bearer alice".parse().unwrap());
    let exchange = match client
        .streaming(
            request,
            http::uri::PathAndQuery::from_static("/fixture.Identity/Exchange"),
            Codec::<
                OutputEnvelope<Reply, Reply>,
                harmos::operations::Response<InputEnvelope<Ask, Reply>>,
            >::new(decode, encode)
            .unwrap(),
        )
        .await
    {
        Ok(response) => response
            .into_inner()
            .message()
            .await
            .err()
            .expect("exchange fails"),
        Err(status) => status,
    };
    drop(attached);
    runtime.stop().await.unwrap();
    [unary, streaming, upload, exchange]
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("external_authority_parity", || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    use harmos::operations::discovery::Discovery;
                    use harmos_contract::plugin::Grant;
                    let runtime = Runtime::<App>::builder((), ())
                        .register(Lanes::catalog())
                        .runtime()
                        .await
                        .unwrap();
                    runtime
                        .operations
                        .register_wire_operation::<Guard>("app", Lanes::Open, Arc::new(()))
                        .unwrap();
                    runtime
                        .operations
                        .register_wire_operation::<Descend>("app", Lanes::Open, Arc::new(()))
                        .unwrap();
                    runtime
                        .operations
                        .register_wire_operation::<Discovery>("host", Lanes::Open, Arc::new(()))
                        .unwrap();
                    // Internal Boot Work Does Not Freeze The Later Listener Policy.
                    assert_eq!(
                        runtime
                            .operations
                            .call::<Guarded>(Ask {})
                            .await
                            .unwrap()
                            .actor,
                        "host:anonymous"
                    );
                    let mode = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                    let operations = Arc::new(runtime.operations.clone());
                    let policy = Arc::new(ExternalAccess {
                        operations: Arc::downgrade(&operations),
                        mode: mode.clone(),
                    });
                    runtime
                        .operations
                        .register_authorizer(policy.clone())
                        .unwrap();
                    assert!(
                        runtime
                            .operations
                            .register_authorizer(policy.clone())
                            .is_err()
                    );
                    runtime
                        .grpc()
                        .register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions))))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_service(Endpoint {
                            authority: harmos::grpc::server::Authority::TrustedHost(
                                runtime.operations.clone(),
                            ),
                        })
                        .unwrap();
                    let attached = runtime.grpc().attach(settings()).await.unwrap();
                    let channel = tonic::transport::Endpoint::from_shared(format!(
                        "http://{}",
                        attached.local_addr()
                    ))
                    .unwrap()
                    .connect()
                    .await
                    .unwrap();
                    assert_eq!(
                        external_catalog(channel.clone(), None)
                            .await
                            .unwrap_err()
                            .code(),
                        tonic::Code::Unauthenticated
                    );
                    let alice = external_catalog(channel.clone(), Some("Bearer alice"))
                        .await
                        .unwrap();
                    let guarded = alice
                        .iter()
                        .find(|entry| entry.contract.operation().id.as_str() == "grpc.guarded")
                        .unwrap();
                    assert_eq!(guarded.grants, [Grant::Survey("reading".into())]);
                    let bob = external_catalog(channel.clone(), Some("Bearer bob"))
                        .await
                        .unwrap();
                    assert!(
                        bob.iter()
                            .all(|entry| entry.contract.operation().id.as_str() != "grpc.guarded")
                    );
                    assert_eq!(
                        external_call(
                            channel.clone(),
                            Some("Bearer alice"),
                            "/fixture.Identity/Guarded"
                        )
                        .await
                        .unwrap(),
                        "external:alice"
                    );
                    assert_eq!(
                        external_call(
                            channel.clone(),
                            Some("Bearer bob"),
                            "/fixture.Identity/Guarded"
                        )
                        .await
                        .unwrap_err()
                        .code(),
                        tonic::Code::PermissionDenied
                    );
                    assert_eq!(
                        external_call(
                            channel.clone(),
                            Some("Bearer limited"),
                            "/fixture.Identity/Guarded"
                        )
                        .await
                        .unwrap_err()
                        .code(),
                        tonic::Code::PermissionDenied
                    );
                    assert_eq!(
                        external_call(
                            channel.clone(),
                            Some("Bearer alice"),
                            "/fixture.Identity/Descendant"
                        )
                        .await
                        .unwrap(),
                        "external:alice"
                    );
                    assert_eq!(
                        external_call(
                            channel.clone(),
                            Some("Bearer bob"),
                            "/fixture.Identity/Descendant"
                        )
                        .await
                        .unwrap_err()
                        .code(),
                        tonic::Code::PermissionDenied
                    );
                    assert_eq!(
                        external_call(
                            channel.clone(),
                            Some("Bearer alice"),
                            "/fixture.Identity/Guarded"
                        )
                        .await
                        .unwrap(),
                        "external:alice"
                    );
                    assert_eq!(
                        external_call(
                            channel.clone(),
                            Some("Bearer carol"),
                            "/fixture.Identity/Guarded"
                        )
                        .await
                        .unwrap(),
                        "external:carol"
                    );
                    runtime.operations.clear_cache();
                    let mut context = harmos::operations::CallContext::default();
                    context.insert(Principal("bob".into())).unwrap();
                    assert_eq!(
                        runtime
                            .operations
                            .with_context(context)
                            .call::<Descendant>(Ask {})
                            .await
                            .unwrap()
                            .actor,
                        "host:bob"
                    );
                    assert_eq!(
                        external_call(
                            channel.clone(),
                            Some("Bearer carol"),
                            "/fixture.Identity/Guarded"
                        )
                        .await
                        .unwrap(),
                        "external:carol"
                    );
                    mode.store(1, std::sync::atomic::Ordering::Release);
                    assert!(
                        external_catalog(channel.clone(), Some("Bearer alice"))
                            .await
                            .unwrap()
                            .iter()
                            .all(|entry| entry.contract.operation().id.as_str() != "grpc.guarded")
                    );
                    assert_eq!(
                        external_call(
                            channel.clone(),
                            Some("Bearer alice"),
                            "/fixture.Identity/Guarded"
                        )
                        .await
                        .unwrap_err()
                        .code(),
                        tonic::Code::PermissionDenied
                    );
                    mode.store(2, std::sync::atomic::Ordering::Release);
                    assert_eq!(
                        external_call(
                            channel.clone(),
                            Some("Bearer alice"),
                            "/fixture.Identity/Guarded"
                        )
                        .await
                        .unwrap_err()
                        .code(),
                        tonic::Code::InvalidArgument
                    );
                    mode.store(0, std::sync::atomic::Ordering::Release);
                    assert_eq!(
                        external_call(
                            channel.clone(),
                            Some("Bearer alice"),
                            "/fixture.Identity/Guarded"
                        )
                        .await
                        .unwrap(),
                        "external:alice"
                    );
                    assert!(runtime.operations.register_authorizer(policy).is_err());
                    runtime.stop().await.unwrap();

                    // Existing Explicit Anonymous Routes Have No New Registration Requirement.
                    let runtime = Runtime::<App>::builder((), ())
                        .register(Lanes::catalog())
                        .runtime()
                        .await
                        .unwrap();
                    runtime
                        .operations
                        .register_wire_operation::<Guard>("app", Lanes::Open, Arc::new(()))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_service(Endpoint {
                            authority: harmos::grpc::server::Authority::TrustedHost(
                                runtime.operations.clone(),
                            ),
                        })
                        .unwrap();
                    let attached = runtime.grpc().attach(settings()).await.unwrap();
                    let channel = tonic::transport::Endpoint::from_shared(format!(
                        "http://{}",
                        attached.local_addr()
                    ))
                    .unwrap()
                    .connect()
                    .await
                    .unwrap();
                    assert_eq!(
                        external_call(channel, None, "/fixture.Identity/Guarded")
                            .await
                            .unwrap(),
                        "external:anonymous"
                    );
                    runtime.stop().await.unwrap();
                    let runtime = Runtime::<App>::builder((), ())
                        .register(Lanes::catalog())
                        .runtime()
                        .await
                        .unwrap();
                    runtime
                        .operations
                        .register_wire_operation::<Guard>("app", Lanes::Open, Arc::new(()))
                        .unwrap();
                    let operations = Arc::new(runtime.operations.clone());
                    runtime
                        .operations
                        .register_authorizer(Arc::new(ExternalAccess {
                            operations: Arc::downgrade(&operations),
                            mode: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                        }))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_service(Endpoint {
                            authority: harmos::grpc::server::Authority::TrustedHost(
                                runtime.operations.clone(),
                            ),
                        })
                        .unwrap();
                    let attached = runtime.grpc().attach(settings()).await.unwrap();
                    let channel = tonic::transport::Endpoint::from_shared(format!(
                        "http://{}",
                        attached.local_addr()
                    ))
                    .unwrap()
                    .connect()
                    .await
                    .unwrap();
                    assert_eq!(
                        external_call(channel, None, "/fixture.Identity/Guarded")
                            .await
                            .unwrap_err()
                            .code(),
                        tonic::Code::PermissionDenied
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }

    {
        scenario_runtime::check("empty_manager_attaches", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = Runtime::<App>::builder((), ())
                        .register(Lanes::catalog())
                        .runtime()
                        .await
                        .unwrap();
                    assert!(runtime.grpc().local_addr().is_none());
                    for round in 0..2 {
                        let attached = runtime.grpc().attach(settings()).await.unwrap();
                        let address = attached.local_addr();
                        assert_ne!(address.port(), 0);
                        assert_eq!(runtime.grpc().local_addr(), Some(address));
                        assert!(
                            runtime.grpc().attach(settings()).await.is_err(),
                            "one listener at a time"
                        );
                        let channel =
                            tonic::transport::Endpoint::from_shared(format!("http://{address}"))
                                .unwrap()
                                .connect()
                                .await
                                .unwrap();
                        assert_eq!(
                            call(channel.clone(), None).await.unwrap_err().code(),
                            tonic::Code::Unimplemented
                        );
                        if round == 0 {
                            attached.detach().await.unwrap();
                        } else {
                            drop(attached);
                        }
                        assert!(runtime.grpc().local_addr().is_none());
                        assert!(call(channel, None).await.is_err(), "round {round} released");
                    }
                    // A Dropped Listener Settles Before The Next One Binds The Same Address
                    let attached = runtime.grpc().attach(settings()).await.unwrap();
                    let address = attached.local_addr();
                    drop(attached);
                    let mut same = settings();
                    same.address = address;
                    let rebound = runtime.grpc().attach(same).await.unwrap();
                    assert_eq!(rebound.local_addr(), address);
                    runtime.stop().await.unwrap();
                    assert!(runtime.grpc().local_addr().is_none());
                    assert!(runtime.grpc().attach(settings()).await.is_err());
                });
        });
    }
    {
        scenario_runtime::check("stalled_listener_settles", || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    use harmos::grpc::OutputEnvelope;
                    let runtime = Runtime::<App>::builder((), ())
                        .register(Lanes::catalog())
                        .runtime()
                        .await
                        .unwrap();
                    let stopped = Arc::new(tokio::sync::Semaphore::new(0));
                    runtime
                        .operations
                        .register_operation::<Hold>("app", Lanes::Open, stopped.clone())
                        .unwrap();
                    runtime
                        .operations
                        .register_operation::<Read>("app", Lanes::Open, Arc::new(()))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions))))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_service(Endpoint {
                            authority: harmos::grpc::server::Authority::TrustedHost(
                                runtime.operations.clone(),
                            ),
                        })
                        .unwrap();
                    for abandoned in [false, true] {
                        let attached = runtime.grpc().attach(settings()).await.unwrap();
                        let address = attached.local_addr();
                        let channel =
                            tonic::transport::Endpoint::from_shared(format!("http://{address}"))
                                .unwrap()
                                .initial_connection_window_size(1)
                                .initial_stream_window_size(1)
                                .connect()
                                .await
                                .unwrap();
                        let mut client = tonic::client::Grpc::new(channel);
                        client.ready().await.unwrap();
                        let mut request =
                            tonic::Request::new(harmos::operations::Response::new(Ask {}));
                        request
                            .metadata_mut()
                            .insert("authorization", "Bearer alice".parse().unwrap());
                        let (decode, encode) = limits();
                        let mut unread = client
                            .server_streaming(
                                request,
                                http::uri::PathAndQuery::from_static("/fixture.Identity/Hold"),
                                Codec::<
                                    OutputEnvelope<Reply, Reply>,
                                    harmos::operations::Response<Ask>,
                                >::new(decode, encode)
                                .unwrap(),
                            )
                            .await
                            .unwrap()
                            .into_inner();
                        // Keep The One-Byte Window Stalled Throughout Listener Retirement.
                        let replacement = tokio::time::timeout(Duration::from_secs(2), async {
                            if abandoned {
                                drop(attached);
                            } else {
                                attached.detach().await.unwrap();
                                assert!(runtime.grpc().local_addr().is_none());
                            }
                            runtime.grpc().attach(settings()).await.unwrap()
                        })
                        .await
                        .expect("unread peer cannot hold listener retirement");
                        assert!(tokio::net::TcpStream::connect(address).await.is_err());
                        tokio::time::timeout(Duration::from_secs(1), async {
                            while let Ok(Some(_)) = unread.message().await {}
                            stopped.acquire().await.unwrap().forget();
                        })
                        .await
                        .expect("connection and original operation settle");
                        let channel = tonic::transport::Endpoint::from_shared(format!(
                            "http://{}",
                            replacement.local_addr()
                        ))
                        .unwrap()
                        .connect()
                        .await
                        .unwrap();
                        assert_eq!(call(channel, Some("Bearer alice")).await.unwrap(), "alice");
                        replacement.detach().await.unwrap();
                    }
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("shutdown_streams_unavailable", || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    use harmos::grpc::{InputEnvelope, OutputEnvelope};
                    let runtime = Runtime::<App>::builder((), ())
                        .register(Lanes::catalog())
                        .runtime()
                        .await
                        .unwrap();
                    runtime
                        .operations
                        .register_operation::<Hold>(
                            "app",
                            Lanes::Open,
                            Arc::new(tokio::sync::Semaphore::new(0)),
                        )
                        .unwrap();
                    runtime
                        .operations
                        .register_operation::<Upload>("app", Lanes::Open, Arc::new(()))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions))))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_service(Endpoint {
                            authority: harmos::grpc::server::Authority::TrustedHost(
                                runtime.operations.clone(),
                            ),
                        })
                        .unwrap();
                    let attached = runtime.grpc().attach(settings()).await.unwrap();
                    let channel = tonic::transport::Endpoint::from_shared(format!(
                        "http://{}",
                        attached.local_addr()
                    ))
                    .unwrap()
                    .connect()
                    .await
                    .unwrap();
                    let mut client = tonic::client::Grpc::new(channel);
                    let (decode, encode) = limits();
                    client.ready().await.unwrap();
                    let mut request =
                        tonic::Request::new(harmos::operations::Response::new(Ask {}));
                    request
                        .metadata_mut()
                        .insert("authorization", "Bearer alice".parse().unwrap());
                    let mut held =
                        client
                            .server_streaming(
                                request,
                                http::uri::PathAndQuery::from_static("/fixture.Identity/Hold"),
                                Codec::<
                                    OutputEnvelope<Reply, Reply>,
                                    harmos::operations::Response<Ask>,
                                >::new(decode, encode)
                                .unwrap(),
                            )
                            .await
                            .unwrap()
                            .into_inner();
                    assert!(held.message().await.unwrap().is_some());
                    let mut uploader = client.clone();
                    let upload = tokio::spawn(async move {
                        uploader.ready().await.unwrap();
                        // The Upload Stays Open Until The Listener Settles It.
                        let opened = futures_util::StreamExt::chain(
                            futures_util::stream::iter([harmos::operations::Response::new(
                                InputEnvelope::<Ask, Reply>::Request(Ask {}),
                            )]),
                            futures_util::stream::pending(),
                        );
                        let mut request = tonic::Request::new(opened);
                        request
                            .metadata_mut()
                            .insert("authorization", "Bearer alice".parse().unwrap());
                        uploader
                            .client_streaming(
                                request,
                                http::uri::PathAndQuery::from_static("/fixture.Identity/Upload"),
                                Codec::<
                                    Reply,
                                    harmos::operations::Response<InputEnvelope<Ask, Reply>>,
                                >::new(decode, encode)
                                .unwrap(),
                            )
                            .await
                            .err()
                            .expect("pending upload is settled")
                    });
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    // Bounded Below The Forced Settlement Grace, So Only Graceful Closure Passes.
                    let observed = tokio::time::timeout(Duration::from_millis(800), async {
                        let streamed = held.message().await.err().expect("held stream is settled");
                        (streamed, upload.await.unwrap())
                    });
                    let (shutdown, observed) = tokio::join!(
                        tokio::time::timeout(Duration::from_secs(3), runtime.grpc().shutdown()),
                        observed
                    );
                    shutdown.unwrap().unwrap();
                    let (streamed, uploaded) = observed.expect("streams settle gracefully");
                    assert_eq!(streamed.code(), tonic::Code::Unavailable, "{streamed:?}");
                    assert_eq!(uploaded.code(), tonic::Code::Unavailable, "{uploaded:?}");
                    drop(attached);
                    runtime.stop().await.unwrap();
                });
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("grpc_web_served", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let runtime = Runtime::<App>::builder((), ())
                        .register(Lanes::catalog())
                        .runtime()
                        .await
                        .unwrap();
                    let mut settings = settings();
                    settings.web = true;
                    let attached = runtime.grpc().attach(settings).await.unwrap();
                    let mut stream = tokio::net::TcpStream::connect(attached.local_addr())
                        .await
                        .unwrap();
                    stream
                        .write_all(
                            b"POST /missing.v1.Service/Call HTTP/1.1\r\nHost: harmos\r\n\
              Content-Type: application/grpc-web+proto\r\nX-Grpc-Web: 1\r\n\
              Content-Length: 5\r\nConnection: close\r\n\r\n\0\0\0\0\0",
                        )
                        .await
                        .unwrap();
                    let mut response = Vec::new();
                    tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut response))
                        .await
                        .unwrap()
                        .unwrap();
                    let response = String::from_utf8_lossy(&response).to_ascii_lowercase();
                    assert!(response.starts_with("http/1.1 200"), "{response}");
                    assert!(response.contains("application/grpc-web"), "{response}");
                    assert!(response.contains("grpc-status: 12"), "{response}");
                    drop(attached);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("grpc_web_cors", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    const ORIGIN: &str = "tauri://localhost";
                    const OTHER: &str = "https://other.example";
                    const ALLOW: &str = "access-control-allow-origin";
                    let runtime = Runtime::<App>::builder((), ())
                        .register(Lanes::catalog())
                        .runtime()
                        .await
                        .unwrap();
                    runtime
                        .operations
                        .register_operation::<Read>("app", Lanes::Open, Arc::new(()))
                        .unwrap();
                    runtime
                        .operations
                        .register_operation::<Download>("app", Lanes::Open, Arc::new(()))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions))))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_service(Endpoint {
                            authority: harmos::grpc::server::Authority::TrustedHost(
                                runtime.operations.clone(),
                            ),
                        })
                        .unwrap();
                    for invalid in [
                        "*",
                        "null",
                        "https://host/path",
                        "https://user@host",
                        "https://host?query",
                        "https://host#fragment",
                        "1https://host",
                    ] {
                        assert!(harmos::grpc::Origin::parse(invalid).is_err(), "{invalid}");
                    }
                    let mut disabled = settings();
                    disabled.allowed_origins = vec![ORIGIN.parse().unwrap()];
                    assert!(runtime.grpc().attach(disabled).await.is_err());
                    assert!(runtime.grpc().local_addr().is_none());
                    let mut cross = settings();
                    cross.web = true;
                    cross.allowed_origins = vec![ORIGIN.parse().unwrap()];
                    let attached = runtime.grpc().attach(cross).await.unwrap();
                    let address = attached.local_addr();
                    let preflight = |address, origin: &'static str| async move {
                        web(
                            address,
                            "OPTIONS",
                            "/fixture.Identity/Who",
                            &[
                                ("Origin", origin),
                                ("Access-Control-Request-Method", "POST"),
                                (
                                    "Access-Control-Request-Headers",
                                    "content-type,x-grpc-web,authorization",
                                ),
                            ],
                        )
                        .await
                    };
                    let accepted = preflight(address, ORIGIN).await;
                    assert!(accepted.starts_with("http/1.1 204"), "{accepted}");
                    assert!(
                        accepted.contains(&format!("{ALLOW}: {ORIGIN}")),
                        "{accepted}"
                    );
                    for header in [
                        "post",
                        "x-grpc-web",
                        "authorization",
                        "access-control-max-age",
                    ] {
                        assert!(accepted.contains(header), "{header} in {accepted}");
                    }
                    let refused = preflight(address, OTHER).await;
                    assert!(refused.starts_with("http/1.1 403"), "{refused}");
                    assert!(!refused.contains(ALLOW), "{refused}");
                    for path in ["/fixture.Identity/Who", "/fixture.Identity/Download"] {
                        let headers = |origin| {
                            [
                                ("Origin", origin),
                                ("Content-Type", "application/grpc-web+proto"),
                                ("X-Grpc-Web", "1"),
                                ("Authorization", "Bearer alice"),
                            ]
                        };
                        let served = web(address, "POST", path, &headers(ORIGIN)).await;
                        assert!(served.starts_with("http/1.1 200"), "{served}");
                        assert!(served.contains(&format!("{ALLOW}: {ORIGIN}")), "{served}");
                        assert!(served.contains("access-control-expose-headers"), "{served}");
                        let foreign = web(address, "POST", path, &headers(OTHER)).await;
                        assert!(foreign.starts_with("http/1.1 403"), "{foreign}");
                        assert!(!foreign.contains("grpc-status"), "{foreign}");
                    }
                    // Authentication Still Decides Actual Requests From Allowed Origins
                    let anonymous = web(
                        address,
                        "POST",
                        "/fixture.Identity/Who",
                        &[
                            ("Origin", ORIGIN),
                            ("Content-Type", "application/grpc-web+proto"),
                            ("X-Grpc-Web", "1"),
                        ],
                    )
                    .await;
                    assert!(anonymous.contains("grpc-status: 16"), "{anonymous}");
                    attached.detach().await.unwrap();
                    let mut plain_settings = settings();
                    plain_settings.web = true;
                    let attached = runtime.grpc().attach(plain_settings).await.unwrap();
                    let address = attached.local_addr();
                    let plain = web(
                        address,
                        "POST",
                        "/fixture.Identity/Who",
                        &[
                            ("Origin", OTHER),
                            ("Content-Type", "application/grpc-web+proto"),
                            ("X-Grpc-Web", "1"),
                            ("Authorization", "Bearer alice"),
                        ],
                    )
                    .await;
                    assert!(plain.starts_with("http/1.1 200"), "{plain}");
                    assert!(!plain.contains("access-control"), "{plain}");
                    let unlisted = preflight(address, ORIGIN).await;
                    assert!(!unlisted.starts_with("http/1.1 204"), "{unlisted}");
                    assert!(!unlisted.contains("access-control"), "{unlisted}");
                    drop(attached);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("status_propagation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let statuses = failures(|operations| {
                        operations
                            .register_operation::<Reject>("app", Lanes::Open, Arc::new(()))
                            .unwrap();
                        operations
                            .register_operation::<RejectStream>("app", Lanes::Open, Arc::new(()))
                            .unwrap();
                        operations
                            .register_operation::<RejectUpload>("app", Lanes::Open, Arc::new(()))
                            .unwrap();
                        operations
                            .register_operation::<RejectExchange>("app", Lanes::Open, Arc::new(()))
                            .unwrap();
                    })
                    .await;
                    for status in statuses {
                        assert_eq!(status.code(), tonic::Code::NotFound);
                        assert_eq!(status.message(), "no such actor");
                        assert_eq!(status.details(), REJECTION_DETAILS);
                        assert_eq!(
                            status.metadata().get("x-fixture-rejection").unwrap(),
                            "absent"
                        );
                        assert_eq!(
                            status
                                .metadata()
                                .get_bin("x-fixture-context-bin")
                                .unwrap()
                                .to_bytes()
                                .unwrap()
                                .as_ref(),
                            &[0, 255, 1]
                        );
                    }
                    let statuses = failures(|operations| {
                        operations
                            .register_operation::<Crash>("app", Lanes::Open, Arc::new(()))
                            .unwrap();
                        operations
                            .register_operation::<CrashStream>("app", Lanes::Open, Arc::new(()))
                            .unwrap();
                        operations
                            .register_operation::<CrashUpload>("app", Lanes::Open, Arc::new(()))
                            .unwrap();
                        operations
                            .register_operation::<CrashExchange>("app", Lanes::Open, Arc::new(()))
                            .unwrap();
                    })
                    .await;
                    for status in statuses {
                        assert_eq!(status.code(), tonic::Code::Internal);
                        assert!(matches!(
                            status.message(),
                            "operation request failed" | "operation input failed"
                        ));
                    }
                });
        });
    }
    {
        scenario_runtime::check("runtime_stops_listener", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = Runtime::<App>::builder((), ())
                        .register(Lanes::catalog())
                        .runtime()
                        .await
                        .unwrap();
                    assert!(runtime.grpc().local_addr().is_none());
                    runtime
                        .operations
                        .register_operation::<Read>("app", Lanes::Open, Arc::new(()))
                        .unwrap();
                    let trace = Arc::new(Mutex::new(Vec::new()));
                    runtime
                        .grpc()
                        .register_middleware(Arc::new(Trace(trace.clone())))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions))))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_service(Endpoint {
                            authority: harmos::grpc::server::Authority::TrustedHost(
                                runtime.operations.clone(),
                            ),
                        })
                        .unwrap();
                    let attached = runtime.grpc().attach(settings()).await.unwrap();
                    let address = attached.local_addr();
                    assert!(
                        runtime
                            .grpc()
                            .register_middleware(Arc::new(Trace(trace.clone())))
                            .is_err()
                    );
                    let channel =
                        tonic::transport::Endpoint::from_shared(format!("http://{address}"))
                            .unwrap()
                            .connect()
                            .await
                            .unwrap();
                    assert_eq!(
                        call(channel.clone(), None).await.unwrap_err().code(),
                        tonic::Code::Unauthenticated
                    );
                    let (alice, bob) = tokio::join!(
                        call(channel.clone(), Some("Bearer alice")),
                        call(channel.clone(), Some("Bearer bob"))
                    );
                    assert_eq!(alice.unwrap(), "alice");
                    assert_eq!(bob.unwrap(), "bob");
                    assert_eq!(
                        *trace.lock().unwrap(),
                        [
                            tonic::Code::Unauthenticated,
                            tonic::Code::Ok,
                            tonic::Code::Ok
                        ]
                    );
                    tokio::time::timeout(Duration::from_secs(5), runtime.stop())
                        .await
                        .unwrap()
                        .unwrap();
                    assert!(runtime.grpc().local_addr().is_none());
                    assert!(call(channel, Some("Bearer alice")).await.is_err());
                });
        });
    }
    {
        scenario_runtime::check("streaming_preserves_principals", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    use harmos::grpc::{InputEnvelope, OutputEnvelope};
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_operation::<Download>("app", Lanes::Open, Arc::new(()))
        .unwrap();
    runtime
        .operations
        .register_operation::<Upload>("app", Lanes::Open, Arc::new(()))
        .unwrap();
    runtime
        .operations
        .register_operation::<Exchange>("app", Lanes::Open, Arc::new(()))
        .unwrap();
    let trace = Arc::new(Mutex::new(Vec::new()));
    runtime
        .grpc()
        .register_middleware(Arc::new(Trace(trace.clone())))
        .unwrap();
    runtime
        .grpc()
        .register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions))))
        .unwrap();
    runtime
        .grpc()
        .register_service(Endpoint {
            authority: harmos::grpc::server::Authority::TrustedHost(runtime.operations.clone()),
        })
        .unwrap();
    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let address = attached.local_addr();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}"))
        .unwrap()
        .connect()
        .await
        .unwrap();
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();
    let (decode, encode) = limits();
    let mut request = tonic::Request::new(harmos::operations::Response::new(Ask {}));
    request
        .metadata_mut()
        .insert("authorization", "Bearer alice".parse().unwrap());
    let response = client
        .server_streaming(
            request,
            http::uri::PathAndQuery::from_static("/fixture.Identity/Download"),
            Codec::<OutputEnvelope<Reply, Reply>, harmos::operations::Response<Ask>>::new(decode, encode).unwrap(),
        )
        .await
        .unwrap();
    let mut stream = response.into_inner();
    assert!(
        matches!(&*stream.message().await.unwrap().unwrap(), OutputEnvelope::Item(Reply { actor }) if actor == "alice")
    );
    assert!(
        matches!(&*stream.message().await.unwrap().unwrap(), OutputEnvelope::Response(Reply { actor }) if actor == "alice")
    );
    assert!(stream.message().await.unwrap().is_none());
    for exchange in [false, true] {
        client.ready().await.unwrap();
        let values = [
            harmos::operations::Response::new(InputEnvelope::<Ask, Reply>::Request(Ask {})),
            harmos::operations::Response::new(InputEnvelope::Item(Reply {
                actor: "forged".into(),
            })),
        ];
        let mut request = tonic::Request::new(futures_util::stream::iter(values));
        request
            .metadata_mut()
            .insert("authorization", "Bearer bob".parse().unwrap());
        if exchange {
            let response = client
                .streaming(
                    request,
                    http::uri::PathAndQuery::from_static("/fixture.Identity/Exchange"),
                    Codec::<OutputEnvelope<Reply, Reply>, harmos::operations::Response<InputEnvelope<Ask, Reply>>>::new(
                        decode, encode,
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
            let mut stream = response.into_inner();
            assert!(
                matches!(&*stream.message().await.unwrap().unwrap(), OutputEnvelope::Item(Reply { actor }) if actor == "bob")
            );
            assert!(
                matches!(&*stream.message().await.unwrap().unwrap(), OutputEnvelope::Response(Reply { actor }) if actor == "bob")
            );
            assert!(stream.message().await.unwrap().is_none());
        } else {
            let response = client
                .client_streaming(
                    request,
                    http::uri::PathAndQuery::from_static("/fixture.Identity/Upload"),
                    Codec::<Reply, harmos::operations::Response<InputEnvelope<Ask, Reply>>>::new(decode, encode).unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.into_inner().actor, "bob");
        }
    }
    assert_eq!(*trace.lock().unwrap(), [tonic::Code::Ok; 3]);
    runtime.stop().await.unwrap();

});
        });
    }
    #[cfg(feature = "grpc")]
    {
        scenario_runtime::check("configured_tls_serves", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use tonic::transport::{
                        Certificate, ClientTlsConfig, Identity, ServerTlsConfig,
                    };
                    let runtime = Runtime::<App>::builder((), ())
                        .register(Lanes::catalog())
                        .runtime()
                        .await
                        .unwrap();
                    runtime
                        .operations
                        .register_operation::<Read>("app", Lanes::Open, Arc::new(()))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions))))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_service(Endpoint {
                            authority: harmos::grpc::server::Authority::TrustedHost(
                                runtime.operations.clone(),
                            ),
                        })
                        .unwrap();
                    let mut settings = settings();
                    settings.tls = Some(ServerTlsConfig::new().identity(Identity::from_pem(
                        include_bytes!("../../fixtures/grpc/server.pem"),
                        include_bytes!("../../fixtures/grpc/server-key.pem"),
                    )));
                    let attached = runtime.grpc().attach(settings).await.unwrap();
                    let address = attached.local_addr();
                    let channel =
                        tonic::transport::Endpoint::from_shared(format!("https://{address}"))
                            .unwrap()
                            .tls_config(
                                ClientTlsConfig::new()
                                    .domain_name("localhost")
                                    .ca_certificate(Certificate::from_pem(include_bytes!(
                                        "../../fixtures/grpc/ca.pem"
                                    ))),
                            )
                            .unwrap()
                            .connect()
                            .await
                            .unwrap();
                    assert_eq!(call(channel, Some("Bearer alice")).await.unwrap(), "alice");
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("observes_cancel_deadline", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::grpc::OutputEnvelope;
                    for expired in [false, true] {
                        let runtime = Runtime::<App>::builder((), ())
                            .register(Lanes::catalog())
                            .runtime()
                            .await
                            .unwrap();
                        let stopped = Arc::new(tokio::sync::Semaphore::new(0));
                        runtime
                            .operations
                            .register_operation::<Hold>("app", Lanes::Open, stopped.clone())
                            .unwrap();
                        let trace = Arc::new(Mutex::new(Vec::new()));
                        runtime
                            .grpc()
                            .register_middleware(Arc::new(Trace(trace.clone())))
                            .unwrap();
                        runtime
                            .grpc()
                            .register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions))))
                            .unwrap();
                        runtime
                            .grpc()
                            .register_service(Endpoint {
                                authority: harmos::grpc::server::Authority::TrustedHost(
                                    runtime.operations.clone(),
                                ),
                            })
                            .unwrap();
                        let mut settings = settings();
                        if expired {
                            settings.request_timeout = Duration::from_secs(1);
                        }
                        let attached = runtime.grpc().attach(settings).await.unwrap();
                        let address = attached.local_addr();
                        let channel =
                            tonic::transport::Endpoint::from_shared(format!("http://{address}"))
                                .unwrap()
                                .connect()
                                .await
                                .unwrap();
                        let mut client = tonic::client::Grpc::new(channel);
                        client.ready().await.unwrap();
                        let mut request =
                            tonic::Request::new(harmos::operations::Response::new(Ask {}));
                        request
                            .metadata_mut()
                            .insert("authorization", "Bearer alice".parse().unwrap());
                        let (decode, encode) = limits();
                        let response = client
                            .server_streaming(
                                request,
                                http::uri::PathAndQuery::from_static("/fixture.Identity/Hold"),
                                Codec::<
                                    OutputEnvelope<Reply, Reply>,
                                    harmos::operations::Response<Ask>,
                                >::new(decode, encode)
                                .unwrap(),
                            )
                            .await
                            .unwrap();
                        let mut stream = response.into_inner();
                        assert!(matches!(
                            &*stream.message().await.unwrap().unwrap(),
                            OutputEnvelope::Item(_)
                        ));
                        assert!(trace.lock().unwrap().is_empty());
                        if expired {
                            assert_eq!(
                                stream
                                    .message()
                                    .await
                                    .err()
                                    .expect("stream deadline must fail")
                                    .code(),
                                tonic::Code::DeadlineExceeded
                            );
                        }
                        drop(stream);
                        tokio::time::timeout(Duration::from_secs(1), stopped.acquire())
                            .await
                            .unwrap()
                            .unwrap()
                            .forget();
                        assert_eq!(
                            *trace.lock().unwrap(),
                            [if expired {
                                tonic::Code::DeadlineExceeded
                            } else {
                                tonic::Code::Cancelled
                            }]
                        );
                        runtime.stop().await.unwrap();
                    }
                });
        });
    }
    {
        scenario_runtime::check("authentication_bounded_operation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = Runtime::<App>::builder((), ())
                        .register(Lanes::catalog())
                        .runtime()
                        .await
                        .unwrap();
                    let trace = Arc::new(Mutex::new(Vec::new()));
                    runtime
                        .grpc()
                        .register_middleware(Arc::new(Trace(trace.clone())))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_middleware(Arc::new(Authenticate::new(Arc::new(
                            PendingAuthentication,
                        ))))
                        .unwrap();
                    // No Operation Is Registered: Authentication Must Terminate Before Dispatch.
                    runtime
                        .grpc()
                        .register_service(Endpoint {
                            authority: harmos::grpc::server::Authority::TrustedHost(
                                runtime.operations.clone(),
                            ),
                        })
                        .unwrap();
                    let mut settings = settings();
                    settings.request_timeout = Duration::from_millis(50);
                    let attached = runtime.grpc().attach(settings).await.unwrap();
                    let address = attached.local_addr();
                    let channel =
                        tonic::transport::Endpoint::from_shared(format!("http://{address}"))
                            .unwrap()
                            .connect()
                            .await
                            .unwrap();
                    let result = tokio::time::timeout(
                        Duration::from_secs(2),
                        call(channel, Some("Bearer alice")),
                    )
                    .await
                    .unwrap();
                    assert_eq!(result.unwrap_err().code(), tonic::Code::DeadlineExceeded);
                    assert_eq!(*trace.lock().unwrap(), [tonic::Code::DeadlineExceeded]);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("access_never_grant", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use tonic_reflection::pb::v1::{
                        ServerReflectionRequest, server_reflection_client::ServerReflectionClient,
                        server_reflection_request::MessageRequest,
                        server_reflection_response::MessageResponse,
                    };
                    let runtime = Runtime::<App>::builder((), ())
                        .register(Lanes::catalog())
                        .runtime()
                        .await
                        .unwrap();
                    runtime
                        .operations
                        .register_operation::<Read>("app", Lanes::Open, Arc::new(()))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions))))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_middleware(Arc::new(DiscoveryAccess))
                        .unwrap();
                    runtime
                        .grpc()
                        .register_service(Endpoint {
                            authority: harmos::grpc::server::Authority::TrustedHost(
                                runtime.operations.clone(),
                            ),
                        })
                        .unwrap();
                    let mut configured = settings();
                    configured.reflection = true;
                    let attached = runtime.grpc().attach(configured).await.unwrap();
                    let address = attached.local_addr();
                    let channel =
                        tonic::transport::Endpoint::from_shared(format!("http://{address}"))
                            .unwrap()
                            .connect()
                            .await
                            .unwrap();
                    let mut discovery = ServerReflectionClient::new(channel.clone());
                    let query = || {
                        futures_util::stream::iter([ServerReflectionRequest {
                            host: String::new(),
                            message_request: Some(MessageRequest::ListServices(String::new())),
                        }])
                    };
                    assert_eq!(
                        discovery
                            .server_reflection_info(query())
                            .await
                            .unwrap_err()
                            .code(),
                        tonic::Code::Unauthenticated
                    );
                    let mut request = tonic::Request::new(query());
                    request
                        .metadata_mut()
                        .insert("authorization", "Bearer bob".parse().unwrap());
                    let mut response = discovery
                        .server_reflection_info(request)
                        .await
                        .unwrap()
                        .into_inner();
                    let answer = response.message().await.unwrap().unwrap();
                    let Some(MessageResponse::ListServicesResponse(services)) =
                        answer.message_response
                    else {
                        panic!("authorized discovery returns a standard service inventory");
                    };
                    assert!(
                        services
                            .service
                            .iter()
                            .any(|service| service.name == "grpc.reflection.v1.ServerReflection")
                    );
                    assert!(response.message().await.unwrap().is_none());
                    assert_eq!(
                        call(channel.clone(), Some("Bearer bob"))
                            .await
                            .unwrap_err()
                            .code(),
                        tonic::Code::PermissionDenied
                    );
                    assert_eq!(call(channel, Some("Bearer alice")).await.unwrap(), "alice");
                    runtime.stop().await.unwrap();
                });
        });
    }
}
