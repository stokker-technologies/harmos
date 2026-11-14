/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise Host Caller Admission

/* === Modules === */

#[rustfmt::skip]
mod wire {
    include!(concat!(env!("OUT_DIR"), "/conformance.host.v1.rs"));
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    fmt::Display,
    io::Error as IoError,
    num::NonZeroUsize,
    pin::Pin,
    result::Result as StdResult,
    sync::OnceLock,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    task::{Context, Poll},
    time::{Duration, SystemTime},
};

// Third Party Imports
use futures_core::Stream;
use futures_util::{StreamExt, stream};
use prost::Message;
use prost_reflect::{DescriptorPool, MessageDescriptor, ReflectMessage};
use prost_types::FileDescriptorSet;
use tokio::{net::TcpListener, sync::oneshot};
use tonic::{
    Code, Request, Response, Status, Streaming,
    client::Grpc as RpcClient,
    codegen::http::uri::PathAndQuery,
    transport::{Channel, Endpoint, Server},
};
use tonic_prost::ProstCodec;

// Local Imports
use crate::{Error, Result, fixture};
use harmos_operations::OperationPolicy;
use harmos_operations::clock::Get as ClockGet;
use harmos_operations::execution::operations::{
    Catalog, ExecutionContext, Grpc, OperationFactory, ProtobufOperation, Resource, ServingError, Visibility,
};
use harmos_protocol::host::{InvocationsCreateRequest, invocations_client::InvocationsClient};
use harmos_protocol::model::{Classification, Failure};
use harmos_protocol::plugin::{
    InstanceGetRequest, InstanceGetResponse, InstanceInvokeRequest, InstanceInvokeResponse, InstanceSamplesRequest,
    InstanceSamplesResponse, InstanceStopRequest, InstanceStopResponse,
    instance_client::InstanceClient,
    instance_server::{Instance, InstanceServer},
};
use harmos_runtime::{
    Application, Lifecycle, Runtime, Setup, Startup,
    clock::{Clock, Controller},
    grpc::Settings as GrpcSettings,
    operations::Settings as OperationSettings,
    plugin::grpc::{Placement, PlacementSettings},
    state::Never,
};
use wire::{CallerProbeGetRequest, CallerProbeGetResponse};

/* === Constants === */

const TARGET: &str = "conformance.host.v1.CallerProbe/Get";
const CREATE: &str = "harmos.host.v1.Invocations/Create";

/* === Macros === */

/* === Type Aliases === */

type Frames<T> = Pin<Box<dyn Stream<Item = StdResult<T, Status>> + Send>>;

/* === Structs === */

struct App;
struct Probe;

struct Forwarding<T> {
    incoming: Pin<Box<Streaming<T>>>,
}

#[derive(Clone)]
struct Forward {
    client: InstanceClient<Channel>,
    sample: Arc<Mutex<Option<Vec<u8>>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<T> Stream for Forwarding<T>
where
    Streaming<T>: Stream<Item = StdResult<T, Status>>,
{
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<T>> {
        match self.incoming.as_mut().poll_next(context) {
            Poll::Ready(Some(Ok(frame))) => Poll::Ready(Some(frame)),
            Poll::Ready(Some(Err(_)) | None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl ReflectMessage for CallerProbeGetRequest {
    fn descriptor(&self) -> MessageDescriptor {
        pool().get_message_by_name("conformance.host.v1.CallerProbeGetRequest").expect("caller probe request")
    }
}

impl ReflectMessage for CallerProbeGetResponse {
    fn descriptor(&self) -> MessageDescriptor {
        pool().get_message_by_name("conformance.host.v1.CallerProbeGetResponse").expect("caller probe response")
    }
}

impl Lifecycle for App {
    type Config = ();
    type Error = IoError;
    type Context = Startup<Self>;

    async fn prepare(_: Self::Config, setup: &mut Setup<Self>) -> StdResult<Self, Self::Error> {
        let factory = OperationFactory::protobuf::<Probe>().expect("caller probe contract");
        setup.catalog(Catalog::new().entry(factory), Default::default());
        setup.catalog(harmos_operations::lifecycle_catalog(), Default::default());
        setup.composition(include_str!("../../../sdk/tests/fixtures/client/composition.toml")).map_err(IoError::other)?;
        Ok(Self)
    }
}

impl Application for App {
    type State = ();
    type Transaction = Never;
    type Record = ();
}

impl ProtobufOperation for Probe {
    type Request = CallerProbeGetRequest;
    type Response = CallerProbeGetResponse;
    type Endpoints = ();
    const ID: &'static str = TARGET;
    const POLICY: OperationPolicy = ClockGet::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const GRPC: Option<Grpc> = Some(Grpc::new("conformance.host.v1", "CallerProbe", "Get"));
    const RESOURCES: &'static [Resource] = &[Resource::of::<AtomicU64>()];

    async fn run(context: ExecutionContext, _: CallerProbeGetRequest, _: &()) -> StdResult<CallerProbeGetResponse, ServingError> {
        context.resource::<AtomicU64>().map_err(|_| ServingError::Catalog)?.fetch_add(1, Ordering::SeqCst);
        Ok(CallerProbeGetResponse {})
    }
}

#[tonic::async_trait]
impl Instance for Forward {
    async fn get(&self, request: Request<InstanceGetRequest>) -> StdResult<Response<InstanceGetResponse>, Status> {
        let mut response = self.client.clone().get(request).await?;
        let declarations = response
            .get_mut()
            .declarations
            .as_mut()
            .ok_or_else(|| Status::invalid_argument("Admit Caller Fixture: Missing Declarations"))?;
        declarations.operations.retain(|operation| matches!(operation.id.as_str(), "conformance.caller_refusal" | "conformance.step.0"));
        declarations.streams.retain(|stream| stream.id == "conformance.tick_caller_refusal");
        declarations.requires.retain(|requirement| requirement.id == CREATE);
        if declarations.operations.len() != 2 || declarations.streams.len() != 1 || declarations.requires.len() != 1 {
            return Err(Status::invalid_argument("Admit Caller Fixture: Missing Caller Refusal Contracts"));
        }
        declarations.digest = harmos_protocol::schema::declarations_digest(declarations);
        Ok(response)
    }

    type InvokeStream = Streaming<InstanceInvokeResponse>;

    async fn invoke(&self, request: Request<Streaming<InstanceInvokeRequest>>) -> StdResult<Response<Self::InvokeStream>, Status> {
        let frames = Forwarding { incoming: Box::pin(request.into_inner()) };
        self.client.clone().invoke(frames).await
    }

    type SamplesStream = Frames<InstanceSamplesResponse>;

    async fn samples(&self, request: Request<Streaming<InstanceSamplesRequest>>) -> StdResult<Response<Self::SamplesStream>, Status> {
        let frames = Forwarding { incoming: Box::pin(request.into_inner()) };
        let incoming = self.client.clone().samples(frames).await?.into_inner();
        let sample = self.sample.clone();
        let frames = incoming.map(move |frame| {
            if let Ok(frame) = &frame {
                if let Some(row) = frame.samples.first() {
                    *sample.lock().expect("caller sample lock") = Some(row.row.clone());
                }
            }
            frame
        });
        Ok(Response::new(Box::pin(frames)))
    }

    async fn stop(&self, _: Request<InstanceStopRequest>) -> StdResult<Response<InstanceStopResponse>, Status> {
        // The Suite Owns the Underlying Plugin and Stops It in Rule 16.
        Ok(Response::new(InstanceStopResponse {}))
    }
}

/* === Public Functions === */

pub fn request() -> Vec<u8> {
    InvocationsCreateRequest {
        operation: TARGET.into(),
        fingerprint: harmos_protocol::schema::fingerprint(
            TARGET,
            &FileDescriptorSet::decode(include_bytes!(concat!(env!("OUT_DIR"), "/caller-descriptors.bin")).as_slice())
                .expect("generated caller descriptors"),
            "conformance.host.v1.CallerProbeGetRequest",
            "conformance.host.v1.CallerProbeGetResponse",
        )
        .expect("caller probe fingerprint"),
        payload: Vec::new(),
    }
    .encode_to_vec()
}

pub async fn run(rule: usize, address: &str) -> Result<()> {
    // Compose a Real Host With the Same Caller Narrowing Used by SDK Generation.
    let counter = Arc::new(AtomicU64::new(0));
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<AtomicU64>(counter.clone());
        setup.settings().clock(clock);
        setup.settings().operation_settings(OperationSettings {
            preparation_bytes: NonZeroUsize::new(16 * 1024 * 1024),
            ..Default::default()
        });
    })
    .await
    .map_err(invalid)?;

    // Prove the Probe Is Live Before Checking Refusal Before Dispatch.
    runtime.operations.call_protobuf::<Probe>(CallerProbeGetRequest {}).await.map_err(invalid)?;
    require(counter.load(Ordering::SeqCst) == 1, "Call Probe: Missing Invocation")?;

    let outcome =
        if rule == 33 { client_refusal(&runtime, &counter).await } else { plugin_refusal(&runtime, &counter, &controller, address).await };
    runtime.shutdown().await.map_err(invalid)?;
    outcome
}

/* === Private Functions === */

fn pool() -> &'static DescriptorPool {
    static POOL: OnceLock<DescriptorPool> = OnceLock::new();
    POOL.get_or_init(|| {
        DescriptorPool::decode(include_bytes!(concat!(env!("OUT_DIR"), "/caller-descriptors.bin")).as_slice())
            .expect("caller probe descriptors")
    })
}

async fn client_refusal(runtime: &Runtime<App>, counter: &AtomicU64) -> Result<()> {
    // A Client Knows Both the Direct Binding and the Dynamic Contract.
    let listener = runtime
        .grpc()
        .attach(GrpcSettings {
            address: "127.0.0.1:0".parse().expect("loopback"),
            ..Default::default()
        })
        .await
        .map_err(invalid)?;
    let address = format!("http://{}", listener.local_addr());
    let channel = Endpoint::from_shared(address.clone()).map_err(invalid)?.connect().await?;
    let mut client = RpcClient::new(channel);
    client.ready().await.map_err(invalid)?;
    let codec = ProstCodec::<CallerProbeGetRequest, CallerProbeGetResponse>::default();
    let status = client
        .unary(Request::new(CallerProbeGetRequest {}), PathAndQuery::from_static("/conformance.host.v1.CallerProbe/Get"), codec)
        .await
        .err()
        .ok_or_else(|| Error::Invalid("Call Probe: Client Reached Plugins-Only Handler".into()))?;
    require(status.code() == Code::PermissionDenied, "Call Probe: Direct Caller Was Not Refused")?;
    let mut client = InvocationsClient::connect(address).await?;
    let status = client
        .create(InvocationsCreateRequest::decode(request().as_slice())?)
        .await
        .err()
        .ok_or_else(|| Error::Invalid("Create Invocation: Client Reached Plugins-Only Handler".into()))?;
    require(status.code() == Code::PermissionDenied, "Create Invocation: Client Was Not Refused")?;
    require(counter.load(Ordering::SeqCst) == 1, "Call Probe: Refused Client Reached Handler")?;
    drop(client);
    drop(listener);
    Ok(())
}

async fn plugin_refusal(runtime: &Runtime<App>, counter: &AtomicU64, controller: &Controller, address: &str) -> Result<()> {
    // Admit Only This Journey's SDK Contributions Without Taking Ownership of Its Process.
    let sample = Arc::new(Mutex::new(None));
    let forward = Forward {
        client: InstanceClient::connect(address.to_owned()).await?,
        sample: sample.clone(),
    };
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = format!("http://{}", listener.local_addr()?);
    let incoming = stream::unfold(listener, |listener| async {
        let socket = listener.accept().await.map(|(socket, _)| socket);
        Some((socket, listener))
    });
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(Server::builder().add_service(InstanceServer::new(forward)).serve_with_incoming_shutdown(incoming, async {
        let _ = stopped.await;
    }));
    let settings = PlacementSettings {
        connect_timeout: Duration::from_secs(10),
        reconnect_delay: Duration::from_secs(1),
        stop_grace: Duration::from_secs(10),
        stream_window: NonZeroUsize::new(8).expect("stream window"),
    };
    let result = async {
        let attached = runtime.attach_grpc(Placement::Remote(address), settings).await.map_err(invalid)?;
        let result = async {
            // Refuse the Same CLIENTS Operation From Invoke and an Unanswered Samples Tick.
            let payload = runtime.operations.call_encoded("conformance.caller_refusal", fixture::payload(&[])).await.map_err(invalid)?;
            refusal(&payload)?;
            controller
                .advance_until(Duration::from_secs(10), || sample.lock().expect("caller sample lock").is_some())
                .await
                .map_err(invalid)?;
            let payload = sample
                .lock()
                .expect("caller sample lock")
                .clone()
                .ok_or_else(|| Error::Invalid("Tick Caller Refusal: Missing Sample".into()))?;
            refusal(&payload)?;
            require(counter.load(Ordering::SeqCst) == 1, "Create Invocation: Refused Plugin Dispatched a Nested Call")?;

            // Keep the Connection Usable After Both Refusals.
            let payload = fixture::payload(b"after refusal");
            let reply = runtime.operations.call_encoded("conformance.step.0", payload.clone()).await.map_err(invalid)?;
            require(reply == payload, "Invoke After Refusal: Connection Is Unusable")
        }
        .await;
        attached.stop("Caller Journey Complete").await.map_err(invalid)?;
        result
    }
    .await;
    let _ = stop.send(());
    server.await.map_err(invalid)??;
    result
}

fn refusal(payload: &[u8]) -> Result<()> {
    let value = fixture::Value::decode(payload)?;
    let failure = Failure::decode(value.value.as_slice())?;
    require(
        failure.classification == Classification::Refused as i32 && failure.operation == CREATE && !failure.plugin.is_empty(),
        "Create Invocation: Caller Refusal Changed Classification or Operation",
    )
}

fn require(condition: bool, action: &str) -> Result<()> {
    if condition { Ok(()) } else { Err(Error::Invalid(action.into())) }
}

fn invalid(error: impl Display) -> Error {
    Error::Invalid(error.to_string())
}

/* === Tests === */
