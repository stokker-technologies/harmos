/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Lane Stories Through the Native Host.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use crate::{Error, Result, callers::Forward, conformance::Value, fixture};
use harmos_operations::execution::operations::{
    Catalog, ExecutionContext, OperationFactory, ProtobufOperation, Resource, ServingError, Visibility,
};
use harmos_protocol::plugin::{instance_client::InstanceClient, instance_server::InstanceServer};
use harmos_runtime::{
    Application, Lifecycle, Runtime, Setup, Startup,
    operations::Lane,
    plugin::grpc::{Placement, PlacementSettings},
    state::Never,
};
use prost_reflect::{DescriptorPool, MessageDescriptor, ReflectMessage};
use std::{
    collections::BTreeMap,
    io, result,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot},
};
use tonic::transport::Server;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct App;
struct Pending;

struct Signals {
    entered: mpsc::UnboundedSender<Vec<u8>>,
    release: Mutex<BTreeMap<Vec<u8>, oneshot::Sender<()>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ReflectMessage for Value {
    fn descriptor(&self) -> MessageDescriptor {
        DescriptorPool::from_file_descriptor_set(fixture::types())
            .expect("fixture types")
            .get_message_by_name(fixture::ROW)
            .expect("fixture row")
    }
}

impl Lifecycle for App {
    type Config = ();
    type Error = io::Error;
    type Context = Startup<Self>;

    async fn prepare(_: (), setup: &mut Setup<Self>) -> result::Result<Self, io::Error> {
        let factory = OperationFactory::protobuf::<Pending>().expect("lane barrier contract");
        setup.catalog(Catalog::new().entry(factory), Lane::default());
        Ok(Self)
    }
}

impl Application for App {
    type State = ();
    type Transaction = Never;
    type Record = ();
}

impl ProtobufOperation for Pending {
    type Request = Value;
    type Response = Value;
    type Endpoints = ();
    const ID: &'static str = "conformance.host.pending";
    const POLICY: harmos_operations::OperationPolicy = <harmos_operations::clock::Get as ProtobufOperation>::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<Signals>()];

    async fn run(context: ExecutionContext, request: Value, _: &()) -> result::Result<Value, ServingError> {
        let signals = context.resource::<Signals>().map_err(|_| ServingError::Catalog)?;
        let (release, waiting) = oneshot::channel();
        signals.release.lock().expect("lane barrier lock").insert(request.value.clone(), release);
        signals.entered.send(request.value.clone()).map_err(|_| ServingError::Catalog)?;
        tokio::select! {
            result = waiting => result.map_err(|_| ServingError::Catalog)?,
            () = context.cancellation.cancelled() => {},
        }
        Ok(request)
    }
}

impl Signals {
    fn release(&self, marker: &[u8]) {
        self.release.lock().expect("lane barrier lock").remove(marker).expect("entered invocation").send(()).expect("waiting invocation");
    }
}

/* === Public Functions === */

pub(super) async fn run(address: &str) -> Result<()> {
    // Compose the Host Barrier and a Forwarder Whose Stop Leaves the Suite's Plugin Running.
    let (entered, mut events) = mpsc::unbounded_channel();
    let signals = Arc::new(Signals {
        entered,
        release: Mutex::new(BTreeMap::new()),
    });
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Signals>(signals.clone());
    })
    .await
    .map_err(invalid)?;
    let client = InstanceClient::connect(address.to_owned()).await?;
    let forward = Forward {
        client,
        lanes: true,
        sample: Arc::new(Mutex::new(None)),
    };
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let incoming =
        futures_util::stream::unfold(listener, |listener| async { Some((listener.accept().await.map(|(socket, _)| socket), listener)) });
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(Server::builder().add_service(InstanceServer::new(forward)).serve_with_incoming_shutdown(incoming, async {
        let _ = stopped.await;
    }));
    let attached = runtime
        .attach_grpc(
            Placement::Remote(format!("http://{address}")),
            PlacementSettings {
                reconnect_delay: Duration::from_secs(1),
                stop_grace: Duration::from_secs(30),
                stream_window: std::num::NonZeroUsize::new(8).unwrap(),
                interval_window: Duration::from_secs(1),
                interval_lag: Default::default(),
            },
        )
        .await
        .map_err(invalid)?;

    // Serial Operations Preserve Arrival Order Across Different Declarations.
    let first = runtime.operations.submit_encoded("conformance.lane.first", fixture::payload(b"1")).map_err(invalid)?;
    event(&mut events, b"1").await?;
    let second = runtime.operations.submit_encoded("conformance.lane.second", fixture::payload(b"2")).map_err(invalid)?;
    let third = runtime.operations.submit_encoded("conformance.lane.first", fixture::payload(b"3")).map_err(invalid)?;
    signals.release(b"1");
    first.await.map_err(invalid)?;
    event(&mut events, b"2").await?;
    signals.release(b"2");
    second.await.map_err(invalid)?;
    event(&mut events, b"3").await?;
    signals.release(b"3");
    third.await.map_err(invalid)?;

    // Concurrent Width Two Admits Two and Queues the Third.
    let first = runtime.operations.submit_encoded("conformance.lane.parallel", fixture::payload(b"4")).map_err(invalid)?;
    event(&mut events, b"4").await?;
    let second = runtime.operations.submit_encoded("conformance.lane.parallel", fixture::payload(b"5")).map_err(invalid)?;
    event(&mut events, b"5").await?;
    let third = runtime.operations.submit_encoded("conformance.lane.parallel", fixture::payload(b"6")).map_err(invalid)?;
    signals.release(b"5");
    second.await.map_err(invalid)?;
    event(&mut events, b"6").await?;
    signals.release(b"4");
    signals.release(b"6");
    first.await.map_err(invalid)?;
    third.await.map_err(invalid)?;

    // Different and Default Lanes Do Not Wait for the Busy Serial Lane.
    let first = runtime.operations.submit_encoded("conformance.lane.first", fixture::payload(b"7")).map_err(invalid)?;
    event(&mut events, b"7").await?;
    let other = runtime.operations.submit_encoded("conformance.lane.other", fixture::payload(b"8")).map_err(invalid)?;
    event(&mut events, b"8").await?;
    let open = runtime.operations.submit_encoded("conformance.lane.open", fixture::payload(b"9")).map_err(invalid)?;
    event(&mut events, b"9").await?;

    // Cancel a Queued Invocation Before Any Plugin Dispatch.
    let cancelled = runtime.operations.submit_encoded("conformance.lane.second", fixture::payload(b"cancelled")).map_err(invalid)?;
    let settlement = cancelled.settle();
    let result = cancelled.cancel().await;
    if !matches!(result, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Cancelled { .. }))) {
        return Err(invalid("Cancel Lane Waiter: Return Cancelled"));
    }
    settlement.await.map_err(invalid)?;
    signals.release(b"8");
    signals.release(b"9");
    other.await.map_err(invalid)?;
    open.await.map_err(invalid)?;
    signals.release(b"7");
    first.await.map_err(invalid)?;
    let probe = runtime.operations.submit_encoded("conformance.lane.first", fixture::payload(b"probe")).map_err(invalid)?;
    event(&mut events, b"probe").await?;
    // A Queued Cancellation Must Never Have Generated a Host Barrier Call.
    if signals.release.lock().expect("lane barrier lock").contains_key(b"cancelled".as_slice()) {
        return Err(invalid("Cancel Lane Waiter: Do Not Enter the Plugin"));
    }

    // Stop Releases Waiters With the Existing Unavailable Failure.
    let waiter = runtime.operations.submit_encoded("conformance.lane.second", fixture::payload(b"stopped")).map_err(invalid)?;
    let (result, stop_result) = tokio::join!(waiter, attached.stop("Lane Stories Complete"));
    if !matches!(result, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Unavailable))) {
        return Err(invalid("Stop Plugin: Fail Lane Waiters as Unavailable"));
    }
    stop_result.map_err(invalid)?;
    let _ = probe.await;
    runtime.shutdown().await.map_err(invalid)?;
    let _ = stop.send(());
    server.await.map_err(invalid)??;
    Ok(())
}

/* === Private Functions === */

async fn event(events: &mut mpsc::UnboundedReceiver<Vec<u8>>, expected: &[u8]) -> Result<()> {
    if events.recv().await.as_deref() != Some(expected) {
        return Err(invalid("Admit Lane Invocation: Preserve Declared Capacity and Arrival Order"));
    }
    Ok(())
}

fn invalid(error: impl std::fmt::Display) -> Error {
    Error::Invalid(error.to_string())
}

/* === Tests === */
