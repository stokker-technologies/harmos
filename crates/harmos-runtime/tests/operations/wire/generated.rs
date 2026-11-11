/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    num::NonZeroUsize,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

// Third Party Imports
use prost_reflect::{DescriptorPool, MessageDescriptor, ReflectMessage};
use tonic::codegen::http::uri::PathAndQuery;

// Local Imports
use super::{Lanes, app::App};
use harmos_operations::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, ProtobufOutput, ServingError, Visibility};
use harmos_runtime::operations::{OperationPolicy, Registration, RetryPolicy};

/* === Constants === */

const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(5),
    attempt_timeout: Duration::from_secs(5),
    cancellation_timeout: Duration::ZERO,
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[rustfmt::skip]
#[path = "../../fixtures/protobuf/generated.rs"]
mod messages;
use messages::{Reply, Request};

struct Echo;
struct Watch;
struct Access(AtomicBool);

#[derive(Clone, PartialEq, prost::Message)]
struct Output {
    #[prost(oneof = "output::Value", tags = "1, 2")]
    value: Option<output::Value>,
}
mod output {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Value {
        #[prost(message, tag = "1")]
        Item(super::Reply),
        #[prost(message, tag = "2")]
        Response(super::Reply),
    }
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ReflectMessage for Request {
    fn descriptor(&self) -> MessageDescriptor {
        pool().get_message_by_name("generated.v1.Request").unwrap()
    }
}
impl ReflectMessage for Reply {
    fn descriptor(&self) -> MessageDescriptor {
        pool().get_message_by_name("generated.v1.Reply").unwrap()
    }
}

impl ProtobufOperation for Echo {
    type Request = Request;
    type Response = Reply;
    type Endpoints = ();
    const ID: &'static str = "generated.echo";
    const GRPC: Grpc = Grpc::new("generated.host.v1", "Operations", "Echo");
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    async fn run(_: ExecutionContext, request: Request, _: &()) -> Result<Reply, ServingError> {
        Ok(reply(request))
    }
}

impl ProtobufOperation for Watch {
    type Request = Request;
    type Response = Reply;
    type Endpoints = ProtobufOutput<Reply>;
    const ID: &'static str = "generated.watch";
    const GRPC: Grpc = Grpc::new("generated.host.v1", "Operations", "Watch");
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    async fn run(_: ExecutionContext, request: Request, output: &Self::Endpoints) -> Result<Reply, ServingError> {
        let response = reply(request);
        output.send(response.clone()).await.map_err(ServingError::Stream)?;
        Ok(response)
    }
}

impl harmos_runtime::operations::authorization::Authorizer for Access {
    fn authorize(
        &self, _: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Authorization>> {
        Ok(self.0.load(Ordering::SeqCst).then(|| harmos_runtime::operations::authorization::Authorization::Full(provider.contract.clone())))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn pool() -> &'static DescriptorPool {
    static POOL: OnceLock<DescriptorPool> = OnceLock::new();
    POOL.get_or_init(|| DescriptorPool::decode(include_bytes!("../../fixtures/protobuf/operations.bin").as_slice()).unwrap())
}
fn reply(request: Request) -> Reply {
    Reply {
        text: request.text,
        offset: request.offset,
        samples: request.samples,
        annotation: request.annotation,
    }
}
fn request() -> Request {
    Request {
        text: "protobuf".into(),
        offset: -128,
        samples: vec![0, 127, 128],
        annotation: Some(String::new()),
    }
}

/* === Tests === */

#[tokio::test]
async fn generated_messages_are_registered_reflected_and_admitted() {
    // Compile the Test Contract with the Pinned Compiler, Without Installed Protoc.
    let output = tempfile::tempdir().unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/protobuf");
    let mut compiler = prost_build::Config::new();
    let protoc = if std::env::var_os("STKR_TEST_PROFILE").is_some() {
        crate::support::fixtures::protoc()
    } else {
        protoc_bin_vendored::protoc_bin_path().unwrap()
    };
    compiler.protoc_executable(protoc);
    compiler.out_dir(output.path());
    compiler.file_descriptor_set_path(output.path().join("operations.bin"));
    compiler.compile_protos(&[source.join("operations.proto")], &[source]).unwrap();
    assert_eq!(std::fs::read(output.path().join("generated.v1.rs")).unwrap(), include_bytes!("../../fixtures/protobuf/generated.rs"));
    assert_eq!(std::fs::read(output.path().join("operations.bin")).unwrap(), include_bytes!("../../fixtures/protobuf/operations.bin"));

    // Register Generated Handlers and the Product Authorization Hook.
    let runtime = harmos_runtime::Runtime::<App>::launch((), ()).await.unwrap();
    runtime
        .operations
        .register_batch(Registration::new(Lanes::Open).protobuf::<Echo>().unwrap().protobuf::<Watch>().unwrap())
        .await
        .unwrap();
    let access = Arc::new(Access(AtomicBool::new(false)));
    runtime.operations.register_authorizer(access.clone()).unwrap();
    let settings = harmos_runtime::grpc::Settings {
        address: "127.0.0.1:0".parse().unwrap(),
        reflection: true,
        tls: None,
        connections: NonZeroUsize::new(4).unwrap(),
        calls_per_connection: NonZeroUsize::new(8).unwrap(),
        request_timeout: Duration::from_secs(5),
        shutdown_timeout: Duration::from_secs(5),
        operations: harmos_runtime::grpc::server::Settings {
            decode: harmos_runtime::grpc::DecodeSettings {
                message_bytes: NonZeroUsize::new(128 * 1024).unwrap(),
                retained_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
            },
            ..Default::default()
        },
    };
    let attached = runtime.grpc().attach(settings).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    let mut client = tonic::client::Grpc::new(channel.clone());
    client.ready().await.unwrap();

    // Refuse an Unauthorized Client Before Running the Handler.
    let denied = client
        .unary(
            tonic::Request::new(request()),
            PathAndQuery::from_static("/generated.host.v1.Operations/Echo"),
            tonic_prost::ProstCodec::<Request, Reply>::default(),
        )
        .await;
    assert_eq!(denied.unwrap_err().code(), tonic::Code::PermissionDenied);
    access.0.store(true, Ordering::SeqCst);

    // Reflect the Original Message Names, Signed Encoding and Presence.
    use tonic_reflection::pb::v1::{
        self as reflection, server_reflection_request::MessageRequest, server_reflection_response::MessageResponse,
    };
    let mut reflector = reflection::server_reflection_client::ServerReflectionClient::new(channel);
    let reflected = reflector
        .server_reflection_info(futures_util::stream::iter([reflection::ServerReflectionRequest {
            host: String::new(),
            message_request: Some(MessageRequest::FileContainingSymbol("generated.host.v1.Operations".into())),
        }]))
        .await
        .unwrap()
        .into_inner()
        .message()
        .await
        .unwrap()
        .unwrap();
    let Some(MessageResponse::FileDescriptorResponse(files)) = reflected.message_response else { panic!("expected descriptors") };
    let mut descriptors = prost_types::FileDescriptorSet::default();
    for file in files.file_descriptor_proto {
        descriptors.file.push(<prost_types::FileDescriptorProto as prost::Message>::decode(file.as_slice()).unwrap());
    }
    let reflected = DescriptorPool::from_file_descriptor_set(descriptors).unwrap();
    let service = reflected.get_service_by_name("generated.host.v1.Operations").unwrap();
    let method = service.methods().find(|method| method.name() == "Echo").unwrap();
    assert_eq!(method.input().full_name(), "generated.v1.Request");
    assert_eq!(method.input().get_field_by_name("offset").unwrap().kind(), prost_reflect::Kind::Sint64);
    assert!(method.input().get_field_by_name("annotation").unwrap().supports_presence());

    // Call Unary and Server-Streamed Operations Without a Rust Mirror.
    client.ready().await.unwrap();
    let response = client
        .unary(
            tonic::Request::new(request()),
            PathAndQuery::from_static("/generated.host.v1.Operations/Echo"),
            tonic_prost::ProstCodec::<Request, Reply>::default(),
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(response, reply(request()));
    client.ready().await.unwrap();
    let mut stream = client
        .server_streaming(
            tonic::Request::new(request()),
            PathAndQuery::from_static("/generated.host.v1.Operations/Watch"),
            tonic_prost::ProstCodec::<Request, Output>::default(),
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(stream.message().await.unwrap().unwrap().value, Some(output::Value::Item(reply(request()))));
    assert_eq!(stream.message().await.unwrap().unwrap().value, Some(output::Value::Response(reply(request()))));
    assert!(stream.message().await.unwrap().is_none());

    // Admit a Byte-Heavy Message That Fits the Decoding Budget.
    client.ready().await.unwrap();
    let bytes = Request {
        text: "x".repeat(4096),
        ..Default::default()
    };
    let expected = reply(bytes.clone());
    let response = client
        .unary(
            tonic::Request::new(bytes),
            PathAndQuery::from_static("/generated.host.v1.Operations/Echo"),
            tonic_prost::ProstCodec::<Request, Reply>::default(),
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(response, expected);

    // Refuse Decoding Memory While the Wire Payload Still Fits the Message Limit.
    client.ready().await.unwrap();
    let oversized = Request {
        text: "x".repeat(96 * 1024),
        ..Default::default()
    };
    let refused = client
        .unary(
            tonic::Request::new(oversized),
            PathAndQuery::from_static("/generated.host.v1.Operations/Echo"),
            tonic_prost::ProstCodec::<Request, Reply>::default(),
        )
        .await;
    assert_eq!(refused.unwrap_err().code(), tonic::Code::ResourceExhausted);

    // Shut Down the Listener and Host.
    drop(attached);
    runtime.shutdown().await.unwrap();
}
