/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise Network Authentication, Streaming And Listener Shutdown.

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;
use harmos_runtime::Runtime;

/* === Tests === */

#[tokio::test]
async fn builtin_grpc_roundtrip() {
    use harmos_operations::lifecycle::{Cancel, Get};

    #[derive(harmos_runtime::Message, Debug, Default)]
    #[harmos_path(harmos_runtime)]
    #[derive(harmos_runtime::Request)]
    #[request(response = ())]
    #[derive(Clone)]
    struct ParkedContract {}

    static PARKED_CONTRACT_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
        harmos_runtime::operations::WireDeclaration::request::<ParkedContract>(
            "grpc.parked",
            harmos_runtime::OperationPolicy {
                timeout: Duration::from_secs(5),
                attempt_timeout: Duration::from_secs(5),
                cancellation_timeout: std::time::Duration::ZERO,
                retries: RetryPolicy::Never,
            },
        );

    const PARKED_CONTRACT_OPERATION: harmos_runtime::operations::OperationSpec<ParkedContract> =
        harmos_runtime::operations::OperationSpec::<ParkedContract>::new(&PARKED_CONTRACT_DESCRIPTION);
    async fn parked_contract_handler(
        _request: ParkedContract, ctx: harmos_runtime::operations::FunctionContext<ParkedContract>,
    ) -> Result<harmos_runtime::operations::Response<()>, Error> {
        let release = ctx.resource::<tokio::sync::Notify>().expect("declared resource is registered");
        release.notified().await;
        Ok(().into())
    }

    fn parked_contract_operation()
    -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
        harmos_runtime::operations::OperationFactory::function(
            PARKED_CONTRACT_OPERATION,
            const { &[harmos_runtime::Resource::of::<tokio::sync::Notify>()] },
            parked_contract_handler,
            |_| harmos_runtime::operations::FailureKind::Permanent,
        )
    }

    let release = Arc::new(tokio::sync::Notify::new());
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<tokio::sync::Notify>(release.clone());
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(16 * 1024).unwrap()),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    runtime
        .operations
        .register_operation(parked_contract_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    crate::support::app::register(&runtime, harmos_operations::lifecycle_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let paths = runtime
        .operations
        .grpc_methods()
        .into_iter()
        .filter_map(|method| method.contract.grpc().map(|grpc| grpc.path()))
        .collect::<Vec<_>>();
    assert!(paths.contains(&"/harmos.client.v1.Invocations/Get".to_owned()));
    assert!(paths.contains(&"/harmos.client.v1.Invocations/Cancel".to_owned()));

    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    let parked = runtime.operation(ParkedContract {}).start().await.unwrap();
    let id = parked.id();

    // An External Caller Neither Sees nor Cancels a Host Instance
    let status = generated_call::<harmos_protocol::client::InvocationsGetRequest, harmos_protocol::client::InvocationsGetResponse>(
        channel.clone(),
        "/harmos.client.v1.Invocations/Get",
        Get::of(id),
    )
    .await;
    assert!(status.is_err());
    let unknown = harmos_runtime::operations::OperationInstanceId::new(u64::MAX);
    let cancel = generated_call::<harmos_protocol::client::InvocationsCancelRequest, harmos_protocol::client::InvocationsCancelResponse>(
        channel.clone(),
        "/harmos.client.v1.Invocations/Cancel",
        Cancel::of(unknown),
    )
    .await;
    assert!(cancel.is_err());
    let foreign = generated_call::<harmos_protocol::client::InvocationsCancelRequest, harmos_protocol::client::InvocationsCancelResponse>(
        channel.clone(),
        "/harmos.client.v1.Invocations/Cancel",
        Cancel::of(id),
    )
    .await;
    assert_eq!(foreign.unwrap_err().code(), cancel.unwrap_err().code());
    assert!(!parked.status().cancellation_requested());

    // The Submitting Host Reads and Cancels Its Own Instance
    let owned = runtime.operations.call_protobuf::<Get>(Get::of(id)).await.unwrap();
    assert_eq!(owned.invocation.unwrap().phase, "running");
    let cancelled = runtime.operations.call_protobuf::<Cancel>(Cancel::of(id)).await.unwrap();
    assert!(cancelled.invocation.unwrap().cancellation_requested);
    assert!(parked.await.is_err());
    drop(attached);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn external_authority_parity() {
    use harmos_storage::StorageManager;

    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<Option<Arc<StorageManager>>>(None);
    })
    .await
    .unwrap();
    runtime
        .operations
        .register_operation(guarded_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    runtime
        .operations
        .register_operation(
            descendant_declaration_operation().expect("representable operation"),
            harmos_runtime::operations::Lane::default(),
        )
        .await
        .unwrap();
    crate::support::app::register(&runtime, harmos_operations::host_catalog(), harmos_runtime::operations::Lane::default()).await.unwrap();
    // Internal Boot Work Does Not Freeze The Later Listener Policy.
    assert_eq!(runtime.operation(GuardedDeclaration {}).await.unwrap().actor, "host:anonymous");
    let mode = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let operations = Arc::new(runtime.operations.clone());
    let policy = Arc::new(ExternalAccess {
        operations: Arc::downgrade(&operations),
        mode: mode.clone(),
    });
    runtime.operations.register_authorizer(policy.clone()).unwrap();
    assert!(runtime.operations.register_authorizer(policy.clone()).is_err());
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint {
            authority: harmos_runtime::grpc::server::Authority::trusted_host(&runtime.operations),
        })
        .unwrap();
    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    assert_eq!(external_catalog(channel.clone(), None).await.unwrap_err().code(), tonic::Code::Unauthenticated);
    let alice = external_catalog(channel.clone(), Some("Bearer alice")).await.unwrap();
    let guarded = alice.iter().find(|entry| entry.contract.as_ref().unwrap().id.as_str() == "grpc.guarded").unwrap();
    assert_eq!(
        guarded.grants,
        [harmos_protocol::v1::Grant {
            operation: "grpc.guarded".into(),
            scope: Some("reading".into())
        }]
    );
    let bob = external_catalog(channel.clone(), Some("Bearer bob")).await.unwrap();
    assert!(bob.iter().all(|entry| entry.contract.as_ref().unwrap().id.as_str() != "grpc.guarded"));
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
    let mut context = harmos_runtime::operations::CallContext::default();
    context.insert(Principal("bob".into())).unwrap();
    assert_eq!(
        runtime.operations.with_context(context).operation::<DescendantDeclaration>(DescendantDeclaration {}).await.unwrap().actor,
        "host:bob"
    );
    assert_eq!(external_call(channel.clone(), Some("Bearer carol"), "/fixture.Identity/Guarded").await.unwrap(), "external:carol");
    mode.store(1, std::sync::atomic::Ordering::Release);
    assert!(
        external_catalog(channel.clone(), Some("Bearer alice")).await.unwrap().iter().all(|entry| entry
            .contract
            .as_ref()
            .unwrap()
            .id
            .as_str()
            != "grpc.guarded")
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
    mode.store(3, std::sync::atomic::Ordering::Release);
    let incompatible = external_catalog(channel.clone(), Some("Bearer alice")).await.unwrap();
    assert!(incompatible.iter().all(|entry| entry.contract.as_ref().unwrap().id.as_str() != "grpc.guarded"));
    let denied = external_call(channel.clone(), Some("Bearer alice"), "/fixture.Identity/Guarded").await.unwrap_err();
    assert_eq!(denied.code(), tonic::Code::PermissionDenied, "full control must validate the live contract");
    mode.store(4, std::sync::atomic::Ordering::Release);
    let full = external_catalog(channel.clone(), Some("Bearer limited")).await.unwrap();
    let guarded = full.iter().find(|entry| entry.contract.as_ref().unwrap().id.as_str() == "grpc.guarded").unwrap();
    assert!(guarded.grants.is_empty(), "full control does not invent finite scopes");
    assert!(guarded.conditional_access);
    assert_eq!(external_call(channel.clone(), Some("Bearer limited"), "/fixture.Identity/Guarded").await.unwrap(), "external:limited");
    mode.store(0, std::sync::atomic::Ordering::Release);
    assert_eq!(external_call(channel.clone(), Some("Bearer alice"), "/fixture.Identity/Guarded").await.unwrap(), "external:alice");
    assert!(runtime.operations.register_authorizer(policy).is_err());
    crate::support::app::shutdown(&runtime).await.unwrap();

    // Existing Explicit Anonymous Routes Have No New Registration Requirement.
    let runtime = Runtime::<App>::launch((), ()).await.unwrap();
    runtime
        .operations
        .register_operation(guarded_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    runtime
        .grpc()
        .register_service(Endpoint {
            authority: harmos_runtime::grpc::server::Authority::trusted_host(&runtime.operations),
        })
        .unwrap();
    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    assert_eq!(external_call(channel, None, "/fixture.Identity/Guarded").await.unwrap(), "external:anonymous");
    crate::support::app::shutdown(&runtime).await.unwrap();
    let runtime = Runtime::<App>::launch((), ()).await.unwrap();
    runtime
        .operations
        .register_operation(guarded_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
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
            authority: harmos_runtime::grpc::server::Authority::trusted_host(&runtime.operations),
        })
        .unwrap();
    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    assert_eq!(external_call(channel, None, "/fixture.Identity/Guarded").await.unwrap_err().code(), tonic::Code::PermissionDenied);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn empty_manager_attaches() {
    let runtime = Runtime::<App>::launch((), ()).await.unwrap();
    assert!(runtime.grpc().local_addr().is_none());
    for round in 0..2 {
        let attached = runtime.grpc().attach(settings()).await.unwrap();
        let address = attached.local_addr();
        assert_ne!(address.port(), 0);
        assert_eq!(runtime.grpc().local_addr(), Some(address));
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
    // A Dropped Listener Settles Before the Next One Binds the Same Address
    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let address = attached.local_addr();
    drop(attached);
    let mut same = settings();
    same.address = address;
    let rebound = runtime.grpc().attach(same).await.unwrap();
    assert_eq!(rebound.local_addr(), address);
    crate::support::app::shutdown(&runtime).await.unwrap();
    assert!(runtime.grpc().local_addr().is_none());
    assert!(runtime.grpc().attach(settings()).await.is_err());
}

#[tokio::test]
async fn independent_listeners() {
    use tonic::transport::{Certificate, ClientTlsConfig, Identity, ServerTlsConfig};
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<Behavior>(Behavior::read());
    })
    .await
    .unwrap();
    runtime
        .operations
        .register_operation(who_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint {
            authority: harmos_runtime::grpc::server::Authority::trusted_host(&runtime.operations),
        })
        .unwrap();
    let mut plain_settings = settings();
    plain_settings.connections = NonZeroUsize::new(1).unwrap();
    let mut tls_settings = settings();
    tls_settings.tls = Some(ServerTlsConfig::new().identity(Identity::from_pem(
        include_bytes!("../../fixtures/grpc/server.pem"),
        include_bytes!("../../fixtures/grpc/server-key.pem"),
    )));
    let (plain, secure) = tokio::join!(runtime.grpc().attach(plain_settings), runtime.grpc().attach(tls_settings));
    let plain = plain.unwrap();
    let secure = secure.unwrap();
    let address = plain.local_addr();
    assert_ne!(address, secure.local_addr());
    assert!(runtime.grpc().local_addr().is_none(), "multiple listeners have no singular address");

    let endpoint = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap();
    let plain_channel = endpoint.connect().await.unwrap();
    let secure_endpoint = tonic::transport::Endpoint::from_shared(format!("https://{}", secure.local_addr())).unwrap();
    let secure_endpoint = secure_endpoint
        .tls_config(
            ClientTlsConfig::new()
                .domain_name("localhost")
                .ca_certificate(Certificate::from_pem(include_bytes!("../../fixtures/grpc/ca.pem"))),
        )
        .unwrap();
    let secure_channel = secure_endpoint.connect().await.unwrap();
    assert_eq!(call(plain_channel.clone(), Some("Bearer alice")).await.unwrap(), "alice");
    assert_eq!(call(secure_channel.clone(), Some("Bearer bob")).await.unwrap(), "bob");

    // A Full Connection Budget And Failed Bind Leave The Sibling Usable.
    let waiting_channel = endpoint.connect().await.unwrap();
    let mut blocked = Box::pin(call(waiting_channel, Some("Bearer alice")));
    tokio::select! {
        biased;
        result = &mut blocked => panic!("the occupied listener admitted a call on another connection: {result:?}"),
        result = call(secure_channel.clone(), Some("Bearer alice")) => assert_eq!(result.unwrap(), "alice"),
    }
    let mut conflicting = settings();
    conflicting.address = address;
    assert!(runtime.grpc().attach(conflicting).await.is_err());
    assert_eq!(call(secure_channel.clone(), Some("Bearer alice")).await.unwrap(), "alice");
    assert_eq!(call(plain_channel.clone(), Some("Bearer alice")).await.unwrap(), "alice");

    // Detach And Rebind One Listener Without Closing The Other Connection.
    plain.detach().await.unwrap();
    assert!(blocked.await.is_err(), "detaching the saturated listener releases its waiting caller");
    assert!(call(plain_channel, Some("Bearer alice")).await.is_err());
    assert_eq!(runtime.grpc().local_addr(), Some(secure.local_addr()));
    let mut replacement = settings();
    replacement.address = address;
    let replacement = runtime.grpc().attach(replacement).await.unwrap();
    assert_eq!(replacement.local_addr(), address);
    assert_eq!(call(secure_channel.clone(), Some("Bearer bob")).await.unwrap(), "bob");
    drop(replacement);
    let mut replacement = settings();
    replacement.address = address;
    let replacement = runtime.grpc().attach(replacement).await.unwrap();
    let replacement_channel = endpoint.connect().await.unwrap();
    assert_eq!(call(replacement_channel.clone(), Some("Bearer alice")).await.unwrap(), "alice");

    // Shutdown Cancels Both Before Draining Their Independent Residents.
    runtime.shutdown().await.unwrap();
    assert!(call(replacement_channel, Some("Bearer alice")).await.is_err());
    assert!(call(secure_channel, Some("Bearer alice")).await.is_err());
    assert!(runtime.grpc().local_addr().is_none());
    assert!(runtime.grpc().attach(settings()).await.is_err());
    drop((secure, replacement));
}

#[tokio::test]
async fn stalled_listener_settles() {
    let stopped = Arc::new(tokio::sync::Semaphore::new(0));
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<Behavior>(Behavior::holding(stopped.clone()));
    })
    .await
    .unwrap();
    runtime
        .operations
        .register_operation(hold_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    runtime
        .operations
        .register_operation(who_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint {
            authority: harmos_runtime::grpc::server::Authority::trusted_host(&runtime.operations),
        })
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
                Codec::<harmos_runtime::grpc::Transcoded, Reply, harmos_runtime::operations::Response<Ask>>::new(decode, encode).unwrap(),
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
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn shutdown_streams_unavailable() {
    use harmos_runtime::grpc::InputEnvelope;
    let uploading = Arc::new(tokio::sync::Notify::new());
    let stopped = Arc::new(tokio::sync::Semaphore::new(0));
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<Behavior>(Behavior::holding_and_uploading(stopped.clone(), uploading.clone()));
    })
    .await
    .unwrap();
    runtime
        .operations
        .register_operation(hold_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    runtime
        .operations
        .register_operation(upload_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint {
            authority: harmos_runtime::grpc::server::Authority::trusted_host(&runtime.operations),
        })
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
            Codec::<harmos_runtime::grpc::Transcoded, Reply, harmos_runtime::operations::Response<Ask>>::new(decode, encode).unwrap(),
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
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn status_propagation() {
    let statuses = failures(Behavior::reject(), |operations| async move {
        operations
            .register_operation(who_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
            .await
            .unwrap();
        operations
            .register_operation(
                download_declaration_operation().expect("representable operation"),
                harmos_runtime::operations::Lane::default(),
            )
            .await
            .unwrap();
        operations
            .register_operation(
                upload_declaration_operation().expect("representable operation"),
                harmos_runtime::operations::Lane::default(),
            )
            .await
            .unwrap();
        operations
            .register_operation(
                exchange_declaration_operation().expect("representable operation"),
                harmos_runtime::operations::Lane::default(),
            )
            .await
            .unwrap();
    })
    .await;
    for status in statuses {
        assert_eq!(status.code(), tonic::Code::NotFound);
        assert_eq!(status.message(), "no such actor");
        assert_eq!(status.details(), REJECTION_DETAILS);
        assert_eq!(status.metadata().get("x-fixture-rejection").unwrap(), "absent");
        assert_eq!(status.metadata().get_bin("x-fixture-context-bin").unwrap().to_bytes().unwrap().as_ref(), &[0, 255, 1]);
    }
    let statuses = failures(Behavior::crash(), |operations| async move {
        operations
            .register_operation(who_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
            .await
            .unwrap();
        operations
            .register_operation(
                download_declaration_operation().expect("representable operation"),
                harmos_runtime::operations::Lane::default(),
            )
            .await
            .unwrap();
        operations
            .register_operation(
                upload_declaration_operation().expect("representable operation"),
                harmos_runtime::operations::Lane::default(),
            )
            .await
            .unwrap();
        operations
            .register_operation(
                exchange_declaration_operation().expect("representable operation"),
                harmos_runtime::operations::Lane::default(),
            )
            .await
            .unwrap();
    })
    .await;
    for status in statuses {
        assert_eq!(status.code(), tonic::Code::Internal);
        assert!(matches!(status.message(), "operation request failed" | "operation input failed"));
    }
}

#[tokio::test]
async fn runtime_stops_listener() {
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<Behavior>(Behavior::read());
    })
    .await
    .unwrap();
    assert!(runtime.grpc().local_addr().is_none());
    runtime
        .operations
        .register_operation(who_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let trace = Arc::new(Mutex::new(Vec::new()));
    runtime.grpc().register_middleware(Arc::new(Trace(trace.clone()))).unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint {
            authority: harmos_runtime::grpc::server::Authority::trusted_host(&runtime.operations),
        })
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
    crate::support::app::shutdown(&runtime).await.unwrap();
    assert!(runtime.grpc().local_addr().is_none());
    assert!(call(channel, Some("Bearer alice")).await.is_err());
}

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Reply, output = Reply)]
#[derive(Clone)]
struct NativeDownload {}

static NATIVE_DOWNLOAD_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<NativeDownload>("grpc.download", WHO_POLICY)
        .visibility(harmos_runtime::operations::Visibility::Public)
        .grpc(harmos_runtime::operations::Grpc::new("fixture", "Identity", "Download"));

const NATIVE_DOWNLOAD_OPERATION: harmos_runtime::operations::OperationSpec<NativeDownload> =
    harmos_runtime::operations::OperationSpec::<NativeDownload>::new(&NATIVE_DOWNLOAD_DESCRIPTION);
async fn native_download_handler(
    _request: NativeDownload, ctx: harmos_runtime::operations::FunctionContext<NativeDownload>,
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

fn native_download_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        NATIVE_DOWNLOAD_OPERATION,
        const { &[harmos_runtime::Resource::of::<Behavior>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        native_download_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Reply, input = Reply)]
#[derive(Clone)]
struct NativeUpload {}

static NATIVE_UPLOAD_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<NativeUpload>("grpc.upload", WHO_POLICY)
        .visibility(harmos_runtime::operations::Visibility::Public)
        .grpc(harmos_runtime::operations::Grpc::new("fixture", "Identity", "Upload"));

const NATIVE_UPLOAD_OPERATION: harmos_runtime::operations::OperationSpec<NativeUpload> =
    harmos_runtime::operations::OperationSpec::<NativeUpload>::new(&NATIVE_UPLOAD_DESCRIPTION);
async fn native_upload_handler(
    _request: NativeUpload, ctx: harmos_runtime::operations::FunctionContext<NativeUpload>,
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

fn native_upload_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        NATIVE_UPLOAD_OPERATION,
        const { &[harmos_runtime::Resource::of::<Behavior>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        native_upload_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Reply, input = Reply, output = Reply)]
#[derive(Clone)]
struct NativeExchange {}

static NATIVE_EXCHANGE_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<NativeExchange>("grpc.exchange", WHO_POLICY)
        .visibility(harmos_runtime::operations::Visibility::Public)
        .grpc(harmos_runtime::operations::Grpc::new("fixture", "Identity", "Exchange"));

const NATIVE_EXCHANGE_OPERATION: harmos_runtime::operations::OperationSpec<NativeExchange> =
    harmos_runtime::operations::OperationSpec::<NativeExchange>::new(&NATIVE_EXCHANGE_DESCRIPTION);
async fn native_exchange_handler(
    _request: NativeExchange, ctx: harmos_runtime::operations::FunctionContext<NativeExchange>,
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

fn native_exchange_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        NATIVE_EXCHANGE_OPERATION,
        const { &[harmos_runtime::Resource::of::<Behavior>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        native_exchange_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

#[tokio::test]
async fn streaming_preserves_principals() {
    streaming_principals(false).await;
}

#[tokio::test]
async fn native_batch_streaming_preserves_principals() {
    streaming_principals(true).await;
}

async fn streaming_principals(native: bool) {
    use harmos_runtime::grpc::InputEnvelope;
    let behavior = Arc::new(Behavior::uploading(Arc::new(tokio::sync::Notify::new())));
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<Behavior>(behavior.clone());
    })
    .await
    .unwrap();
    if native {
        runtime
            .operations
            .register_operation(native_download_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
            .await
            .unwrap();
        runtime
            .operations
            .register_operation(native_upload_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
            .await
            .unwrap();
        runtime
            .operations
            .register_operation(native_exchange_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
            .await
            .unwrap();
    } else {
        runtime
            .operations
            .register_operation(
                download_declaration_operation().expect("representable operation"),
                harmos_runtime::operations::Lane::default(),
            )
            .await
            .unwrap();
        runtime
            .operations
            .register_operation(
                upload_declaration_operation().expect("representable operation"),
                harmos_runtime::operations::Lane::default(),
            )
            .await
            .unwrap();
        runtime
            .operations
            .register_operation(
                exchange_declaration_operation().expect("representable operation"),
                harmos_runtime::operations::Lane::default(),
            )
            .await
            .unwrap();
    }
    let trace = Arc::new(Mutex::new(Vec::new()));
    runtime.grpc().register_middleware(Arc::new(Trace(trace.clone()))).unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    if !native {
        runtime
            .grpc()
            .register_service(Endpoint {
                authority: harmos_runtime::grpc::server::Authority::trusted_host(&runtime.operations),
            })
            .unwrap();
    }
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
            Codec::<harmos_runtime::grpc::Transcoded, Reply, harmos_runtime::operations::Response<Ask>>::new(decode, encode).unwrap(),
        )
        .await
        .unwrap();
    let mut stream = response.into_inner();
    assert!(matches!(&*stream.message().await.unwrap().unwrap(), Reply { actor } if actor == "alice"));
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
                    Codec::<harmos_runtime::grpc::Transcoded, Reply, harmos_runtime::operations::Response<InputEnvelope<Ask, Reply>>>::new(
                        decode, encode,
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
            let mut stream = response.into_inner();
            assert!(matches!(&*stream.message().await.unwrap().unwrap(), Reply { actor } if actor == "bob"));
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
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn configured_tls_serves() {
    use tonic::transport::{Certificate, ClientTlsConfig, Identity, ServerTlsConfig};
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<Behavior>(Behavior::read());
    })
    .await
    .unwrap();
    runtime
        .operations
        .register_operation(who_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint {
            authority: harmos_runtime::grpc::server::Authority::trusted_host(&runtime.operations),
        })
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
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn observes_cancel_deadline() {
    tokio::time::timeout(Duration::from_secs(10), async {
        #[derive(harmos_runtime::Message, Debug, Default)]
        #[harmos_path(harmos_runtime)]
        #[derive(harmos_runtime::Request)]
        #[request(response = Reply, output = Reply)]
        #[derive(Clone)]
        struct NativeHold {}

        static NATIVE_HOLD_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
            harmos_runtime::operations::WireDeclaration::request::<NativeHold>("grpc.hold", WHO_POLICY)
                .visibility(harmos_runtime::operations::Visibility::Public)
                .grpc(harmos_runtime::operations::Grpc::new("fixture", "Identity", "Hold"));

        const NATIVE_HOLD_OPERATION: harmos_runtime::operations::OperationSpec<NativeHold> =
            harmos_runtime::operations::OperationSpec::<NativeHold>::new(&NATIVE_HOLD_DESCRIPTION);
        async fn native_hold_handler(
            _request: NativeHold, ctx: harmos_runtime::operations::FunctionContext<NativeHold>,
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

        fn native_hold_operation()
        -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
            harmos_runtime::operations::OperationFactory::function(
                NATIVE_HOLD_OPERATION,
                const {
                    &[harmos_runtime::Resource::of::<Behavior>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()]
                },
                native_hold_handler,
                |_| harmos_runtime::operations::FailureKind::Permanent,
            )
        }

        for expired in [false, true] {
            let clock = crate::support::clock::Controlled::new();
            let stopped = Arc::new(tokio::sync::Semaphore::new(0));
            let runtime = crate::support::app::launch::<App>((), (), |setup| {
                setup.settings().clock(clock.source());
                setup.resource::<Behavior>(Behavior::holding(stopped.clone()));
            })
            .await
            .unwrap();
            runtime
                .operations
                .register_operation(native_hold_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
                .await
                .unwrap();
            let trace = Arc::new(Mutex::new(Vec::new()));
            runtime.grpc().register_middleware(Arc::new(Trace(trace.clone()))).unwrap();
            runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
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
                    Codec::<harmos_runtime::grpc::Transcoded, Reply, harmos_runtime::operations::Response<Ask>>::new(decode, encode)
                        .unwrap(),
                )
                .await
                .unwrap();
            let mut stream = response.into_inner();

            assert!(stream.message().await.unwrap().is_some());
            assert!(trace.lock().unwrap().is_empty());

            if expired {
                // Streaming Uses Its Operation Idle Budget After Authentication.
                clock.advance(WHO_POLICY.timeout);
                let status = stream.message().await.err().expect("stream deadline must fail");
                assert_eq!(status.code(), tonic::Code::DeadlineExceeded);
                assert!(status.message().contains("grpc.hold"), "{status}");
            }

            drop(stream);

            (stopped.acquire()).await.unwrap().forget();
            assert_eq!(*trace.lock().unwrap(), [if expired { tonic::Code::DeadlineExceeded } else { tonic::Code::Cancelled }]);

            crate::support::app::shutdown(&runtime).await.unwrap();
        }
    })
    .await
    .expect("listener story hang guard");
}

#[tokio::test]
async fn authentication_bounded_operation() {
    let clock = crate::support::clock::Controlled::new();
    let authenticating = Arc::new(tokio::sync::Notify::new());
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.settings().clock(clock.source());
    })
    .await
    .unwrap();
    let trace = Arc::new(Mutex::new(Vec::new()));
    runtime.grpc().register_middleware(Arc::new(Trace(trace.clone()))).unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(PendingAuthentication(authenticating.clone()))))).unwrap();
    // No Handler Is Registered: Authentication Must Terminate Before Dispatch.
    runtime
        .grpc()
        .register_service(Endpoint {
            authority: harmos_runtime::grpc::server::Authority::trusted_host(&runtime.operations),
        })
        .unwrap();
    let mut settings = settings();
    settings.request_timeout = Duration::from_millis(50);
    let attached = runtime.grpc().attach(settings).await.unwrap();
    let address = attached.local_addr();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
    let calling = call(channel, Some("Bearer alice"));
    tokio::pin!(calling);
    assert!(crate::support::events::until(calling.as_mut(), authenticating.notified()).await.is_ok());
    clock.advance(Duration::from_millis(50));
    let result = calling.await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::DeadlineExceeded);
    assert_eq!(*trace.lock().unwrap(), [tonic::Code::DeadlineExceeded]);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn access_never_grant() {
    use tonic_reflection::pb::v1::{
        ServerReflectionRequest, server_reflection_client::ServerReflectionClient, server_reflection_request::MessageRequest,
        server_reflection_response::MessageResponse,
    };
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<Behavior>(Behavior::read());
    })
    .await
    .unwrap();
    runtime
        .operations
        .register_operation(who_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime.grpc().register_middleware(Arc::new(DiscoveryAccess)).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint {
            authority: harmos_runtime::grpc::server::Authority::trusted_host(&runtime.operations),
        })
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
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn reload_certificate_preserves_stream() {
    use harmos_runtime::grpc::{InputEnvelope, ServerCertificate};
    use harmos_runtime::operations::Response;
    use rustls::pki_types::ServerName;
    let first = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let second = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let certificate = ServerCertificate::from_pem(first.cert.pem().as_bytes(), first.signing_key.serialize_pem().as_bytes()).unwrap();
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<Behavior>(Behavior::read());
    })
    .await
    .unwrap();
    runtime
        .operations
        .register_operation(exchange_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(Sessions)))).unwrap();
    runtime
        .grpc()
        .register_service(Endpoint {
            authority: harmos_runtime::grpc::server::Authority::trusted_host(&runtime.operations),
        })
        .unwrap();
    let attached = runtime.grpc().attach_with_certificate(settings(), certificate.clone(), Duration::from_secs(2)).await.unwrap();
    let address = attached.local_addr();
    let channel = tonic::transport::Endpoint::from_shared(format!("https://{address}"))
        .unwrap()
        .tls_config(
            tonic::transport::ClientTlsConfig::new()
                .domain_name("localhost")
                .ca_certificate(tonic::transport::Certificate::from_pem(first.cert.pem())),
        )
        .unwrap()
        .connect()
        .await
        .unwrap();
    let (sender, receiver) = tokio::sync::mpsc::channel(4);
    sender.send(Response::new(InputEnvelope::<Ask, Reply>::Request(Ask {}))).await.unwrap();
    sender.send(Response::new(InputEnvelope::Item(Reply { actor: "before".into() }))).await.unwrap();
    let input = futures_util::stream::unfold(receiver, |mut receiver| async move { receiver.recv().await.map(|value| (value, receiver)) });
    let mut request = tonic::Request::new(input);
    request.metadata_mut().insert("authorization", "Bearer alice".parse().unwrap());
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();
    let (decode, encode) = limits();
    let codec = Codec::<harmos_runtime::grpc::Transcoded, Reply, Response<InputEnvelope<Ask, Reply>>>::new(decode, encode).unwrap();
    let mut stream =
        client.streaming(request, http::uri::PathAndQuery::from_static("/fixture.Identity/Exchange"), codec).await.unwrap().into_inner();
    assert!(matches!(&*stream.message().await.unwrap().unwrap(), Reply { actor } if actor == "alice"));

    // Invalid And Mismatched Material Never Replaces The Active Identity.
    assert!(certificate.reload(b"invalid", b"invalid").is_err());
    assert!(certificate.reload(second.cert.pem().as_bytes(), first.signing_key.serialize_pem().as_bytes()).is_err());
    let mut roots = rustls::RootCertStore::empty();
    roots.add(first.cert.der().clone()).unwrap();
    roots.add(second.cert.der().clone()).unwrap();
    let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec()];
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    // A Failed Handshake Retires Only Its Socket, Never The Listening Service.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut malformed = tokio::net::TcpStream::connect(address).await.unwrap();
    malformed.write_all(b"plain text on a TLS socket").await.unwrap();
    let mut discarded = Vec::new();
    let _ = malformed.read_to_end(&mut discarded).await;
    drop(malformed);

    let socket = tokio::net::TcpStream::connect(address).await.unwrap();
    let connection = connector.connect(ServerName::try_from("localhost").unwrap(), socket).await.unwrap();
    assert_eq!(connection.get_ref().1.peer_certificates().unwrap()[0], *first.cert.der());
    drop(connection);

    certificate.reload(second.cert.pem().as_bytes(), second.signing_key.serialize_pem().as_bytes()).unwrap();
    sender.send(Response::new(InputEnvelope::Item(Reply { actor: "after".into() }))).await.unwrap();
    assert!(matches!(&*stream.message().await.unwrap().unwrap(), Reply { actor } if actor == "alice"));
    let socket = tokio::net::TcpStream::connect(address).await.unwrap();
    let connection = connector.connect(ServerName::try_from("localhost").unwrap(), socket).await.unwrap();
    assert_eq!(connection.get_ref().1.peer_certificates().unwrap()[0], *second.cert.der());
    drop(connection);
    drop(sender);
    assert!(stream.message().await.unwrap().is_none());
    drop(stream);
    attached.detach().await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn native_stream_progress_outlives_listener_and_operation_totals() {
    #[derive(harmos_runtime::Message, Debug, Default)]
    #[harmos_path(harmos_runtime)]
    #[derive(harmos_runtime::Request)]
    #[request(response = u32, output = u32)]
    #[derive(Clone)]
    struct Progress {}

    static PROGRESS_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
        harmos_runtime::operations::WireDeclaration::request::<Progress>("grpc.progress", WHO_POLICY)
            .visibility(harmos_runtime::operations::Visibility::Public)
            .grpc(harmos_runtime::operations::Grpc::new("fixture", "Deadlines", "Progress"));

    const PROGRESS_OPERATION: harmos_runtime::operations::OperationSpec<Progress> =
        harmos_runtime::operations::OperationSpec::<Progress>::new(&PROGRESS_DESCRIPTION);
    async fn progress_handler(
        _request: Progress, ctx: harmos_runtime::operations::FunctionContext<Progress>,
    ) -> harmos_runtime::Result<harmos_runtime::Response<u32>> {
        ctx.streams().send_with(4, || Ok(0)).await?;
        for value in 1..=4 {
            ctx.resource::<tokio::sync::Semaphore>().expect("declared resource is registered").acquire().await.unwrap().forget();
            ctx.streams().send_with(4, || Ok(value)).await?;
        }
        Ok(4.into())
    }

    fn progress_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError>
    {
        harmos_runtime::operations::OperationFactory::function(
            PROGRESS_OPERATION,
            const { &[harmos_runtime::Resource::of::<tokio::sync::Semaphore>()] },
            progress_handler,
            |_| harmos_runtime::operations::FailureKind::Permanent,
        )
    }
    let next = Arc::new(tokio::sync::Semaphore::new(0));
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup
            .resource::<tokio::sync::Semaphore>(next.clone())
            .operation(progress_operation().expect("representable operation"), harmos_runtime::operations::Lane::default());
    })
    .await
    .unwrap();
    let mut settings = settings();
    settings.request_timeout = Duration::from_secs(1);
    let attached = runtime.grpc().attach(settings).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();
    let (decode, encode) = limits();
    let mut output = client
        .server_streaming(
            tonic::Request::new(harmos_runtime::operations::Response::new(Progress {})),
            http::uri::PathAndQuery::from_static("/fixture.Deadlines/Progress"),
            Codec::<harmos_runtime::grpc::Transcoded, u32, harmos_runtime::operations::Response<Progress>>::new(decode, encode).unwrap(),
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(*output.message().await.unwrap().unwrap(), 0);
    tokio::time::pause();
    // Keep Socket Delivery From Auto-Advancing The Paused Clock.
    let awake = tokio::spawn(async {
        loop {
            tokio::task::yield_now().await;
        }
    });
    for value in 1..=4 {
        tokio::time::advance(Duration::from_millis(1500)).await;
        next.add_permits(1);
        assert_eq!(*output.message().await.unwrap().unwrap(), value);
    }
    assert!(output.message().await.unwrap().is_none());
    awake.abort();
    let _ = awake.await;
    tokio::time::resume();
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn settled_stream_drain_keeps_idle_deadline() {
    #[derive(harmos_runtime::Message, Debug, Default)]
    #[harmos_path(harmos_runtime)]
    #[derive(harmos_runtime::Request)]
    #[request(response = (), output = Vec<u8>)]
    #[derive(Clone)]
    struct Buffered {}

    static BUFFERED_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
        harmos_runtime::operations::WireDeclaration::request::<Buffered>("grpc.buffered", WHO_POLICY)
            .visibility(harmos_runtime::operations::Visibility::Public)
            .grpc(harmos_runtime::operations::Grpc::new("fixture", "Deadlines", "Buffered"));

    const BUFFERED_OPERATION: harmos_runtime::operations::OperationSpec<Buffered> =
        harmos_runtime::operations::OperationSpec::<Buffered>::new(&BUFFERED_DESCRIPTION);
    async fn buffered_handler(
        _request: Buffered, ctx: harmos_runtime::operations::FunctionContext<Buffered>,
    ) -> harmos_runtime::Result<harmos_runtime::Response<()>> {
        for _ in 0..32 {
            ctx.streams().send_with(128 * 1024 + 64, || Ok(vec![7; 128 * 1024])).await?;
        }
        ctx.resource::<tokio::sync::Notify>().expect("declared resource is registered").notify_one();
        Ok(().into())
    }

    fn buffered_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError>
    {
        harmos_runtime::operations::OperationFactory::function(
            BUFFERED_OPERATION,
            const { &[harmos_runtime::Resource::of::<tokio::sync::Notify>()] },
            buffered_handler,
            |_| harmos_runtime::operations::FailureKind::Permanent,
        )
    }
    let finished = Arc::new(tokio::sync::Notify::new());
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup
            .resource::<tokio::sync::Notify>(finished.clone())
            .operation(buffered_operation().expect("representable operation"), harmos_runtime::operations::Lane::default());
    })
    .await
    .unwrap();
    let mut settings = settings();
    let capacity = NonZeroUsize::new(2 * 1024 * 1024).unwrap();
    settings.operations.output = StreamSettings {
        items: NonZeroUsize::new(64).unwrap(),
        bytes: NonZeroUsize::new(8 * 1024 * 1024).unwrap(),
    };
    settings.operations.encode = EncodeSettings {
        message_bytes: capacity,
        buffer_bytes: capacity,
    };
    let attached = runtime.grpc().attach(settings).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr()))
        .unwrap()
        .initial_stream_window_size(Some(1024))
        .connect()
        .await
        .unwrap();
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();
    let mut output = client
        .server_streaming(
            tonic::Request::new(harmos_runtime::operations::Response::new(Buffered {})),
            http::uri::PathAndQuery::from_static("/fixture.Deadlines/Buffered"),
            Codec::<harmos_runtime::grpc::Transcoded, Vec<u8>, harmos_runtime::operations::Response<Buffered>>::new(
                DecodeSettings {
                    message_bytes: capacity,
                    retained_bytes: capacity,
                },
                EncodeSettings {
                    message_bytes: capacity,
                    buffer_bytes: capacity,
                },
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner();
    finished.notified().await;
    assert!(output.message().await.unwrap().is_some());
    tokio::time::pause();
    let awake = tokio::spawn(async {
        loop {
            tokio::task::yield_now().await;
        }
    });
    tokio::time::advance(Duration::from_secs(3)).await;
    let status = loop {
        match output.message().await {
            Err(status) => break status,
            Ok(Some(_)) => {}
            Ok(None) => panic!("a stalled buffered drain must expire"),
        }
    };
    assert_eq!(status.code(), tonic::Code::DeadlineExceeded);
    assert!(status.message().contains("grpc.buffered"), "{status}");
    awake.abort();
    let _ = awake.await;
    tokio::time::resume();
    crate::support::app::shutdown(&runtime).await.unwrap();
}

async fn generated_call<Q, R>(channel: tonic::transport::Channel, path: &'static str, request: Q) -> Result<R, tonic::Status>
where
    Q: prost::Message + Default + Send + Sync + 'static,
    R: prost::Message + Default + Send + Sync + 'static,
{
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.map_err(|_| tonic::Status::unavailable("channel"))?;
    client
        .unary(tonic::Request::new(request), http::uri::PathAndQuery::from_static(path), tonic_prost::ProstCodec::default())
        .await
        .map(tonic::Response::into_inner)
}
