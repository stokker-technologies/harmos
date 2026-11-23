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
    io,
    num::NonZeroUsize,
    pin::Pin,
    result,
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
    Code, Request, Response, Status, Streaming, client,
    codegen::http::uri::PathAndQuery,
    transport::{Channel, Endpoint, Server},
};
use tonic_prost::ProstCodec;
use wire::{CallerProbeGetRequest, CallerProbeGetResponse};

// Local Imports
use crate::host::invocations_client::InvocationsClient;
use crate::{Error, Result, fixture};
use harmos_operations::OperationPolicy;
use harmos_operations::clock;
use harmos_operations::execution::operations::{
    Catalog, ExecutionContext, Grpc, OperationFactory, ProtobufOperation, Resource, ServingError, Visibility,
};
use harmos_protocol::host::{InvocationsCreateRequest, PluginsGetRequest, PluginsGetResponse, TimeGetRequest, TimeGetResponse};
use harmos_protocol::model::{Classification, Failure};
use harmos_protocol::plugin::{
    InstanceGetRequest, InstanceGetResponse, InstanceInvokeRequest, InstanceInvokeResponse, InstanceSamplesRequest,
    InstanceSamplesResponse, InstanceStopRequest, InstanceStopResponse,
    instance_client::InstanceClient,
    instance_samples_response,
    instance_server::{Instance, InstanceServer},
};
use harmos_runtime::{
    Application, Lifecycle, Runtime, Setup, Startup,
    clock::{Clock, Controller},
    grpc, operations,
    plugin::grpc::{Placement, PlacementSettings},
    state::Never,
};

/* === Constants === */

const TARGET: &str = "conformance.host.v1.CallerProbe/Get";
const CREATE: &str = "application.host.v1.Invocations/Create";

/* === Macros === */

/* === Type Aliases === */

type Frames<T> = Pin<Box<dyn Stream<Item = result::Result<T, Status>> + Send>>;

/* === Structs === */

struct App;
struct Probe;
struct NativeRefusal;

struct Forwarding<T> {
    incoming: Pin<Box<Streaming<T>>>,
}

#[derive(Clone)]
pub(crate) struct Forward {
    pub(crate) client: InstanceClient<Channel>,
    pub(crate) lanes: bool,
    pub(crate) sample: Arc<Mutex<Option<Vec<u8>>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<T> Stream for Forwarding<T>
where
    Streaming<T>: Stream<Item = result::Result<T, Status>>,
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
    type Error = io::Error;
    type Context = Startup<Self>;

    async fn prepare(_: Self::Config, setup: &mut Setup<Self>) -> result::Result<Self, Self::Error> {
        setup.catalog(crate::product::catalog(), Default::default());
        let factory = OperationFactory::protobuf::<Probe>().expect("caller probe contract");
        setup.catalog(Catalog::new().entry(factory), Default::default());
        setup.catalog(
            Catalog::new().entry(OperationFactory::protobuf::<NativeRefusal>().expect("native refusal contract")),
            Default::default(),
        );
        setup.catalog(harmos_operations::lifecycle_catalog(), Default::default());
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
    const POLICY: OperationPolicy = clock::Get::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const GRPC: Option<Grpc> = Some(Grpc::new("conformance.host.v1", "CallerProbe", "Get"));
    const RESOURCES: &'static [Resource] = &[Resource::of::<AtomicU64>()];

    async fn run(context: ExecutionContext, _: CallerProbeGetRequest, _: &()) -> result::Result<CallerProbeGetResponse, ServingError> {
        context.resource::<AtomicU64>().map_err(|_| ServingError::Catalog)?.fetch_add(1, Ordering::SeqCst);
        Ok(CallerProbeGetResponse {})
    }
}

impl ProtobufOperation for NativeRefusal {
    type Request = PluginsGetRequest;
    type Response = PluginsGetResponse;
    type Endpoints = ();
    const ID: &'static str = "conformance.native_refusal";
    const POLICY: OperationPolicy = clock::Get::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const GRPC: Option<Grpc> = Some(Grpc::new("conformance.host.v1", "Refusal", "Get"));
    const RESOURCES: &'static [Resource] = &[];

    async fn run(_: ExecutionContext, _: PluginsGetRequest, _: &()) -> result::Result<PluginsGetResponse, ServingError> {
        let failure = Failure {
            classification: Classification::Permanent as i32,
            code: "unsupported".into(),
            message: "Read Probe: capability is unavailable".into(),
            action: "Read Probe".into(),
            operation: Self::ID.into(),
            cause: Some(Box::new(Failure {
                code: "capability_absent".into(),
                ..Default::default()
            })),
            ..Default::default()
        };
        Err(ServingError::Native(Arc::new(Status::with_details(
            Code::Unimplemented,
            failure.message.clone(),
            failure.encode_to_vec().into(),
        ))))
    }
}

#[tonic::async_trait]
impl Instance for Forward {
    async fn get(&self, request: Request<InstanceGetRequest>) -> result::Result<Response<InstanceGetResponse>, Status> {
        let mut response = self.client.clone().get(request).await?;
        let description = response.get_mut();
        if self.lanes {
            // Retain the Target's Namespace and Identify the Lane Adapter's Carrier.
            description.version = "1.0.0".into();
            description.sdk = "conformance-lanes".into();
            description.sdk_version = env!("CARGO_PKG_VERSION").into();
        }
        let declarations =
            description.declarations.as_mut().ok_or_else(|| Status::invalid_argument("Admit Caller Fixture: Missing Declarations"))?;
        if self.lanes {
            declarations.operations.retain(|operation| operation.id.starts_with("conformance.lane."));
            declarations.streams.clear();
            declarations.requires.retain(|requirement| requirement.id == "conformance.host.pending");
        } else {
            declarations
                .operations
                .retain(|operation| matches!(operation.id.as_str(), "conformance.caller_refusal" | "conformance.step.0"));
            declarations.streams.retain(|stream| stream.id == "conformance.window_caller_refusal");
            declarations.requires.retain(|requirement| requirement.id == CREATE);
            if declarations.operations.len() != 2 || declarations.streams.len() != 1 || declarations.requires.len() != 1 {
                return Err(Status::invalid_argument("Admit Caller Fixture: Missing Caller Refusal Contracts"));
            }
        }
        declarations.digest = harmos_protocol::schema::declarations_digest(declarations);
        Ok(response)
    }

    type InvokeStream = Streaming<InstanceInvokeResponse>;

    async fn invoke(&self, request: Request<Streaming<InstanceInvokeRequest>>) -> result::Result<Response<Self::InvokeStream>, Status> {
        let frames = Forwarding { incoming: Box::pin(request.into_inner()) };
        self.client.clone().invoke(frames).await
    }

    type SamplesStream = Frames<InstanceSamplesResponse>;

    async fn samples(&self, request: Request<Streaming<InstanceSamplesRequest>>) -> result::Result<Response<Self::SamplesStream>, Status> {
        let frames = Forwarding { incoming: Box::pin(request.into_inner()) };
        let incoming = self.client.clone().samples(frames).await?.into_inner();
        let sample = self.sample.clone();
        let frames = incoming.map(move |frame| {
            // Keep the First Window Sample's Row, Rebuilt From the Part's Value Column.
            if let Ok(InstanceSamplesResponse {
                frame: Some(instance_samples_response::Frame::Batch(batch)),
                ..
            }) = &frame
                && let Ok(columns) = fixture::Values::decode(batch.columns.as_slice())
                && let Some(value) = columns.value.first()
            {
                sample.lock().expect("caller sample lock").get_or_insert_with(|| fixture::payload(value));
            }
            frame
        });
        Ok(Response::new(Box::pin(frames)))
    }

    async fn stop(&self, _: Request<InstanceStopRequest>) -> result::Result<Response<InstanceStopResponse>, Status> {
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
    // Compose a Host From Declared Audiences.
    let counter = Arc::new(AtomicU64::new(0));
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let runtime = Runtime::<App>::start_with((), (), |setup| {
        setup.resource::<AtomicU64>(counter.clone());
        setup.settings().clock(clock);
        setup.settings().operation_settings(operations::Settings {
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
        if rule == 30 { client_refusal(&runtime, &counter).await } else { plugin_refusal(&runtime, &counter, &controller, address).await };
    runtime.stop().await.map_err(invalid)?;
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
        .attach(grpc::Settings {
            address: "127.0.0.1:0".parse().expect("loopback"),
            ..Default::default()
        })
        .await
        .map_err(invalid)?;
    let address = format!("http://{}", listener.local_addr());
    let channel = Endpoint::from_shared(address.clone()).map_err(invalid)?.connect().await?;
    let mut client = client::Grpc::new(channel.clone());
    client.ready().await.map_err(invalid)?;
    let codec = ProstCodec::<CallerProbeGetRequest, CallerProbeGetResponse>::default();
    let status = client
        .unary(Request::new(CallerProbeGetRequest {}), PathAndQuery::from_static("/conformance.host.v1.CallerProbe/Get"), codec)
        .await
        .err()
        .ok_or_else(|| Error::Invalid("Call Probe: Client Reached Plugins-Only Handler".into()))?;
    require(status.code() == Code::PermissionDenied, "Call Probe: Direct Caller Was Not Refused")?;
    let failure = Failure::decode(status.details())?;
    wrong_caller(&failure, TARGET)?;
    // Private Host Services Have Neither Routes nor a Dynamic Client Escape.
    let mut direct = client::Grpc::new(channel.clone());
    direct.ready().await.map_err(invalid)?;
    let result: result::Result<Response<TimeGetResponse>, Status> =
        direct.unary(Request::new(TimeGetRequest {}), PathAndQuery::from_static("/harmos.host.v1.Time/Get"), ProstCodec::default()).await;
    require(result.unwrap_err().code() == Code::Unimplemented, "Call Host Time: Private Service Has a Route")?;
    let mut client = InvocationsClient::connect(address).await?;
    let host_contract = OperationFactory::protobuf::<clock::Get>().map_err(invalid)?;
    let status = client
        .create(InvocationsCreateRequest {
            operation: clock::Get::ID.into(),
            fingerprint: host_contract.contract().expect("private host contract").fingerprint().0.to_vec(),
            payload: TimeGetRequest {}.encode_to_vec(),
        })
        .await
        .unwrap_err();
    require(status.code() == Code::PermissionDenied, "Invoke Host Time: Dynamic Invocation Reached a Private Host Operation")?;
    let status = client
        .create(InvocationsCreateRequest::decode(request().as_slice())?)
        .await
        .err()
        .ok_or_else(|| Error::Invalid("Create Invocation: Client Reached Plugins-Only Handler".into()))?;
    require(status.code() == Code::PermissionDenied, "Create Invocation: Client Was Not Refused")?;
    let failure = Failure::decode(status.details())?;
    wrong_caller(&failure, TARGET)?;
    require(counter.load(Ordering::SeqCst) == 1, "Call Probe: Refused Client Reached Handler")?;

    // An Explicit Native Status Has the Same Kind, Action and Cause Through Create.
    let mut direct = client::Grpc::new(channel);
    direct.ready().await.map_err(invalid)?;
    let status = direct
        .unary(
            Request::new(PluginsGetRequest::default()),
            PathAndQuery::from_static("/conformance.host.v1.Refusal/Get"),
            ProstCodec::<PluginsGetRequest, PluginsGetResponse>::default(),
        )
        .await
        .err()
        .ok_or_else(|| invalid("Read Probe: native refusal succeeded"))?;
    require(status.code() == Code::Unimplemented, "Read Probe: direct status was rewritten")?;
    let contract = OperationFactory::protobuf::<NativeRefusal>().map_err(invalid)?;
    let encoded = client
        .create(InvocationsCreateRequest {
            operation: NativeRefusal::ID.into(),
            fingerprint: contract.contract().expect("native refusal contract").fingerprint().0.to_vec(),
            payload: PluginsGetRequest::default().encode_to_vec(),
        })
        .await
        .err()
        .ok_or_else(|| invalid("Read Probe: encoded refusal succeeded"))?;
    require(encoded.code() == status.code(), "Read Probe: encoded status was rewritten")?;
    require(encoded.message() == status.message(), "Read Probe: encoded action was rewritten")?;
    let failure = Failure::decode(encoded.details())?;
    require(failure == Failure::decode(status.details())?, "Read Probe: encoded cause was rewritten")?;
    require(failure.action == "Read Probe", "Read Probe: missing action")?;
    require(failure.cause.is_some_and(|cause| cause.code == "capability_absent"), "Read Probe: missing cause")?;
    drop(client);
    drop(listener);
    Ok(())
}

async fn plugin_refusal(runtime: &Runtime<App>, counter: &AtomicU64, controller: &Controller, address: &str) -> Result<()> {
    // Admit Only This Journey's SDK Contributions Without Taking Ownership of Its Process.
    let sample = Arc::new(Mutex::new(None));
    let mut client = InstanceClient::connect(address.to_owned()).await?;
    let description = client.get(InstanceGetRequest { revision: 1, ..Default::default() }).await?.into_inner();
    let fingerprint = description
        .declarations
        .as_ref()
        .and_then(|declarations| declarations.operations.iter().find(|operation| operation.id == "conformance.caller_refusal"))
        .ok_or_else(|| Error::Invalid("Call Operation: Missing Refusal Fixture".into()))?
        .fingerprint
        .clone();
    let forward = Forward {
        client,
        lanes: false,
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
        reconnect_delay: Duration::from_secs(1),
        stop_grace: Duration::from_secs(10),
        stream_window: NonZeroUsize::new(8).expect("stream window"),
        interval_window: Duration::from_secs(1),
        interval_lag: Default::default(),
    };
    let result = async {
        let attached = runtime.attach_grpc(Placement::Remote(address), settings).await.map_err(invalid)?;
        // The Unscoped Demand Grants the Refused Caller's Window Only While Its Stream Is Watched.
        let _watching = runtime
            .streams
            .current_publication("conformance.window_caller_refusal")
            .ok_or_else(|| Error::Invalid("Watch Caller Stream: Missing Stream".into()))?
            .watch();
        let result = async {
            // Refuse the Same CLIENTS Operation From Invoke and an Unanswered Samples Window.
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
                .ok_or_else(|| Error::Invalid("Window Caller Refusal: Missing Sample".into()))?;
            refusal(&payload)?;
            require(counter.load(Ordering::SeqCst) == 1, "Create Invocation: Refused Plugin Dispatched a Nested Call")?;

            // A Client's Nested Invocation Keeps the Plugin's Original Denial.
            let listener = runtime
                .grpc()
                .attach(grpc::Settings {
                    address: "127.0.0.1:0".parse().expect("loopback"),
                    ..Default::default()
                })
                .await
                .map_err(invalid)?;
            let mut invocations = InvocationsClient::connect(format!("http://{}", listener.local_addr())).await?;
            let status = invocations
                .create(InvocationsCreateRequest {
                    operation: "conformance.caller_refusal".into(),
                    fingerprint,
                    payload: fixture::payload(b"propagate"),
                })
                .await
                .err()
                .ok_or_else(|| Error::Invalid("Create Invocation: Nested Refusal Was Lost".into()))?;
            require(status.code() == Code::PermissionDenied, "Create Invocation: Nested Refusal Became Internal")?;
            wrong_caller(&Failure::decode(status.details())?, CREATE)?;
            drop(invocations);
            drop(listener);

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
    wrong_caller(&failure, CREATE)?;
    require(
        failure.classification == Classification::Refused as i32 && failure.operation == CREATE && !failure.plugin.is_empty(),
        "Create Invocation: Caller Refusal Changed Classification or Operation",
    )
}

fn wrong_caller(failure: &Failure, operation: &str) -> Result<()> {
    require(
        failure.classification == Classification::Refused as i32
            && failure.code == harmos_operations::WRONG_CALLER.to_string()
            && failure.action == "Call"
            && failure.operation == operation
            && failure.message.contains(operation)
            && failure.message.contains(&failure.action),
        "Call Operation: Wrong-Caller Failure Lost Its Kind, Code, Action or Operation",
    )
}

fn require(condition: bool, action: &str) -> Result<()> {
    if condition { Ok(()) } else { Err(Error::Invalid(action.into())) }
}

fn invalid(error: impl Display) -> Error {
    Error::Invalid(error.to_string())
}

/* === Tests === */

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn harmos_host_operations_are_unreachable_on_the_wire() {
        super::run(30, "").await.unwrap();
    }
}
