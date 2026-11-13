/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Product Action Refusals Survive the Host; Stop Retains Its Safe Boundary.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

use super::{
    app::App,
    generated::messages::{Reply, Request},
};
use harmos_runtime::operations::{ExecutionContext, Grpc, OperationFactory, ProtobufOperation, Resource, ServingError, Visibility};
use prost::Message;
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Work {
    queued: AtomicBool,
    cancellations: AtomicUsize,
    stopping: tokio::sync::Notify,
    boundary: tokio::sync::Notify,
}
struct Cancel;
struct Stop;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Cancel {
    type Request = Request;
    type Response = Reply;
    type Endpoints = ();
    const ID: &'static str = "controls.cancel";
    const POLICY: harmos_runtime::OperationPolicy = harmos_operations::lifecycle::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const GRPC: Option<Grpc> = Some(Grpc::new("control.v1", "Work", "Cancel"));
    const RESOURCES: &'static [Resource] = &[Resource::of::<Work>()];
    async fn run(context: ExecutionContext, _: Request, _: &()) -> Result<Reply, ServingError> {
        let work = context.resource::<Work>().unwrap();
        if work.queued.load(Ordering::SeqCst) {
            return Err(ServingError::Handler(
                harmos_protocol::v1::Failure {
                    classification: harmos_protocol::v1::Classification::Refused as i32,
                    code: "queued_cancel".into(),
                    message: "Cancel Work: Target Is Queued".into(),
                    action: "Cancel After Starting Work".into(),
                    cause: Some(Box::new(harmos_protocol::v1::Failure {
                        code: "queued_target".into(),
                        ..Default::default()
                    })),
                    ..Default::default()
                }
                .into(),
            ));
        }
        work.cancellations.fetch_add(1, Ordering::SeqCst);
        Ok(Reply::default())
    }
}
impl ProtobufOperation for Stop {
    type Request = Request;
    type Response = Reply;
    type Endpoints = ();
    const ID: &'static str = "controls.stop";
    const POLICY: harmos_runtime::OperationPolicy = harmos_operations::lifecycle::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const GRPC: Option<Grpc> = Some(Grpc::new("control.v1", "Work", "Stop"));
    const RESOURCES: &'static [Resource] = &[Resource::of::<Work>()];
    async fn run(context: ExecutionContext, _: Request, _: &()) -> Result<Reply, ServingError> {
        let work = context.resource::<Work>().unwrap();
        work.queued.store(false, Ordering::SeqCst);
        work.stopping.notify_one();
        work.boundary.notified().await;
        Ok(Reply { offset: 42, ..Default::default() })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn client_preserves_queued_cancel_refusal_and_stop_waits_for_the_product_boundary() {
    let work = Arc::new(Work::default());
    work.queued.store(true, Ordering::SeqCst);
    let runtime = harmos_runtime::Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Work>(work.clone());
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: NonZeroUsize::new(4096),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    runtime.operations.register_catalog(harmos_operations::lifecycle_catalog(), Default::default()).await.unwrap();
    let cancel = OperationFactory::protobuf::<Cancel>().unwrap();
    let stop = OperationFactory::protobuf::<Stop>().unwrap();
    let cancel_contract = cancel.contract().unwrap();
    let stop_contract = stop.contract().unwrap();
    runtime
        .operations
        .register_batch(harmos_runtime::operations::Registration::new(Default::default()).entry(cancel).entry(stop))
        .await
        .unwrap();
    let listener = runtime
        .grpc()
        .attach(harmos_runtime::grpc::Settings {
            address: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        })
        .await
        .unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
    let mut client = harmos_protocol::v1::invocations_client::InvocationsClient::new(channel);
    let error = client
        .invoke(harmos_protocol::v1::InvocationsInvokeRequest {
            operation: Cancel::ID.into(),
            fingerprint: cancel_contract.fingerprint().0.to_vec(),
            payload: Request::default().encode_to_vec(),
        })
        .await
        .unwrap_err();
    let failure = harmos_protocol::v1::Failure::decode(error.details()).unwrap();
    assert_eq!(failure.classification(), harmos_protocol::v1::Classification::Refused);
    assert_eq!(failure.code, "queued_cancel");
    assert!(failure.message.contains("Cancel"));
    assert_eq!(failure.cause.unwrap().code, "queued_target");
    assert!(work.queued.load(Ordering::SeqCst));

    let stopped = tokio::spawn(async move {
        client
            .invoke(harmos_protocol::v1::InvocationsInvokeRequest {
                operation: Stop::ID.into(),
                fingerprint: stop_contract.fingerprint().0.to_vec(),
                payload: Request::default().encode_to_vec(),
            })
            .await
            .unwrap()
            .into_inner()
    });
    work.stopping.notified().await;
    assert_eq!(work.cancellations.load(Ordering::SeqCst), 0);
    assert!(!stopped.is_finished());
    work.boundary.notify_one();
    assert_eq!(Reply::decode(stopped.await.unwrap().payload.as_slice()).unwrap().offset, 42);
    assert_eq!(work.cancellations.load(Ordering::SeqCst), 0);
    drop(listener);
    runtime.shutdown().await.unwrap();
}
