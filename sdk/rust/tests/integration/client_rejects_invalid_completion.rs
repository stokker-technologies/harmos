/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Client Rejects Invalid Completion

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    convert::Infallible,
    num::NonZeroUsize,
    pin::Pin,
    task::{Context, Poll},
};

// Third Party Imports
use super::v1::{JobsStartRequest, JobsStartResponse};
use futures_core::Stream;
use harmos::{Client, ClientSettings, client::Method};
use harmos_protocol::v1::StorageTransferSettings;
use harmos_protocol::{schema, v1 as host};
use prost_reflect::ReflectMessage;
use tonic::{
    Request, Response, Status, Streaming,
    body::Body,
    codegen::{Service, http},
};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Frames = Pin<Box<dyn Stream<Item = Result<Frame, Status>> + Send>>;

/* === Structs === */

#[derive(Clone)]
struct Peer;
struct Invoke;

#[derive(Clone, PartialEq, prost::Message)]
struct Opening {
    #[prost(message, optional, tag = "1")]
    request: Option<JobsStartRequest>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct Frame {
    #[prost(message, optional, tag = "3")]
    unknown: Option<JobsStartResponse>,
    #[prost(message, optional, tag = "1")]
    item: Option<JobsStartResponse>,
    #[prost(message, optional, tag = "2")]
    response: Option<JobsStartResponse>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl tonic::server::NamedService for Peer {
    const NAME: &'static str = "fault.client.v1.Operations";
}

impl<B> Service<http::Request<B>> for Peer
where
    B: tonic::codegen::Body<Data = bytes::Bytes> + Send + 'static,
    B::Error: Into<tonic::codegen::StdError> + Send + 'static,
{
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = tonic::codegen::BoxFuture<Self::Response, Self::Error>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        Box::pin(async move {
            let codec = tonic_prost::ProstCodec::<Frame, Opening>::default();
            Ok(tonic::server::Grpc::new(codec).streaming(Invoke, request).await)
        })
    }
}

impl tonic::server::StreamingService<Opening> for Invoke {
    type Response = Frame;
    type ResponseStream = Frames;
    type Future = tonic::codegen::BoxFuture<Response<Frames>, Status>;
    fn call(&mut self, request: Request<Streaming<Opening>>) -> Self::Future {
        Box::pin(async move {
            let request = request
                .into_inner()
                .message()
                .await?
                .and_then(|opening| opening.request)
                .ok_or_else(|| Status::invalid_argument("opening required"))?;
            let terminal = || {
                Ok(Frame {
                    unknown: None,
                    item: None,
                    response: Some(JobsStartResponse::default()),
                })
            };
            let item = || {
                Ok(Frame {
                    unknown: None,
                    item: Some(JobsStartResponse::default()),
                    response: None,
                })
            };
            let frames = match request.value.parse::<i32>().unwrap() {
                0 => Vec::new(),
                1 => vec![terminal(), terminal()],
                2 => vec![terminal(), item()],
                3 => vec![item()],
                6 => vec![Ok(Frame {
                    unknown: Some(JobsStartResponse::default()),
                    item: None,
                    response: None,
                })],
                7 => vec![Ok(Frame {
                    unknown: None,
                    item: Some(JobsStartResponse::default()),
                    response: Some(JobsStartResponse::default()),
                })],
                5 => vec![Ok(Frame {
                    unknown: None,
                    item: None,
                    response: Some(JobsStartResponse { value: "x".repeat(2048) }),
                })],
                _ => vec![Err(Status::with_details(tonic::Code::PermissionDenied, "denied", bytes::Bytes::from_static(b"evidence")))],
            };
            Ok(Response::new(Box::pin(futures_util::stream::iter(frames)) as Frames))
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn client_rejects_invalid_completion_and_preserves_status_details() {
    // Connect To A Controlled Faulting Peer Through Real gRPC.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let incoming =
        futures_util::stream::unfold(listener, |listener| async { Some((listener.accept().await.map(|(stream, _)| stream), listener)) });
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(tonic::transport::Server::builder().add_service(Peer).serve_with_incoming_shutdown(incoming, async {
        let _ = stopped.await;
    }));
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
    let settings = ClientSettings {
        transfer: StorageTransferSettings { chunk_bytes: Some(256) },
        retained_bytes: NonZeroUsize::new(16 * 1024 * 1024).unwrap(),
        message_bytes: NonZeroUsize::new(1024).unwrap(),
        input_items: NonZeroUsize::new(1).unwrap(),
        input_bytes: NonZeroUsize::new(1024).unwrap(),
    };
    let client = Client::new(channel, settings).unwrap();
    let descriptors = JobsStartRequest::default().descriptor().parent_pool().encode_to_vec();
    let protobuf = host::Protobuf {
        descriptors,
        request: "application.v1.JobsStartRequest".into(),
        response: "application.v1.JobsStartResponse".into(),
        input: None,
        output: None,
    };
    let fingerprint = schema::host_fingerprint("fault.read", &protobuf).unwrap();
    let contract = host::DiscoveredContract {
        id: "fault.read".into(),
        visibility: host::Visibility::Public as i32,
        fingerprint,
        protobuf: Some(protobuf),
        grpc: Some(host::Grpc {
            package: "fault.client.v1".into(),
            service: "Operations".into(),
            method: "Read".into(),
            managed: true,
        }),
        ..Default::default()
    };
    let method = Method::<JobsStartRequest, JobsStartResponse>::new(contract).unwrap();

    // Refuse Missing, Duplicate, Post-Terminal And Undeclared Item Frames.
    for kind in 0..4 {
        let request = JobsStartRequest { value: kind.to_string() };
        let mut running = client.start(&method, Request::new(request)).await.unwrap();
        let error = running.next().await.unwrap_err();
        assert_eq!(error.code(), tonic::Code::DataLoss);
        assert_eq!(running.join().await.unwrap_err().code(), tonic::Code::DataLoss);
    }

    // Refuse Unknown Envelope Variants And Honor Protobuf's Last Oneof Variant.
    let running = client.start(&method, Request::new(JobsStartRequest { value: "6".into() })).await.unwrap();
    assert_eq!(running.join().await.unwrap_err().code(), tonic::Code::DataLoss);
    let running = client.start(&method, Request::new(JobsStartRequest { value: "7".into() })).await.unwrap();
    assert!(running.join().await.is_ok());

    // Preserve A Terminal gRPC Status And Its Binary Details.
    let running = client.start(&method, Request::new(JobsStartRequest { value: "4".into() })).await.unwrap();
    let error = running.join().await.unwrap_err();
    assert_eq!(error.code(), tonic::Code::PermissionDenied);
    assert_eq!(error.details(), b"evidence");

    // Refuse A Frame Larger Than The Consumer's Wire Budget.
    let running = client.start(&method, Request::new(JobsStartRequest { value: "5".into() })).await.unwrap();
    assert_eq!(running.join().await.unwrap_err().code(), tonic::Code::OutOfRange);

    // Close The Peer After Every Response Has Settled.
    stop.send(()).unwrap();
    server.await.unwrap().unwrap();
}
