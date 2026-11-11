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
async fn filesystem_hash_roundtrip() {
    use harmos_runtime::storage::{ByteRange, FileHash, FileSource, StorageManager};
    use sha2::{Digest, Sha256};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.bin");
    let bytes = b"host filesystem across grpc";
    std::fs::write(&path, bytes).unwrap();
    let settings = crate::storage::fixture::config();
    let backends = harmos_runtime::storage::StorageBackends {
        primary: Arc::new(crate::storage::fixture::Data::new(crate::storage::fixture::history())),
        state: Arc::new(crate::storage::fixture::Recovery::new(crate::storage::fixture::history())),
    };
    let storage: StorageManager = StorageManager::new(backends, settings).unwrap();
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.settings().storage(&storage);
        setup.operations::<harmos_operations::Storage>(Lanes::Open);
    })
    .await
    .unwrap();

    let attached = runtime.grpc().attach(super::settings()).await.unwrap();
    let address = format!("http://{}", attached.local_addr());
    let channel = tonic::transport::Endpoint::from_shared(address).unwrap().connect().await.unwrap();
    let request = harmos_operations::storage::HashFile {
        source: FileSource::Local(path),
        range: ByteRange { offset: 5, limit: Some(10) },
    };
    let answer = builtin_call::<harmos_operations::storage::HashFile, FileHash>(channel, "/harmos.storage.v1.Storage/HashFile", request)
        .await
        .unwrap();
    assert_eq!(answer.bytes_read, 10);
    assert_eq!(answer.sha256, <[u8; 32]>::from(Sha256::digest(&bytes[5..15])));
    drop(attached);
    storage.close().await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn builtin_grpc_roundtrip() {
    use harmos_operations::lifecycle::{Cancel, InstanceState, Status};

    #[harmos_runtime::operation(resources = [tokio::sync::Notify], response = (),
                        id = "grpc.parked",
                        policy = harmos_runtime::OperationPolicy {
                            timeout: Duration::from_secs(5),
                            attempt_timeout: Duration::from_secs(5),
                            cancellation_timeout: std::time::Duration::ZERO,
                            retries: RetryPolicy::Never,
                        },
                    )]
    struct ParkedContract {}
    impl harmos_runtime::Operation for ParkedContract {
        type Error = Error;

        async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Error> {
            let release = ctx.resource::<tokio::sync::Notify>();
            release.notified().await;
            Ok(().into())
        }
    }

    let release = Arc::new(tokio::sync::Notify::new());
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<tokio::sync::Notify>(release.clone());
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(16 * 1024).unwrap()),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    runtime.operations.register::<crate::operations::Single<ParkedContract>>(Lanes::Open).await.unwrap();
    runtime.operations.register::<harmos_operations::Lifecycle>(Lanes::Open).await.unwrap();
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
    let parked = runtime.operation(ParkedContract {}).start().await.unwrap();
    let id = parked.id();

    // An External Caller Neither Sees nor Cancels a Host Instance
    let status = generated_call::<harmos_protocol::host::Status, InstanceState>(
        channel.clone(),
        "/harmos.host.v1.Operations/Status",
        Status::of(id),
    )
    .await;
    assert!(status.is_err());
    let unknown = harmos_runtime::operations::OperationInstanceId::new(u64::MAX);
    let cancel = generated_call::<harmos_protocol::host::Cancel, InstanceState>(
        channel.clone(),
        "/harmos.host.v1.Operations/Cancel",
        Cancel::of(unknown),
    )
    .await;
    assert!(cancel.is_err());
    let foreign = generated_call::<harmos_protocol::host::Cancel, InstanceState>(
        channel.clone(),
        "/harmos.host.v1.Operations/Cancel",
        Cancel::of(id),
    )
    .await;
    assert_eq!(foreign.unwrap_err().code(), cancel.unwrap_err().code());
    assert!(!parked.status().cancellation_requested());

    // The Submitting Host Reads and Cancels Its Own Instance
    let owned = runtime.operations.call_protobuf::<Status>(Status::of(id)).await.unwrap();
    assert_eq!(owned.phase, "running");
    let cancelled = runtime.operations.call_protobuf::<Cancel>(Cancel::of(id)).await.unwrap();
    assert!(cancelled.cancellation_requested);
    assert!(parked.await.is_err());
    drop(attached);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn external_authority_parity() {
    use harmos_domain::plugin::Grant;
    use harmos_storage::{ErasedCatalog, StorageManager};

    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Option<Arc<StorageManager<ErasedCatalog>>>>(None);
    })
    .await
    .unwrap();
    runtime.operations.register::<crate::operations::Single<GuardedDeclaration>>(Lanes::Open).await.unwrap();
    runtime.operations.register::<crate::operations::Single<DescendantDeclaration>>(Lanes::Open).await.unwrap();
    runtime.operations.register::<harmos_operations::Host>(Lanes::Open).await.unwrap();
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
    let mut context = harmos_runtime::operations::CallContext::default();
    context.insert(Principal("bob".into())).unwrap();
    assert_eq!(
        runtime.operations.with_context(context).operation::<DescendantDeclaration>(DescendantDeclaration {}).await.unwrap().actor,
        "host:bob"
    );
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
    mode.store(3, std::sync::atomic::Ordering::Release);
    let incompatible = external_catalog(channel.clone(), Some("Bearer alice")).await.unwrap();
    assert!(incompatible.iter().all(|entry| entry.contract.operation().id.as_str() != "grpc.guarded"));
    let denied = external_call(channel.clone(), Some("Bearer alice"), "/fixture.Identity/Guarded").await.unwrap_err();
    assert_eq!(denied.code(), tonic::Code::PermissionDenied, "full control must validate the live contract");
    mode.store(4, std::sync::atomic::Ordering::Release);
    let full = external_catalog(channel.clone(), Some("Bearer limited")).await.unwrap();
    let guarded = full.iter().find(|entry| entry.contract.operation().id.as_str() == "grpc.guarded").unwrap();
    assert!(guarded.grants.is_empty(), "full control does not invent finite scopes");
    assert!(guarded.conditional_access);
    assert_eq!(external_call(channel.clone(), Some("Bearer limited"), "/fixture.Identity/Guarded").await.unwrap(), "external:limited");
    mode.store(0, std::sync::atomic::Ordering::Release);
    assert_eq!(external_call(channel.clone(), Some("Bearer alice"), "/fixture.Identity/Guarded").await.unwrap(), "external:alice");
    assert!(runtime.operations.register_authorizer(policy).is_err());
    runtime.shutdown().await.unwrap();

    // Existing Explicit Anonymous Routes Have No New Registration Requirement.
    let runtime = Runtime::<App>::launch((), ()).await.unwrap();
    runtime.operations.register::<crate::operations::Single<GuardedDeclaration>>(Lanes::Open).await.unwrap();
    runtime
        .grpc()
        .register_service(Endpoint {
            authority: harmos_runtime::grpc::server::Authority::trusted_host(&runtime.operations),
        })
        .unwrap();
    let attached = runtime.grpc().attach(settings()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    assert_eq!(external_call(channel, None, "/fixture.Identity/Guarded").await.unwrap(), "external:anonymous");
    runtime.shutdown().await.unwrap();
    let runtime = Runtime::<App>::launch((), ()).await.unwrap();
    runtime.operations.register::<crate::operations::Single<GuardedDeclaration>>(Lanes::Open).await.unwrap();
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
    runtime.shutdown().await.unwrap();
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
    runtime.shutdown().await.unwrap();
    assert!(runtime.grpc().local_addr().is_none());
    assert!(runtime.grpc().attach(settings()).await.is_err());
}

#[tokio::test]
async fn independent_listeners() {
    use tonic::transport::{Certificate, ClientTlsConfig, Identity, ServerTlsConfig};
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Behavior>(Behavior::read());
    })
    .await
    .unwrap();
    runtime.operations.register::<crate::operations::Single<WhoDeclaration>>(Lanes::Open).await.unwrap();
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
    use harmos_runtime::grpc::OutputEnvelope;
    let stopped = Arc::new(tokio::sync::Semaphore::new(0));
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Behavior>(Behavior::holding(stopped.clone()));
    })
    .await
    .unwrap();
    runtime.operations.register::<crate::operations::Single<HoldDeclaration>>(Lanes::Open).await.unwrap();
    runtime.operations.register::<crate::operations::Single<WhoDeclaration>>(Lanes::Open).await.unwrap();
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
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_streams_unavailable() {
    use harmos_runtime::grpc::{InputEnvelope, OutputEnvelope};
    let uploading = Arc::new(tokio::sync::Notify::new());
    let stopped = Arc::new(tokio::sync::Semaphore::new(0));
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Behavior>(Behavior::holding_and_uploading(stopped.clone(), uploading.clone()));
    })
    .await
    .unwrap();
    runtime.operations.register::<crate::operations::Single<HoldDeclaration>>(Lanes::Open).await.unwrap();
    runtime.operations.register::<crate::operations::Single<UploadDeclaration>>(Lanes::Open).await.unwrap();
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
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn status_propagation() {
    let statuses = failures(Behavior::reject(), |operations| async move {
        operations.register::<crate::operations::Single<WhoDeclaration>>(Lanes::Open).await.unwrap();
        operations.register::<crate::operations::Single<DownloadDeclaration>>(Lanes::Open).await.unwrap();
        operations.register::<crate::operations::Single<UploadDeclaration>>(Lanes::Open).await.unwrap();
        operations.register::<crate::operations::Single<ExchangeDeclaration>>(Lanes::Open).await.unwrap();
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
        operations.register::<crate::operations::Single<WhoDeclaration>>(Lanes::Open).await.unwrap();
        operations.register::<crate::operations::Single<DownloadDeclaration>>(Lanes::Open).await.unwrap();
        operations.register::<crate::operations::Single<UploadDeclaration>>(Lanes::Open).await.unwrap();
        operations.register::<crate::operations::Single<ExchangeDeclaration>>(Lanes::Open).await.unwrap();
    })
    .await;
    for status in statuses {
        assert_eq!(status.code(), tonic::Code::Internal);
        assert!(matches!(status.message(), "operation request failed" | "operation input failed"));
    }
}

#[tokio::test]
async fn runtime_stops_listener() {
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Behavior>(Behavior::read());
    })
    .await
    .unwrap();
    assert!(runtime.grpc().local_addr().is_none());
    runtime.operations.register::<crate::operations::Single<WhoDeclaration>>(Lanes::Open).await.unwrap();
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
    runtime.shutdown().await.unwrap();
    assert!(runtime.grpc().local_addr().is_none());
    assert!(call(channel, Some("Bearer alice")).await.is_err());
}

#[harmos_runtime::operation(resources = [Behavior, harmos_runtime::operations::HostContext], id = "grpc.download", response = Reply, output = Reply, policy = WHO_POLICY, visibility = Public, grpc(package = "fixture", service = "Identity", method = "Download"))]
struct NativeDownload {}
impl harmos_runtime::Operation for NativeDownload {
    type Error = Error;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
        let behavior = ctx.resource::<Behavior>();
        behavior.check(false)?;
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let output = ctx.streams();
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            let actor = context.context().require::<Principal>()?.0.clone();
            output.send_with(128, || Ok(Reply { actor: actor.clone() })).await?;
            Ok(Reply { actor })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

#[harmos_runtime::operation(resources = [Behavior, harmos_runtime::operations::HostContext], id = "grpc.upload", response = Reply, input = Reply, policy = WHO_POLICY, visibility = Public, grpc(package = "fixture", service = "Identity", method = "Upload"))]
struct NativeUpload {}
impl harmos_runtime::Operation for NativeUpload {
    type Error = Error;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
        let behavior = ctx.resource::<Behavior>();
        behavior.check(false)?;
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let input = ctx.streams();
        let result: ::core::result::Result<Reply, Self::Error> = async move {
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
}

#[harmos_runtime::operation(resources = [Behavior, harmos_runtime::operations::HostContext], id = "grpc.exchange", response = Reply, input = Reply, output = Reply, policy = WHO_POLICY, visibility = Public, grpc(package = "fixture", service = "Identity", method = "Exchange"))]
struct NativeExchange {}
impl harmos_runtime::Operation for NativeExchange {
    type Error = Error;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
        let behavior = ctx.resource::<Behavior>();
        behavior.check(true)?;
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let endpoints = ctx.streams();
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

#[tokio::test]
async fn streaming_preserves_principals() {
    streaming_principals(false).await;
}

#[tokio::test]
async fn native_batch_streaming_preserves_principals() {
    streaming_principals(true).await;
}

async fn streaming_principals(native: bool) {
    use harmos_runtime::grpc::{InputEnvelope, OutputEnvelope};
    let behavior = Arc::new(Behavior::uploading(Arc::new(tokio::sync::Notify::new())));
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Behavior>(behavior.clone());
    })
    .await
    .unwrap();
    if native {
        runtime.operations.register::<crate::operations::Single<NativeDownload>>(Lanes::Open).await.unwrap();
        runtime.operations.register::<crate::operations::Single<NativeUpload>>(Lanes::Open).await.unwrap();
        runtime.operations.register::<crate::operations::Single<NativeExchange>>(Lanes::Open).await.unwrap();
    } else {
        runtime.operations.register::<crate::operations::Single<DownloadDeclaration>>(Lanes::Open).await.unwrap();
        runtime.operations.register::<crate::operations::Single<UploadDeclaration>>(Lanes::Open).await.unwrap();
        runtime.operations.register::<crate::operations::Single<ExchangeDeclaration>>(Lanes::Open).await.unwrap();
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
    runtime.shutdown().await.unwrap();
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn configured_tls_serves() {
    use tonic::transport::{Certificate, ClientTlsConfig, Identity, ServerTlsConfig};
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Behavior>(Behavior::read());
    })
    .await
    .unwrap();
    runtime.operations.register::<crate::operations::Single<WhoDeclaration>>(Lanes::Open).await.unwrap();
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
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn observes_cancel_deadline() {
    use harmos_runtime::grpc::OutputEnvelope;
    #[harmos_runtime::operation(resources = [Behavior, harmos_runtime::operations::HostContext], id = "grpc.hold", response = Reply, output = Reply, policy = WHO_POLICY, visibility = Public, grpc(package = "fixture", service = "Identity", method = "Hold"))]
    struct NativeHold {}
    impl harmos_runtime::Operation for NativeHold {
        type Error = Error;
        async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::operations::Response<Reply>> {
            let behavior = ctx.resource::<Behavior>();
            behavior.check(false)?;
            let host = ctx.resource::<harmos_runtime::operations::HostContext>();
            let context = host.borrow();
            let output = ctx.streams();
            let _guard = HoldingGuard(behavior);
            let result: ::core::result::Result<Reply, Self::Error> = async move {
                let actor = context.context().require::<Principal>()?.0.clone();
                output.send_with(128, || Ok(Reply { actor })).await?;
                std::future::pending().await
            }
            .await;
            result.map(::core::convert::Into::into)
        }
    }

    for expired in [false, true] {
        let stopped = Arc::new(tokio::sync::Semaphore::new(0));
        let runtime = Runtime::<App>::launch_with((), (), |setup| {
            setup.resource::<Behavior>(Behavior::holding(stopped.clone()));
        })
        .await
        .unwrap();
        runtime.operations.register::<crate::operations::Single<NativeHold>>(Lanes::Open).await.unwrap();
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
            let status = stream.message().await.err().expect("stream deadline must fail");
            assert_eq!(status.code(), tonic::Code::DeadlineExceeded);
            assert!(status.message().contains("grpc.hold"), "{status}");
        }
        drop(stream);
        (stopped.acquire()).await.unwrap().forget();
        assert_eq!(*trace.lock().unwrap(), [if expired { tonic::Code::DeadlineExceeded } else { tonic::Code::Cancelled }]);
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn authentication_bounded_operation() {
    let runtime = Runtime::<App>::launch((), ()).await.unwrap();
    let trace = Arc::new(Mutex::new(Vec::new()));
    runtime.grpc().register_middleware(Arc::new(Trace(trace.clone()))).unwrap();
    runtime.grpc().register_middleware(Arc::new(Authenticate::new(Arc::new(PendingAuthentication)))).unwrap();
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
    let result = (call(channel, Some("Bearer alice"))).await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::DeadlineExceeded);
    assert_eq!(*trace.lock().unwrap(), [tonic::Code::DeadlineExceeded]);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn access_never_grant() {
    use tonic_reflection::pb::v1::{
        ServerReflectionRequest, server_reflection_client::ServerReflectionClient, server_reflection_request::MessageRequest,
        server_reflection_response::MessageResponse,
    };
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Behavior>(Behavior::read());
    })
    .await
    .unwrap();
    runtime.operations.register::<crate::operations::Single<WhoDeclaration>>(Lanes::Open).await.unwrap();
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
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn reload_certificate_preserves_stream() {
    use harmos_runtime::grpc::{InputEnvelope, OutputEnvelope, ServerCertificate};
    use harmos_runtime::operations::Response;
    use rustls::pki_types::ServerName;
    let first = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let second = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let certificate = ServerCertificate::from_pem(first.cert.pem().as_bytes(), first.signing_key.serialize_pem().as_bytes()).unwrap();
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Behavior>(Behavior::read());
    })
    .await
    .unwrap();
    runtime.operations.register::<crate::operations::Single<ExchangeDeclaration>>(Lanes::Open).await.unwrap();
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
    let codec =
        Codec::<harmos_runtime::grpc::Transcoded, OutputEnvelope<Reply, Reply>, Response<InputEnvelope<Ask, Reply>>>::new(decode, encode)
            .unwrap();
    let mut stream =
        client.streaming(request, http::uri::PathAndQuery::from_static("/fixture.Identity/Exchange"), codec).await.unwrap().into_inner();
    assert!(matches!(&*stream.message().await.unwrap().unwrap(), OutputEnvelope::Item(Reply { actor }) if actor == "alice"));

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
    assert!(matches!(&*stream.message().await.unwrap().unwrap(), OutputEnvelope::Item(Reply { actor }) if actor == "alice"));
    let socket = tokio::net::TcpStream::connect(address).await.unwrap();
    let connection = connector.connect(ServerName::try_from("localhost").unwrap(), socket).await.unwrap();
    assert_eq!(connection.get_ref().1.peer_certificates().unwrap()[0], *second.cert.der());
    drop(connection);
    drop(sender);
    assert!(matches!(&*stream.message().await.unwrap().unwrap(), OutputEnvelope::Response(Reply { actor }) if actor == "alice"));
    assert!(stream.message().await.unwrap().is_none());
    drop(stream);
    attached.detach().await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn native_stream_progress_outlives_listener_and_operation_totals() {
    use harmos_runtime::grpc::OutputEnvelope;
    #[harmos_runtime::operation(resources = [tokio::sync::Semaphore], id = "grpc.progress", response = u32, output = u32,
        policy = WHO_POLICY, visibility = Public, grpc(package = "fixture", service = "Deadlines", method = "Progress"))]
    struct Progress {}
    impl harmos_runtime::Operation for Progress {
        type Error = Error;
        async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::Response<u32>> {
            ctx.streams().send_with(4, || Ok(0)).await?;
            for value in 1..=4 {
                ctx.resource::<tokio::sync::Semaphore>().acquire().await.unwrap().forget();
                ctx.streams().send_with(4, || Ok(value)).await?;
            }
            Ok(4.into())
        }
    }
    let next = Arc::new(tokio::sync::Semaphore::new(0));
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<tokio::sync::Semaphore>(next.clone()).operations::<crate::operations::Single<Progress>>(Lanes::Open);
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
            Codec::<harmos_runtime::grpc::Transcoded, OutputEnvelope<u32, u32>, harmos_runtime::operations::Response<Progress>>::new(
                decode, encode,
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner();
    assert!(matches!(&*output.message().await.unwrap().unwrap(), OutputEnvelope::Item(0)));
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
        assert!(matches!(&*output.message().await.unwrap().unwrap(), OutputEnvelope::Item(item) if *item == value));
    }
    assert!(matches!(&*output.message().await.unwrap().unwrap(), OutputEnvelope::Response(4)));
    assert!(output.message().await.unwrap().is_none());
    awake.abort();
    let _ = awake.await;
    tokio::time::resume();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn settled_stream_drain_keeps_idle_deadline() {
    use harmos_runtime::grpc::OutputEnvelope;
    #[harmos_runtime::operation(resources = [tokio::sync::Notify], id = "grpc.buffered", response = (), output = Vec<u8>,
        policy = WHO_POLICY, visibility = Public, grpc(package = "fixture", service = "Deadlines", method = "Buffered"))]
    struct Buffered {}
    impl harmos_runtime::Operation for Buffered {
        type Error = Error;
        async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::Response<()>> {
            for _ in 0..32 {
                ctx.streams().send_with(128 * 1024 + 64, || Ok(vec![7; 128 * 1024])).await?;
            }
            ctx.resource::<tokio::sync::Notify>().notify_one();
            Ok(().into())
        }
    }
    let finished = Arc::new(tokio::sync::Notify::new());
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<tokio::sync::Notify>(finished.clone()).operations::<crate::operations::Single<Buffered>>(Lanes::Open);
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
            Codec::<harmos_runtime::grpc::Transcoded, OutputEnvelope<Vec<u8>, ()>, harmos_runtime::operations::Response<Buffered>>::new(
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
    assert!(matches!(&*output.message().await.unwrap().unwrap(), OutputEnvelope::Item(_)));
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
    runtime.shutdown().await.unwrap();
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
