/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Cancellation Final Response Proof

/* === Imports === */

// Standard Library Imports
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::app::App;
use harmos_runtime::operations::{OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Refusal, Runtime};

/* === Structs === */

#[derive(Default)]
pub(crate) struct Probe {
    pub(crate) entered: Notify,
    pub(crate) started: AtomicUsize,
    pub(crate) dropped: AtomicUsize,
}

struct Guard(Arc<Probe>);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = u64)]
#[derive(Clone)]
pub(crate) struct Finish {
    pub(crate) mode: u32,
}

pub(crate) static FINISH_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<Finish>(
        "cancel.finish",
        OperationPolicy {
            timeout: Duration::from_secs(5),
            attempt_timeout: Duration::from_secs(4),
            cancellation_timeout: Duration::from_millis(100),
            retries: RetryPolicy::Never,
        },
    )
    .visibility(harmos_runtime::operations::Visibility::Public)
    .grpc(harmos_runtime::operations::Grpc::new("fixture.cancellation.v1", "Operations", "Finish").managed());

pub(crate) const FINISH_OPERATION: harmos_runtime::operations::OperationSpec<Finish> =
    harmos_runtime::operations::OperationSpec::<Finish>::new(&FINISH_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = u64, input = u64, output = u64)]
#[derive(Clone)]
pub(crate) struct Exchange {}

pub(crate) static EXCHANGE_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<Exchange>("cancel.exchange", FINISH_OPERATION.entry().policy)
        .visibility(harmos_runtime::operations::Visibility::Public)
        .grpc(harmos_runtime::operations::Grpc::new("fixture.cancellation.v1", "Operations", "Exchange").managed());

pub(crate) const EXCHANGE_OPERATION: harmos_runtime::operations::OperationSpec<Exchange> =
    harmos_runtime::operations::OperationSpec::<Exchange>::new(&EXCHANGE_DESCRIPTION);

/* === Implementations === */

pub(crate) async fn finish_handler(
    request: Finish, ctx: harmos_runtime::operations::FunctionContext<Finish>,
) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
    let probe = ctx.resource::<Probe>().expect("declared resource is registered");
    probe.started.fetch_add(1, Ordering::SeqCst);
    let _guard = Guard(probe.clone());
    probe.entered.notify_one();
    if request.mode == 2 {
        return std::future::pending().await;
    }
    ctx.cancelled().await;
    assert!(ctx.cancellation_requested());
    match request.mode {
        0 => Ok(42.into()),
        1 => std::future::pending().await,
        _ => Err(std::io::Error::other("cancelled without response")),
    }
}

pub(crate) fn finish_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        FINISH_OPERATION,
        const { &[harmos_runtime::Resource::of::<Probe>()] },
        finish_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

pub(crate) async fn exchange_handler(
    _request: Exchange, ctx: harmos_runtime::operations::FunctionContext<Exchange>,
) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
    ctx.streams().1.send_with(8, || Ok(7)).await.map_err(std::io::Error::other)?;
    while ctx.streams().0.recv().await.is_some() {}
    ctx.cancelled().await;
    Ok(42.into())
}

pub(crate) fn exchange_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        EXCHANGE_OPERATION,
        const { &[harmos_runtime::Resource::of::<Probe>()] },
        exchange_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Tests === */

#[tokio::test(start_paused = true)]
async fn cancel_final_response_in_process() {
    let probe = Arc::new(Probe::default());
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup
            .resource::<Probe>(probe.clone())
            .operation(finish_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
            .operation(exchange_operation().expect("representable operation"), harmos_runtime::operations::Lane::default());
    })
    .await
    .unwrap();
    for mode in 0..4 {
        let handle = runtime.operation(Finish { mode }).start().await.unwrap();
        probe.entered.notified().await;
        let status = handle.subscribe();
        let result = handle.cancel().await;
        if mode == 0 {
            assert_eq!(*result.unwrap(), 42);
        } else if mode == 3 {
            let Error::OperationFailed { source, .. } = result.unwrap_err() else { panic!("expected the operation's failure") };
            assert_eq!(source.downcast_ref::<std::io::Error>().unwrap().kind(), std::io::ErrorKind::Other);
        } else {
            assert!(matches!(result, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));
        }
        assert!(status.borrow().phase().is_terminal());
        assert_eq!(probe.dropped.load(Ordering::SeqCst), mode as usize + 1);
    }
    assert_eq!(probe.started.load(Ordering::SeqCst), 4);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[cfg(feature = "grpc")]
pub(crate) async fn remote<A: harmos_runtime::Application>(runtime: &Runtime<A>, probe: &Probe) {
    let settings = harmos_runtime::grpc::Settings {
        address: "127.0.0.1:0".parse().unwrap(),
        ..Default::default()
    };
    let attached = runtime.grpc().attach(settings).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", attached.local_addr())).unwrap().connect().await.unwrap();
    let transport = harmos_runtime::grpc::server::Settings::default();
    let transport = harmos_runtime::grpc::client::Settings {
        decode: transport.decode,
        encode: transport.encode,
        input: transport.input,
    };
    for mode in 0..4 {
        let handle = harmos_runtime::grpc::client::start::<Finish>(
            channel.clone(),
            FINISH_OPERATION,
            tonic::Request::new(Finish { mode }),
            transport,
        )
        .await
        .unwrap();
        probe.entered.notified().await;
        let result = handle.cancel().await;
        if mode == 0 {
            assert_eq!(*result.unwrap(), 42);
        } else if mode == 3 {
            assert_eq!(result.unwrap_err().code(), tonic::Code::Internal);
        } else {
            assert_eq!(result.unwrap_err().code(), tonic::Code::Cancelled);
        }
    }
    let mut handle =
        harmos_runtime::grpc::client::start::<Exchange>(channel, EXCHANGE_OPERATION, tonic::Request::new(Exchange {}), transport)
            .await
            .unwrap();
    assert_eq!(*handle.next().await.unwrap().unwrap(), 7);
    handle.send(9).await.unwrap();
    handle.finish_input().await.unwrap();
    assert_eq!(*handle.cancel().await.unwrap(), 42);
    drop(attached);
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn cancel_final_response_grpc() {
    let probe = Arc::new(Probe::default());
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup
            .resource::<Probe>(probe.clone())
            .operation(finish_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
            .operation(exchange_operation().expect("representable operation"), harmos_runtime::operations::Lane::default());
    })
    .await
    .unwrap();
    remote(&runtime, &probe).await;
    assert_eq!(probe.dropped.load(Ordering::SeqCst), 4);
    crate::support::app::shutdown(&runtime).await.unwrap();
}
