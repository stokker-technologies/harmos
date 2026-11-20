/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Owned Plugin Listener Contracts

/* === Modules === */

mod collections;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    future::Future,
    pin::pin,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::Poll,
};

// Third Party Imports
use futures_util::{future::poll_fn, stream};
use tokio::sync::Notify;
use tonic::Code;

// Local Imports
use crate::{
    Message, Visibility,
    plugin::{Context, Operation, Plugin},
};
use harmos_protocol::plugin::{
    InstanceGetRequest, InstanceInvokeRequest, InstanceStopRequest, instance_client::InstanceClient, instance_invoke_request,
    instance_invoke_response, response,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Message)]
struct Echo {
    value: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn addressable_listener_waits_for_clean_stop() {
    let mut listener = Plugin::new("example", "1.0.0").start("127.0.0.1:0").await.unwrap();
    let mut client = InstanceClient::connect(format!("http://{}", listener.address())).await.unwrap();
    client.stop(InstanceStopRequest { reason: "Story Stop".into() }).await.unwrap();
    listener.wait().await.unwrap();
}

#[tokio::test]
async fn owned_listener_drains_and_restarts() {
    // Gate an Accepted Invocation Without Depending on Scheduler Timing.
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let cleanup_entered = Arc::new(Notify::new());
    let settled = Arc::new(AtomicUsize::new(0));
    let stopped = Arc::new(AtomicUsize::new(0));
    let operation = Operation::new("example.echo", {
        let entered = entered.clone();
        let release = release.clone();
        let settled = settled.clone();
        async move |_: &Context, request: Echo| {
            // An Author's Future May Be Local Even When the Caller Uses No LocalSet.
            let local = Rc::new(request);
            entered.notify_one();
            release.notified().await;
            settled.fetch_add(1, Ordering::SeqCst);
            Ok((*local).clone())
        }
    })
    .visibility(Visibility::Public);
    let plugin = Plugin::new("example", "1.0.0").register(|catalog| catalog.register(operation)).unwrap().on_stop({
        let stopped = stopped.clone();
        let cleanup_entered = cleanup_entered.clone();
        move || {
            stopped.fetch_add(1, Ordering::SeqCst);
            cleanup_entered.notify_one();
            Ok(())
        }
    });

    // Bind Port Zero and Invoke Through the Returned Concrete Address.
    let mut listener = plugin.start("127.0.0.1:0").await.unwrap();
    let address = listener.address();
    assert_ne!(address.port(), 0);
    let url = format!("http://{address}");
    let mut client = InstanceClient::connect(url.clone()).await.unwrap();
    let request = Echo { value: "hello".into() };
    let opening = InstanceInvokeRequest {
        frame: Some(instance_invoke_request::Frame::Request(harmos_protocol::plugin::Request {
            operation: "example.echo".into(),
            invocation: "invocation".into(),
            attempt: 1,
            payload: prost::Message::encode_to_vec(&request),
            ..Default::default()
        })),
    };
    let mut reply = client.invoke(stream::iter([opening])).await.unwrap().into_inner();
    entered.notified().await;

    // Close Must Remain Pending Until the Accepted Handler Can Settle.
    {
        let mut close = pin!(listener.close());
        assert!(poll_fn(|cx| Poll::Ready(close.as_mut().poll(cx))).await.is_pending());
        cleanup_entered.notified().await;
        assert_eq!(stopped.load(Ordering::SeqCst), 1);
        assert_eq!(settled.load(Ordering::SeqCst), 0);
        release.notify_one();
        let (closed, frame) = tokio::join!(&mut close, reply.message());
        closed.unwrap();
        let frame = frame.unwrap().unwrap();
        let Some(instance_invoke_response::Frame::Response(result)) = frame.frame else {
            panic!("Expected Invocation Response")
        };
        let Some(response::Result::Payload(payload)) = result.result else {
            panic!("Expected Invocation Payload")
        };
        assert_eq!(<Echo as prost::Message>::decode(payload.as_slice()).unwrap().value, "hello");
    }
    assert_eq!(settled.load(Ordering::SeqCst), 1);
    assert_eq!(stopped.load(Ordering::SeqCst), 1);

    // Closed Transports Refuse Calls, Repeated Close Does Nothing, and the Process Can Rebind.
    listener.close().await.unwrap();
    assert_eq!(stopped.load(Ordering::SeqCst), 1);
    assert_eq!(client.get(InstanceGetRequest { revision: 1, ..Default::default() }).await.unwrap_err().code(), Code::Unavailable);
    assert!(InstanceClient::connect(url).await.is_err());
    let mut second = Plugin::new("second", "1.0.0").start(address.to_string()).await.unwrap();
    let mut client = InstanceClient::connect(format!("http://{}", second.address())).await.unwrap();
    assert_eq!(client.get(InstanceGetRequest { revision: 1, ..Default::default() }).await.unwrap().into_inner().id, "second");
    second.close().await.unwrap();
}

#[tokio::test]
async fn remote_stop_and_close_run_cleanup_once() {
    let stopped = Arc::new(AtomicUsize::new(0));
    let mut listener = Plugin::new("remote", "1.0.0")
        .remote()
        .on_stop({
            let stopped = stopped.clone();
            move || {
                stopped.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        })
        .start("127.0.0.1:0")
        .await
        .unwrap();
    let mut client = InstanceClient::connect(format!("http://{}", listener.address())).await.unwrap();
    client.stop(InstanceStopRequest::default()).await.unwrap();
    client.stop(InstanceStopRequest::default()).await.unwrap();
    assert!(client.get(InstanceGetRequest { revision: 1, ..Default::default() }).await.is_ok());
    listener.close().await.unwrap();
    listener.close().await.unwrap();
    assert_eq!(stopped.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cleanup_failure_still_releases_the_listener() {
    let stopped = Arc::new(AtomicUsize::new(0));
    let mut listener = Plugin::new("failure", "1.0.0")
        .on_stop({
            let stopped = stopped.clone();
            move || {
                stopped.fetch_add(1, Ordering::SeqCst);
                Err(crate::Error::Invalid("Stop Plugin: Release the Device".into()))
            }
        })
        .start("127.0.0.1:0")
        .await
        .unwrap();
    let address = listener.address();
    assert!(listener.close().await.is_err());
    listener.close().await.unwrap();
    assert_eq!(stopped.load(Ordering::SeqCst), 1);
    let mut second = Plugin::new("second", "1.0.0").start(address.to_string()).await.unwrap();
    second.close().await.unwrap();
}
