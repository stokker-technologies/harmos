/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generated SDK Client Journey

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{num::NonZeroUsize, time::Duration};

// Third Party Imports
use harmos::{Client, ClientSettings, client::Method};
use harmos_protocol::host;
use tonic::{Request as RpcRequest, transport::Endpoint};

// Local Imports
use super::{
    app::App,
    generated::{
        Echo, Watch,
        messages::{Reply, Request},
    },
};
use harmos_runtime::operations::{
    ExecutionContext, Grpc, OperationPolicy, ProtobufInput, ProtobufOperation, ProtobufOutput, Registration, RetryPolicy, ServingError,
    Visibility,
};

/* === Constants === */

const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(30),
    attempt_timeout: Duration::from_secs(30),
    cancellation_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Collect;
struct Exchange;
struct Wait;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Collect {
    type Request = Request;
    type Response = Reply;
    type Endpoints = ProtobufInput<Request>;
    const ID: &'static str = "generated.collect";
    const GRPC: Option<Grpc> = Some(Grpc::new("generated.host.v1", "Operations", "Collect"));
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    async fn run(_: ExecutionContext, request: Request, input: &Self::Endpoints) -> Result<Reply, ServingError> {
        let mut response = Reply {
            offset: request.offset,
            ..Default::default()
        };
        while let Some(item) = input.recv().await {
            response.offset += item.offset;
            response.samples.extend_from_slice(&item.samples);
        }
        Ok(response)
    }
}

impl ProtobufOperation for Exchange {
    type Request = Request;
    type Response = Reply;
    type Endpoints = (ProtobufInput<Request>, ProtobufOutput<Reply>);
    const ID: &'static str = "generated.exchange";
    const GRPC: Option<Grpc> = Some(Grpc::new("generated.host.v1", "Operations", "Exchange"));
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    async fn run(_: ExecutionContext, request: Request, endpoints: &Self::Endpoints) -> Result<Reply, ServingError> {
        let mut total = request.offset;
        while let Some(item) = endpoints.0.recv().await {
            total += item.offset;
            endpoints
                .1
                .send(Reply {
                    offset: item.offset * 2,
                    samples: item.samples.clone(),
                    ..Default::default()
                })
                .await
                .map_err(ServingError::Stream)?;
        }
        Ok(Reply { offset: total, ..Default::default() })
    }
}

impl ProtobufOperation for Wait {
    type Request = Request;
    type Response = Reply;
    type Endpoints = ProtobufOutput<Reply>;
    const ID: &'static str = "generated.wait";
    const GRPC: Option<Grpc> = Some(Grpc::new("generated.host.v1", "Operations", "Wait"));
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    async fn run(context: ExecutionContext, request: Request, output: &Self::Endpoints) -> Result<Reply, ServingError> {
        output
            .send(Reply {
                text: request.text.clone(),
                ..Default::default()
            })
            .await
            .map_err(ServingError::Stream)?;
        context.cancellation.cancelled().await;
        Ok(Reply {
            text: request.text,
            offset: request.offset,
            ..Default::default()
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn generated_sdk_client_invokes_streams_and_settles_cancellation() {
    // Register The Application's Generated Contracts And Start Its Listener.
    let runtime = harmos_runtime::Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Option<std::sync::Arc<harmos_storage::StorageManager>>>(None);
    })
    .await
    .unwrap();
    let registration = Registration::new(harmos_runtime::operations::Lane::default())
        .protobuf::<harmos_operations::host::Catalog>()
        .unwrap()
        .protobuf::<Echo>()
        .unwrap()
        .protobuf::<Watch>()
        .unwrap()
        .protobuf::<Collect>()
        .unwrap()
        .protobuf::<Exchange>()
        .unwrap()
        .protobuf::<Wait>()
        .unwrap();
    runtime.operations.register_batch(registration).await.unwrap();
    let attached = runtime
        .grpc()
        .attach(harmos_runtime::grpc::Settings {
            address: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        })
        .await
        .unwrap();
    let channel = Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    let settings = ClientSettings {
        message_bytes: NonZeroUsize::new(128 * 1024).unwrap(),
        input_items: NonZeroUsize::new(1).unwrap(),
        input_bytes: NonZeroUsize::new(128 * 1024).unwrap(),
    };
    let client = Client::new(channel, settings).unwrap();

    // Discover And Bind The Exact Generated Application Types.
    let page = client
        .catalog(RpcRequest::new(host::Catalog {
            kind: host::CatalogKind::Operations as i32,
            rows: Some(64),
            bytes: Some(1024 * 1024),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();
    let contract = |id: &str| {
        page.operations.iter().filter_map(|operation| operation.contract.as_ref()).find(|operation| operation.id == id).unwrap().clone()
    };
    let echo = Method::<Request, Reply>::new(contract("generated.echo")).unwrap();
    let watch = Method::<Request, Reply>::new(contract("generated.watch")).unwrap().with_output::<Reply>().unwrap();
    let collect = Method::<Request, Reply>::new(contract("generated.collect")).unwrap().with_input::<Request>().unwrap();
    let exchange = Method::<Request, Reply>::new(contract("generated.exchange"))
        .unwrap()
        .with_input::<Request>()
        .unwrap()
        .with_output::<Reply>()
        .unwrap();
    let wait = Method::<Request, Reply>::new(contract("generated.wait")).unwrap().with_output::<Reply>().unwrap();

    // Refuse Mismatched Generated Schemas And Fingerprints Before Invocation.
    assert!(Method::<Reply, Request>::new(contract("generated.echo")).is_err());
    let mut forged = contract("generated.echo");
    forged.fingerprint[0] ^= 1;
    assert!(Method::<Request, Reply>::new(forged).is_err());
    let unbound = Method::<Request, Reply>::new(contract("generated.watch")).unwrap();
    assert!(client.start(&unbound, RpcRequest::new(Request::default())).await.is_err());

    // Preserve Signed Encoding, Repeated Scalars And Explicit Empty Presence.
    let request = Request {
        text: "generated".into(),
        offset: -128,
        samples: vec![0, 127, 128],
        annotation: Some(String::new()),
    };
    let expected = Reply {
        text: "generated".into(),
        offset: -128,
        samples: vec![0, 127, 128],
        annotation: Some(String::new()),
    };
    let response = client.call(&echo, RpcRequest::new(request.clone())).await.unwrap().into_inner();
    assert_eq!(response, expected);
    let handle = client.start(&echo, RpcRequest::new(request.clone())).await.unwrap();
    assert_eq!(handle.join().await.unwrap(), expected);

    // Receive Output Items And The Distinct Final Response.
    let mut handle = client.start(&watch, RpcRequest::new(request)).await.unwrap();
    assert_eq!(handle.next().await.unwrap().unwrap(), expected);
    assert!(handle.next().await.unwrap().is_none());
    assert_eq!(handle.join().await.unwrap(), expected);

    // Upload Input Items, Half-Close Input And Join The Computed Result.
    let mut handle = client.start(&collect, RpcRequest::new(Request { offset: 10, ..Default::default() })).await.unwrap();
    handle
        .send(Request {
            offset: -3,
            samples: vec![1, 2],
            ..Default::default()
        })
        .await
        .unwrap();
    handle
        .send(Request {
            offset: 5,
            samples: vec![3],
            ..Default::default()
        })
        .await
        .unwrap();
    handle.finish_input().await.unwrap();
    assert!(handle.send(Request::default()).await.is_err());
    let response = handle.join().await.unwrap();
    assert_eq!(response.offset, 12);
    assert_eq!(response.samples, [1, 2, 3]);

    // Exchange Items With A Single-Item Queue And Join After Input Completes.
    let mut handle = client.start(&exchange, RpcRequest::new(Request { offset: 7, ..Default::default() })).await.unwrap();
    handle
        .send(Request {
            offset: -2,
            samples: vec![4],
            ..Default::default()
        })
        .await
        .unwrap();
    let item = handle.next().await.unwrap().unwrap();
    assert_eq!(item.offset, -4);
    assert_eq!(item.samples, [4]);
    handle.send(Request { offset: 3, ..Default::default() }).await.unwrap();
    assert_eq!(handle.next().await.unwrap().unwrap().offset, 6);
    handle.finish_input().await.unwrap();
    assert_eq!(handle.join().await.unwrap().offset, 8);

    // Request Cancellation While Retaining The Server's Final Typed Response.
    let mut handle = client
        .start(
            &wait,
            RpcRequest::new(Request {
                text: "settled".into(),
                offset: 19,
                ..Default::default()
            }),
        )
        .await
        .unwrap();
    assert_eq!(handle.next().await.unwrap().unwrap().text, "settled");
    let response = handle.cancel().await.unwrap();
    assert_eq!(response.text, "settled");
    assert_eq!(response.offset, 19);

    // Refuse An Oversized Request Before It Enters The Outgoing Queue.
    let oversized = Request {
        text: "x".repeat(settings.message_bytes.get()),
        ..Default::default()
    };
    assert_eq!(client.call(&echo, RpcRequest::new(oversized)).await.unwrap_err().code(), tonic::Code::ResourceExhausted);

    // Close The Listener And Host.
    drop(attached);
    runtime.shutdown().await.unwrap();
}
