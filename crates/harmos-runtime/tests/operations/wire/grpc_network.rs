/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise Network Authentication, Streaming And Listener Shutdown.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
#[tokio::test]
async fn builtin_grpc_roundtrip() {
    use harmos_runtime::builtin::{Cancel, InstanceState, Status};
    #[harmos_runtime::message]
    #[harmos_runtime::operation(
                        response = (),
                        id = "grpc.parked",
                        version = 1,
                        policy = OperationPolicy {
                            timeout: Duration::from_secs(5),
                            attempt_timeout: Duration::from_secs(5),
                            retries: RetryPolicy::Never,
                            restart: RestartPolicy::Never,
                        },
                    )]
    struct ParkedContract {}
    struct Parked(Arc<tokio::sync::Notify>);
    impl Operation for Parked {
        type Declaration = ParkedContract;
        type Resources = tokio::sync::Notify;
        type Error = Error;
        type Context<'a> = Context<'a>;
        type Endpoints = ();
        async fn initialize<'ctx>(release: Arc<tokio::sync::Notify>, _: Self::Context<'ctx>) -> Result<Self, Error> {
            Ok(Self(release))
        }
        async fn run<'ctx>(
            &mut self, _: Self::Context<'ctx>, _: &mut ParkedContract, _: &mut Self::Endpoints,
        ) -> Result<harmos_runtime::operations::Response<()>, Error> {
            self.0.notified().await;
            Ok(().into())
        }
    }

    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    let release = Arc::new(tokio::sync::Notify::new());
    runtime.operations.register_operation::<Parked>("app", Lanes::Open, release.clone()).unwrap();
    harmos_runtime::builtin::Catalog::register(&runtime.operations, "host", Lanes::Open).unwrap();
    let paths = runtime
        .operations
        .grpc_methods()
        .into_iter()
        .filter_map(|method| method.contract.grpc().map(|grpc| grpc.path()))
        .collect::<Vec<_>>();
    assert!(paths.contains(&"/harmos.host.v1.Operations/Status".to_owned()));
    assert!(paths.contains(&"/harmos.host.v1.Operations/Cancel".to_owned()));

    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    let parked = runtime.operations.submit(ParkedContract {}).unwrap();
    let id = parked.id();

    // An External Caller Neither Sees Nor Cancels A Host Instance
    let status = builtin_call::<Status, InstanceState>(channel.clone(), "/harmos.host.v1.Operations/Status", Status::of(id)).await;
    assert!(status.is_err());
    let unknown = harmos_runtime::operations::OperationInstanceId::new(u64::MAX);
    let cancel = builtin_call::<Cancel, InstanceState>(channel.clone(), "/harmos.host.v1.Operations/Cancel", Cancel::of(unknown)).await;
    assert!(cancel.is_err());
    let foreign = builtin_call::<Cancel, InstanceState>(channel.clone(), "/harmos.host.v1.Operations/Cancel", Cancel::of(id)).await;
    assert_eq!(foreign.unwrap_err().code(), cancel.unwrap_err().code());
    assert!(!parked.status().cancellation_requested());

    // The Submitting Host Reads And Cancels Its Own Instance
    let owned = runtime.operations.call(Status::of(id)).await.unwrap();
    assert_eq!(owned.phase, "running");
    let cancelled = runtime.operations.call(Cancel::of(id)).await.unwrap();
    assert!(cancelled.cancellation_requested);
    assert!(parked.await.is_err());
    drop(attached);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn external_authority_parity() {
    use harmos_domain::plugin::Grant;
    use harmos_runtime::operations::listing;
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime.operations.register_wire_operation::<Guard>("app", Lanes::Open, Arc::new(())).unwrap();
    runtime.operations.register_wire_operation::<Descend>("app", Lanes::Open, Arc::new(())).unwrap();
    listing::register::<()>(&runtime.operations, "host", Lanes::Open, None).unwrap();
    // Internal Boot Work Does Not Freeze The Later Listener Policy.
    assert_eq!(runtime.operations.call(Guarded {}).await.unwrap().actor, "host:anonymous");
    let mode = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let operations = Arc::new(runtime.operations.clone());
    let policy = Arc::new(ExternalAccess { operations: Arc::downgrade(&operations), mode: mode.clone() });
    runtime.operations.register_authorizer(policy.clone()).unwrap();
    assert!(runtime.operations.register_authorizer(policy.clone()).is_err());
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint { authority: harmos_runtime::grpc::server::Authority::TrustedHost(runtime.operations.clone()) })
        .unwrap();
    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    assert_eq!(external_catalog(channel.clone(), None).await.unwrap_err().code(), tonic::Code::Unauthenticated);
    let alice = external_catalog(channel.clone(), Some("Bearer alice")).await.unwrap();
    let guarded = alice.iter().find(|entry| entry.contract.operation().id.as_str() == "grpc.guarded").unwrap();
    assert_eq!(guarded.grants, [Grant::of("grpc.guarded", "reading")]);
    let bob = external_catalog(channel.clone(), Some("Bearer bob")).await.unwrap();
    assert!(bob.iter().all(|entry| entry.contract.operation().id.as_str() != "grpc.guarded"));
    assert_eq!(external_call(channel.clone(), Some("Bearer alice"), "/fixture.Identity/Guarded").await.unwrap(), "external:alice");
    assert_eq!(
        external_call(channel.clone(), Some("Bearer bob"), "/fixture.Identity/Guarded").await.unwrap_err().code(),
        tonic::Code::PermissionDenied
    );
    assert_eq!(
        external_call(channel.clone(), Some("Bearer limited"), "/fixture.Identity/Guarded").await.unwrap_err().code(),
        tonic::Code::PermissionDenied
    );
    assert_eq!(external_call(channel.clone(), Some("Bearer alice"), "/fixture.Identity/Descendant").await.unwrap(), "external:alice");
    assert_eq!(
        external_call(channel.clone(), Some("Bearer bob"), "/fixture.Identity/Descendant").await.unwrap_err().code(),
        tonic::Code::PermissionDenied
    );
    assert_eq!(external_call(channel.clone(), Some("Bearer alice"), "/fixture.Identity/Guarded").await.unwrap(), "external:alice");
    assert_eq!(external_call(channel.clone(), Some("Bearer carol"), "/fixture.Identity/Guarded").await.unwrap(), "external:carol");
    runtime.operations.clear_cache();
    let mut context = harmos_runtime::operations::CallContext::default();
    context.insert(Principal("bob".into())).unwrap();
    assert_eq!(runtime.operations.with_context(context).call(Descendant {}).await.unwrap().actor, "host:bob");
    assert_eq!(external_call(channel.clone(), Some("Bearer carol"), "/fixture.Identity/Guarded").await.unwrap(), "external:carol");
    mode.store(1, std::sync::atomic::Ordering::Release);
    assert!(
        external_catalog(channel.clone(), Some("Bearer alice"))
            .await
            .unwrap()
            .iter()
            .all(|entry| entry.contract.operation().id.as_str() != "grpc.guarded")
    );
    assert_eq!(
        external_call(channel.clone(), Some("Bearer alice"), "/fixture.Identity/Guarded").await.unwrap_err().code(),
        tonic::Code::PermissionDenied
    );
    mode.store(2, std::sync::atomic::Ordering::Release);
    assert_eq!(
        external_call(channel.clone(), Some("Bearer alice"), "/fixture.Identity/Guarded").await.unwrap_err().code(),
        tonic::Code::InvalidArgument
    );
    mode.store(0, std::sync::atomic::Ordering::Release);
    assert_eq!(external_call(channel.clone(), Some("Bearer alice"), "/fixture.Identity/Guarded").await.unwrap(), "external:alice");
    assert!(runtime.operations.register_authorizer(policy).is_err());
    runtime.stop().await.unwrap();

    // Existing Explicit Anonymous Routes Have No New Registration Requirement.
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime.operations.register_wire_operation::<Guard>("app", Lanes::Open, Arc::new(())).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint { authority: harmos_runtime::grpc::server::Authority::TrustedHost(runtime.operations.clone()) })
        .unwrap();
    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    assert_eq!(external_call(channel, None, "/fixture.Identity/Guarded").await.unwrap(), "external:anonymous");
    runtime.stop().await.unwrap();
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime.operations.register_wire_operation::<Guard>("app", Lanes::Open, Arc::new(())).unwrap();
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
        .register_service(Endpoint { authority: harmos_runtime::grpc::server::Authority::TrustedHost(runtime.operations.clone()) })
        .unwrap();
    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    assert_eq!(external_call(channel, None, "/fixture.Identity/Guarded").await.unwrap_err().code(), tonic::Code::PermissionDenied);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn empty_manager_attaches() {
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    assert!(runtime.grpc().local_addr().is_none());
    for round in 0..2 {
        let attached = runtime.grpc().attach(settings()).await.unwrap();
        let address = attached.local_addr();
        assert_ne!(address.port(), 0);
        assert_eq!(runtime.grpc().local_addr(), Some(address));
        assert!(runtime.grpc().attach(settings()).await.is_err(), "one listener at a time");
        let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
        assert_eq!(call(channel.clone(), None).await.unwrap_err().code(), tonic::Code::Unimplemented);
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
}

#[tokio::test]
async fn stalled_listener_settles() {
    use harmos_runtime::grpc::OutputEnvelope;
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    let stopped = Arc::new(tokio::sync::Semaphore::new(0));
    runtime.operations.register_operation::<Holding>("app", Lanes::Open, stopped.clone()).unwrap();
    runtime.operations.register_operation::<Read>("app", Lanes::Open, Arc::new(())).unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint { authority: harmos_runtime::grpc::server::Authority::TrustedHost(runtime.operations.clone()) })
        .unwrap();
    for abandoned in [false, true] {
        let attached = runtime.grpc().attach(settings()).await.unwrap();
        let address = attached.local_addr();
        let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}"))
            .unwrap()
            .initial_connection_window_size(1)
            .initial_stream_window_size(1)
            .connect()
            .await
            .unwrap();
        let mut client = tonic::client::Grpc::new(channel);
        client.ready().await.unwrap();
        let mut request = tonic::Request::new(harmos_runtime::operations::Response::new(Ask {}));
        request.metadata_mut().insert("authorization", "Bearer alice".parse().unwrap());
        let (decode, encode) = limits();
        let mut unread = client
            .server_streaming(
                request,
                http::uri::PathAndQuery::from_static("/fixture.Identity/Hold"),
                Codec::<harmos_runtime::grpc::Transcoded, OutputEnvelope<Reply, Reply>, harmos_runtime::operations::Response<Ask>>::new(
                    decode, encode,
                )
                .unwrap(),
            )
            .await
            .unwrap()
            .into_inner();
        // Keep The One-Byte Window Stalled Throughout Listener Retirement.
        let replacement = (async {
            if abandoned {
                drop(attached);
            } else {
                attached.detach().await.unwrap();
                assert!(runtime.grpc().local_addr().is_none());
            }
            runtime.grpc().attach(settings()).await.unwrap()
        })
        .await;
        assert!(tokio::net::TcpStream::connect(address).await.is_err());
        (async {
            while let Ok(Some(_)) = unread.message().await {}
            stopped.acquire().await.unwrap().forget();
        })
        .await;
        let channel =
            tonic::transport::Endpoint::from_shared(format!("http://{}", replacement.local_addr())).unwrap().connect().await.unwrap();
        assert_eq!(call(channel, Some("Bearer alice")).await.unwrap(), "alice");
        replacement.detach().await.unwrap();
    }
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn shutdown_streams_unavailable() {
    use harmos_runtime::grpc::{InputEnvelope, OutputEnvelope};
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime.operations.register_operation::<Holding>("app", Lanes::Open, Arc::new(tokio::sync::Semaphore::new(0))).unwrap();
    let uploading = Arc::new(tokio::sync::Notify::new());
    runtime.operations.register_operation::<Uploading>("app", Lanes::Open, uploading.clone()).unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint { authority: harmos_runtime::grpc::server::Authority::TrustedHost(runtime.operations.clone()) })
        .unwrap();
    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    let mut client = tonic::client::Grpc::new(channel);
    let (decode, encode) = limits();
    client.ready().await.unwrap();
    let mut request = tonic::Request::new(harmos_runtime::operations::Response::new(Ask {}));
    request.metadata_mut().insert("authorization", "Bearer alice".parse().unwrap());
    let mut held = client
        .server_streaming(
            request,
            http::uri::PathAndQuery::from_static("/fixture.Identity/Hold"),
            Codec::<harmos_runtime::grpc::Transcoded, OutputEnvelope<Reply, Reply>, harmos_runtime::operations::Response<Ask>>::new(
                decode, encode,
            )
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
            futures_util::stream::iter([harmos_runtime::operations::Response::new(InputEnvelope::<Ask, Reply>::Request(Ask {}))]),
            futures_util::stream::pending(),
        );
        let mut request = tonic::Request::new(opened);
        request.metadata_mut().insert("authorization", "Bearer alice".parse().unwrap());
        uploader
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
            .expect("pending upload is settled")
    });
    uploading.notified().await;
    // Both Calls Entered Their Bodies; Shutdown Must Settle Them Gracefully.
    let observed = async {
        let streamed = held.message().await.err().expect("held stream is settled");
        (streamed, upload.await.unwrap())
    };
    let (shutdown, (streamed, uploaded)) = tokio::join!(runtime.grpc().shutdown(), observed);
    shutdown.expect("graceful closure does not exhaust the shutdown policy");
    assert_eq!(streamed.code(), tonic::Code::Unavailable, "{streamed:?}");
    assert_eq!(uploaded.code(), tonic::Code::Unavailable, "{uploaded:?}");
    drop(attached);
    runtime.stop().await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn grpc_web_served() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    let mut settings = settings();
    settings.web = true;
    let attached = runtime.grpc().attach(settings).await.unwrap();
    let mut stream = tokio::net::TcpStream::connect(attached.local_addr()).await.unwrap();
    stream
        .write_all(
            b"POST /missing.v1.Service/Call HTTP/1.1\r\nHost: harmos\r\n\
              Content-Type: application/grpc-web+proto\r\nX-Grpc-Web: 1\r\n\
              Content-Length: 5\r\nConnection: close\r\n\r\n\0\0\0\0\0",
        )
        .await
        .unwrap();
    let mut response = Vec::new();
    (stream.read_to_end(&mut response)).await.unwrap();
    let response = String::from_utf8_lossy(&response).to_ascii_lowercase();
    assert!(response.starts_with("http/1.1 200"), "{response}");
    assert!(response.contains("application/grpc-web"), "{response}");
    assert!(response.contains("grpc-status: 12"), "{response}");
    drop(attached);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn grpc_web_cors() {
    const ORIGIN: &str = "tauri://localhost";
    const OTHER: &str = "https://other.example";
    const ALLOW: &str = "access-control-allow-origin";
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime.operations.register_operation::<Read>("app", Lanes::Open, Arc::new(())).unwrap();
    runtime.operations.register_operation::<Downloading>("app", Lanes::Open, Arc::new(())).unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint { authority: harmos_runtime::grpc::server::Authority::TrustedHost(runtime.operations.clone()) })
        .unwrap();
    for invalid in ["*", "null", "https://host/path", "https://user@host", "https://host?query", "https://host#fragment", "1https://host"] {
        assert!(harmos_runtime::grpc::Origin::parse(invalid).is_err(), "{invalid}");
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
                ("Access-Control-Request-Headers", "content-type,x-grpc-web,authorization"),
            ],
        )
        .await
    };
    let accepted = preflight(address, ORIGIN).await;
    assert!(accepted.starts_with("http/1.1 204"), "{accepted}");
    assert!(accepted.contains(&format!("{ALLOW}: {ORIGIN}")), "{accepted}");
    for header in ["post", "x-grpc-web", "authorization", "access-control-max-age"] {
        assert!(accepted.contains(header), "{header} in {accepted}");
    }
    let refused = preflight(address, OTHER).await;
    assert!(refused.starts_with("http/1.1 403"), "{refused}");
    assert!(!refused.contains(ALLOW), "{refused}");
    for path in ["/fixture.Identity/Who", "/fixture.Identity/Download"] {
        let headers = |origin| {
            [("Origin", origin), ("Content-Type", "application/grpc-web+proto"), ("X-Grpc-Web", "1"), ("Authorization", "Bearer alice")]
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
        &[("Origin", ORIGIN), ("Content-Type", "application/grpc-web+proto"), ("X-Grpc-Web", "1")],
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
        &[("Origin", OTHER), ("Content-Type", "application/grpc-web+proto"), ("X-Grpc-Web", "1"), ("Authorization", "Bearer alice")],
    )
    .await;
    assert!(plain.starts_with("http/1.1 200"), "{plain}");
    assert!(!plain.contains("access-control"), "{plain}");
    let unlisted = preflight(address, ORIGIN).await;
    assert!(!unlisted.starts_with("http/1.1 204"), "{unlisted}");
    assert!(!unlisted.contains("access-control"), "{unlisted}");
    drop(attached);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn status_propagation() {
    let statuses = failures(|operations| {
        operations.register_operation::<Reject>("app", Lanes::Open, Arc::new(())).unwrap();
        operations.register_operation::<RejectStream>("app", Lanes::Open, Arc::new(())).unwrap();
        operations.register_operation::<RejectUpload>("app", Lanes::Open, Arc::new(())).unwrap();
        operations.register_operation::<RejectExchange>("app", Lanes::Open, Arc::new(())).unwrap();
    })
    .await;
    for status in statuses {
        assert_eq!(status.code(), tonic::Code::NotFound);
        assert_eq!(status.message(), "no such actor");
        assert_eq!(status.details(), REJECTION_DETAILS);
        assert_eq!(status.metadata().get("x-fixture-rejection").unwrap(), "absent");
        assert_eq!(status.metadata().get_bin("x-fixture-context-bin").unwrap().to_bytes().unwrap().as_ref(), &[0, 255, 1]);
    }
    let statuses = failures(|operations| {
        operations.register_operation::<Crash>("app", Lanes::Open, Arc::new(())).unwrap();
        operations.register_operation::<CrashStream>("app", Lanes::Open, Arc::new(())).unwrap();
        operations.register_operation::<CrashUpload>("app", Lanes::Open, Arc::new(())).unwrap();
        operations.register_operation::<CrashExchange>("app", Lanes::Open, Arc::new(())).unwrap();
    })
    .await;
    for status in statuses {
        assert_eq!(status.code(), tonic::Code::Internal);
        assert!(matches!(status.message(), "operation request failed" | "operation input failed"));
    }
}

#[tokio::test]
async fn runtime_stops_listener() {
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    assert!(runtime.grpc().local_addr().is_none());
    runtime.operations.register_operation::<Read>("app", Lanes::Open, Arc::new(())).unwrap();
    let trace = Arc::new(Mutex::new(Vec::new()));
    runtime.grpc().register_middleware(Arc::new(Trace(trace.clone()))).unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint { authority: harmos_runtime::grpc::server::Authority::TrustedHost(runtime.operations.clone()) })
        .unwrap();
    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let address = attached.local_addr();
    assert!(runtime.grpc().register_middleware(Arc::new(Trace(trace.clone()))).is_err());
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
    assert_eq!(call(channel.clone(), None).await.unwrap_err().code(), tonic::Code::Unauthenticated);
    let (alice, bob) = tokio::join!(call(channel.clone(), Some("Bearer alice")), call(channel.clone(), Some("Bearer bob")));
    assert_eq!(alice.unwrap(), "alice");
    assert_eq!(bob.unwrap(), "bob");
    assert_eq!(*trace.lock().unwrap(), [tonic::Code::Unauthenticated, tonic::Code::Ok, tonic::Code::Ok]);
    (runtime.stop()).await.unwrap();
    assert!(runtime.grpc().local_addr().is_none());
    assert!(call(channel, Some("Bearer alice")).await.is_err());
}

#[tokio::test]
async fn streaming_preserves_principals() {
    use harmos_runtime::grpc::{InputEnvelope, OutputEnvelope};
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime.operations.register_operation::<Downloading>("app", Lanes::Open, Arc::new(())).unwrap();
    runtime.operations.register_operation::<Uploading>("app", Lanes::Open, Arc::new(tokio::sync::Notify::new())).unwrap();
    runtime.operations.register_operation::<Exchanging>("app", Lanes::Open, Arc::new(())).unwrap();
    let trace = Arc::new(Mutex::new(Vec::new()));
    runtime.grpc().register_middleware(Arc::new(Trace(trace.clone()))).unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint { authority: harmos_runtime::grpc::server::Authority::TrustedHost(runtime.operations.clone()) })
        .unwrap();
    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let address = attached.local_addr();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();
    let (decode, encode) = limits();
    let mut request = tonic::Request::new(harmos_runtime::operations::Response::new(Ask {}));
    request.metadata_mut().insert("authorization", "Bearer alice".parse().unwrap());
    let response = client
        .server_streaming(
            request,
            http::uri::PathAndQuery::from_static("/fixture.Identity/Download"),
            Codec::<harmos_runtime::grpc::Transcoded, OutputEnvelope<Reply, Reply>, harmos_runtime::operations::Response<Ask>>::new(
                decode, encode,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let mut stream = response.into_inner();
    assert!(matches!(&*stream.message().await.unwrap().unwrap(), OutputEnvelope::Item(Reply { actor }) if actor == "alice"));
    assert!(matches!(&*stream.message().await.unwrap().unwrap(), OutputEnvelope::Response(Reply { actor }) if actor == "alice"));
    assert!(stream.message().await.unwrap().is_none());
    for exchange in [false, true] {
        client.ready().await.unwrap();
        let values = [
            harmos_runtime::operations::Response::new(InputEnvelope::<Ask, Reply>::Request(Ask {})),
            harmos_runtime::operations::Response::new(InputEnvelope::Item(Reply { actor: "forged".into() })),
        ];
        let mut request = tonic::Request::new(futures_util::stream::iter(values));
        request.metadata_mut().insert("authorization", "Bearer bob".parse().unwrap());
        if exchange {
            let response = client
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
                .unwrap();
            let mut stream = response.into_inner();
            assert!(matches!(&*stream.message().await.unwrap().unwrap(), OutputEnvelope::Item(Reply { actor }) if actor == "bob"));
            assert!(matches!(&*stream.message().await.unwrap().unwrap(), OutputEnvelope::Response(Reply { actor }) if actor == "bob"));
            assert!(stream.message().await.unwrap().is_none());
        } else {
            let response = client
                .client_streaming(
                    request,
                    http::uri::PathAndQuery::from_static("/fixture.Identity/Upload"),
                    Codec::<harmos_runtime::grpc::Transcoded, Reply, harmos_runtime::operations::Response<InputEnvelope<Ask, Reply>>>::new(
                        decode, encode,
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.into_inner().actor, "bob");
        }
    }
    assert_eq!(*trace.lock().unwrap(), [tonic::Code::Ok; 3]);
    runtime.stop().await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn configured_tls_serves() {
    use tonic::transport::{Certificate, ClientTlsConfig, Identity, ServerTlsConfig};
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime.operations.register_operation::<Read>("app", Lanes::Open, Arc::new(())).unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint { authority: harmos_runtime::grpc::server::Authority::TrustedHost(runtime.operations.clone()) })
        .unwrap();
    let mut settings = settings();
    settings.tls = Some(ServerTlsConfig::new().identity(Identity::from_pem(
        include_bytes!("../../fixtures/grpc/server.pem"),
        include_bytes!("../../fixtures/grpc/server-key.pem"),
    )));
    let attached = runtime.grpc().attach(settings).await.unwrap();
    let address = attached.local_addr();
    let channel = tonic::transport::Endpoint::from_shared(format!("https://{address}"))
        .unwrap()
        .tls_config(
            ClientTlsConfig::new()
                .domain_name("localhost")
                .ca_certificate(Certificate::from_pem(include_bytes!("../../fixtures/grpc/ca.pem"))),
        )
        .unwrap()
        .connect()
        .await
        .unwrap();
    assert_eq!(call(channel, Some("Bearer alice")).await.unwrap(), "alice");
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn observes_cancel_deadline() {
    use harmos_runtime::grpc::OutputEnvelope;
    for expired in [false, true] {
        let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
        let stopped = Arc::new(tokio::sync::Semaphore::new(0));
        runtime.operations.register_operation::<Holding>("app", Lanes::Open, stopped.clone()).unwrap();
        let trace = Arc::new(Mutex::new(Vec::new()));
        runtime.grpc().register_middleware(Arc::new(Trace(trace.clone()))).unwrap();
        runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
        runtime
            .grpc()
            .register_service(Endpoint { authority: harmos_runtime::grpc::server::Authority::TrustedHost(runtime.operations.clone()) })
            .unwrap();
        let mut settings = settings();
        if expired {
            settings.request_timeout = Duration::from_secs(1);
        }
        let attached = runtime.grpc().attach(settings).await.unwrap();
        let address = attached.local_addr();
        let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
        let mut client = tonic::client::Grpc::new(channel);
        client.ready().await.unwrap();
        let mut request = tonic::Request::new(harmos_runtime::operations::Response::new(Ask {}));
        request.metadata_mut().insert("authorization", "Bearer alice".parse().unwrap());
        let (decode, encode) = limits();
        let response = client
            .server_streaming(
                request,
                http::uri::PathAndQuery::from_static("/fixture.Identity/Hold"),
                Codec::<harmos_runtime::grpc::Transcoded, OutputEnvelope<Reply, Reply>, harmos_runtime::operations::Response<Ask>>::new(
                    decode, encode,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let mut stream = response.into_inner();
        assert!(matches!(&*stream.message().await.unwrap().unwrap(), OutputEnvelope::Item(_)));
        assert!(trace.lock().unwrap().is_empty());
        if expired {
            assert_eq!(stream.message().await.err().expect("stream deadline must fail").code(), tonic::Code::DeadlineExceeded);
        }
        drop(stream);
        (stopped.acquire()).await.unwrap().forget();
        assert_eq!(*trace.lock().unwrap(), [if expired { tonic::Code::DeadlineExceeded } else { tonic::Code::Cancelled }]);
        runtime.stop().await.unwrap();
    }
}

#[tokio::test]
async fn authentication_bounded_operation() {
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    let trace = Arc::new(Mutex::new(Vec::new()));
    runtime.grpc().register_middleware(Arc::new(Trace(trace.clone()))).unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(PendingAuthentication)))).unwrap();
    // No Operation Is Registered: Authentication Must Terminate Before Dispatch.
    runtime
        .grpc()
        .register_service(Endpoint { authority: harmos_runtime::grpc::server::Authority::TrustedHost(runtime.operations.clone()) })
        .unwrap();
    let mut settings = settings();
    settings.request_timeout = Duration::from_millis(50);
    let attached = runtime.grpc().attach(settings).await.unwrap();
    let address = attached.local_addr();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
    let result = (call(channel, Some("Bearer alice"))).await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::DeadlineExceeded);
    assert_eq!(*trace.lock().unwrap(), [tonic::Code::DeadlineExceeded]);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn access_never_grant() {
    use tonic_reflection::pb::v1::{
        ServerReflectionRequest, server_reflection_client::ServerReflectionClient, server_reflection_request::MessageRequest,
        server_reflection_response::MessageResponse,
    };
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime.operations.register_operation::<Read>("app", Lanes::Open, Arc::new(())).unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime.grpc().register_middleware(Arc::new(DiscoveryAccess)).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint { authority: harmos_runtime::grpc::server::Authority::TrustedHost(runtime.operations.clone()) })
        .unwrap();
    let mut configured = settings();
    configured.reflection = true;
    let attached = runtime.grpc().attach(configured).await.unwrap();
    let address = attached.local_addr();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
    let mut discovery = ServerReflectionClient::new(channel.clone());
    let query = || {
        futures_util::stream::iter([ServerReflectionRequest {
            host: String::new(),
            message_request: Some(MessageRequest::ListServices(String::new())),
        }])
    };
    assert_eq!(discovery.server_reflection_info(query()).await.unwrap_err().code(), tonic::Code::Unauthenticated);
    let mut request = tonic::Request::new(query());
    request.metadata_mut().insert("authorization", "Bearer bob".parse().unwrap());
    let mut response = discovery.server_reflection_info(request).await.unwrap().into_inner();
    let answer = response.message().await.unwrap().unwrap();
    let Some(MessageResponse::ListServicesResponse(services)) = answer.message_response else {
        panic!("authorized discovery returns a standard service inventory");
    };
    assert!(services.service.iter().any(|service| service.name == "grpc.reflection.v1.ServerReflection"));
    assert!(response.message().await.unwrap().is_none());
    assert_eq!(call(channel.clone(), Some("Bearer bob")).await.unwrap_err().code(), tonic::Code::PermissionDenied);
    assert_eq!(call(channel, Some("Bearer alice")).await.unwrap(), "alice");
    runtime.stop().await.unwrap();
}
