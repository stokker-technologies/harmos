/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Cancellation Final Response Proof

/* === Modules === */

/* === Re-Exports === */

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
use super::{Lanes, app::App};
use harmos_runtime::operations::{OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Refusal, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
pub(crate) struct Probe {
    pub(crate) entered: Notify,
    pub(crate) started: AtomicUsize,
    pub(crate) dropped: AtomicUsize,
}

struct Guard(Arc<Probe>);

#[harmos_runtime::operation(resources = [Probe], id = "cancel.finish", response = u64, visibility = Public,
    grpc(package = "fixture.cancellation.v1", service = "Operations", method = "Finish"),
    policy = OperationPolicy {
        timeout: Duration::from_secs(5), attempt_timeout: Duration::from_secs(4),
        cancellation_timeout: Duration::from_millis(100), retries: RetryPolicy::Never,
    }
)]
pub(crate) struct Finish {
    pub(crate) mode: u32,
}

#[harmos_runtime::operation(resources = [Probe], id = "cancel.exchange", response = u64, input = u64, output = u64, visibility = Public,
    grpc(package = "fixture.cancellation.v1", service = "Operations", method = "Exchange"), policy = Finish::POLICY
)]
pub(crate) struct Exchange {}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Operation for Finish {
    type Error = std::io::Error;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<u64>, Self::Error> {
        let probe = ctx.resource::<Probe>();
        probe.started.fetch_add(1, Ordering::SeqCst);
        let _guard = Guard(probe.clone());
        probe.entered.notify_one();
        if self.mode == 2 {
            return std::future::pending().await;
        }
        ctx.cancelled().await;
        assert!(ctx.cancellation_requested());
        match self.mode {
            0 => Ok(42.into()),
            1 => std::future::pending().await,
            _ => Err(std::io::Error::other("cancelled without response")),
        }
    }
}

impl harmos_runtime::Operation for Exchange {
    type Error = std::io::Error;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<u64>, Self::Error> {
        ctx.streams().1.send_with(8, || Ok(7)).await.map_err(std::io::Error::other)?;
        while ctx.streams().0.recv().await.is_some() {}
        ctx.cancelled().await;
        Ok(42.into())
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test(start_paused = true)]
async fn cancel_final_response_in_process() {
    let probe = Arc::new(Probe::default());
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup
            .resource::<Probe>(probe.clone())
            .operations::<super::Single<Finish>>(Lanes::Open)
            .operations::<super::Single<Exchange>>(Lanes::Open);
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
        } else {
            assert!(matches!(result, Err(Error::OperationRefused(Refusal::Cancelled))));
        }
        assert!(status.borrow().phase().is_terminal());
        assert_eq!(probe.dropped.load(Ordering::SeqCst), mode as usize + 1);
    }
    assert_eq!(probe.started.load(Ordering::SeqCst), 4);
    runtime.shutdown().await.unwrap();
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
        let handle =
            harmos_runtime::grpc::client::start::<Finish>(channel.clone(), tonic::Request::new(Finish { mode }), transport).await.unwrap();
        probe.entered.notified().await;
        let result = handle.cancel().await;
        if mode == 0 {
            assert_eq!(*result.unwrap(), 42);
        } else {
            assert_eq!(result.unwrap_err().code(), tonic::Code::Cancelled);
        }
    }
    let mut handle = harmos_runtime::grpc::client::start::<Exchange>(channel, tonic::Request::new(Exchange {}), transport).await.unwrap();
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
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup
            .resource::<Probe>(probe.clone())
            .operations::<super::Single<Finish>>(Lanes::Open)
            .operations::<super::Single<Exchange>>(Lanes::Open);
    })
    .await
    .unwrap();
    remote(&runtime, &probe).await;
    assert_eq!(probe.dropped.load(Ordering::SeqCst), 4);
    runtime.shutdown().await.unwrap();
}
