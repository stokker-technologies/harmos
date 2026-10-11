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

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Principal(String);
struct Sessions;
struct PendingAuthentication;
struct DiscoveryAccess;
struct Who;
struct Read;
struct Download;
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
    ) -> harmos::Result<Reply> {
        Ok(Reply {
            actor: context.context().require::<Principal>()?.0.clone(),
        })
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
    ) -> harmos::Result<Reply> {
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
    ) -> harmos::Result<Reply> {
        while input.recv().await.is_some() {}
        Ok(Reply {
            actor: context.context().require::<Principal>()?.0.clone(),
        })
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
    ) -> harmos::Result<Reply> {
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
    ) -> harmos::Result<Reply> {
        let actor = context.context().require::<Principal>()?.0.clone();
        output.send_with(128, || Ok(Reply { actor })).await?;
        std::future::pending().await
    }
    async fn stop(&mut self) -> harmos::Result<()> {
        self.stopped.add_permits(1);
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
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
        #[cfg(feature = "grpc-tls")]
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
    let mut request = tonic::Request::new(Arc::new(Ask {}));
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
            Codec::<Reply, Arc<Ask>>::new(decode, encode)?,
        )
        .await?;
    Ok(response.into_inner().actor.clone())
}

/* === Tests === */

pub(super) fn contract_scenario() {
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
    #[cfg(feature = "grpc-web")]
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
                    let attached = runtime.grpc().attach(settings()).await.unwrap();
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
    let mut request = tonic::Request::new(Arc::new(Ask {}));
    request
        .metadata_mut()
        .insert("authorization", "Bearer alice".parse().unwrap());
    let response = client
        .server_streaming(
            request,
            http::uri::PathAndQuery::from_static("/fixture.Identity/Download"),
            Codec::<OutputEnvelope<Reply, Reply>, Arc<Ask>>::new(decode, encode).unwrap(),
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
            Arc::new(InputEnvelope::<Ask, Reply>::Request(Ask {})),
            Arc::new(InputEnvelope::Item(Reply {
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
                    Codec::<OutputEnvelope<Reply, Reply>, Arc<InputEnvelope<Ask, Reply>>>::new(
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
                    Codec::<Reply, Arc<InputEnvelope<Ask, Reply>>>::new(decode, encode).unwrap(),
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
    #[cfg(feature = "grpc-tls")]
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
                        let mut request = tonic::Request::new(Arc::new(Ask {}));
                        request
                            .metadata_mut()
                            .insert("authorization", "Bearer alice".parse().unwrap());
                        let (decode, encode) = limits();
                        let response = client
                            .server_streaming(
                                request,
                                http::uri::PathAndQuery::from_static("/fixture.Identity/Hold"),
                                Codec::<OutputEnvelope<Reply, Reply>, Arc<Ask>>::new(
                                    decode, encode,
                                )
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
