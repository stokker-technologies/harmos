/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Host Family Stories

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    mem,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime},
};

// Third Party Imports
use prost::{
    DecodeError, Message,
    bytes::{Buf, BufMut},
    encoding::{self, DecodeContext, WireType},
};
use prost_reflect::{MessageDescriptor, ReflectMessage};

// Local Imports
use super::support::{ChildRequest, Gate, Model, Waiting};
use harmos_operations::host::declarations::clock;
use harmos_runtime::facade::storage::PageRequest;
use harmos_runtime::operations::{
    ExecutionContext, NativeOperation, OperationFactory, ProtobufOperation, Registration, ReplaySafety, ServingError,
};
use harmos_runtime::{MemorySize, OperationPolicy};
use harmos_runtime::{
    Runtime,
    catalog::lifecycle_catalog,
    clock::Clock,
    facade::{
        invocations::{InvocationsCancelRequest, InvocationsGetRequest, InvocationsWatchRequest, InvocationsWatchResponse},
        status::{LogsListRequest, StatusGetRequest, StatusWatchRequest, StatusWatchResponse},
        time::TimeGetRequest,
    },
    operations::{CallContext, StreamSettings, native_stream},
    services::status::StatusSettings,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, Default)]
struct Counted {
    encoded: Arc<AtomicUsize>,
    value: u64,
}

struct Encoded;
struct Typed;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Message for Counted {
    fn encode_raw(&self, _: &mut impl BufMut) {
        self.encoded.fetch_add(1, Ordering::SeqCst);
    }
    fn merge_field(&mut self, tag: u32, wire_type: WireType, buf: &mut impl Buf, ctx: DecodeContext) -> Result<(), DecodeError> {
        encoding::skip_field(wire_type, tag, buf, ctx)
    }
    fn encoded_len(&self) -> usize {
        self.encoded.fetch_add(1, Ordering::SeqCst);
        0
    }
    fn clear(&mut self) {
        self.value = 0;
    }
}

impl ReflectMessage for Counted {
    fn descriptor(&self) -> MessageDescriptor {
        TimeGetRequest::default().descriptor()
    }
}

impl MemorySize for Counted {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
    }
}

impl ProtobufOperation for Encoded {
    type Request = Counted;
    type Response = Counted;
    type Endpoints = ();
    const ID: &'static str = "fixture.native_codec";
    const POLICY: OperationPolicy = super::support::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    async fn run(_: ExecutionContext, request: Counted, _: &()) -> Result<Counted, ServingError> {
        Ok(request)
    }
}

impl NativeOperation for Typed {
    type Request = Counted;
    type Response = Counted;
    type Endpoints = ();
    const ID: &'static str = "fixture.native_codec";
    const POLICY: OperationPolicy = super::support::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    async fn run(_: ExecutionContext, request: Counted, _: &()) -> Result<Counted, ServingError> {
        Ok(request)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test(start_paused = true)]
async fn native_time_and_invocations_keep_typed_endpoints() {
    let (clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let runtime = Runtime::<Model>::launch_with((), (), |setup| {
        setup.settings().clock(clock);
    })
    .await
    .unwrap();
    runtime.operations.register_catalog(lifecycle_catalog(), Default::default()).await.unwrap();
    let caller = runtime.for_call(&CallContext::default());
    assert_eq!(caller.time().get(TimeGetRequest {}).await.unwrap().now.unwrap().seconds, 0);

    // Observe and Cancel the Admitted Native Invocation
    let gate = Arc::new(Gate::default());
    let waiting = caller.submit::<Waiting>(ChildRequest(gate.clone()), ()).unwrap();
    gate.started.notified().await;
    let instance = waiting.id().value();
    let state = caller.invocations().get(InvocationsGetRequest { instance }).await.unwrap();
    assert_eq!(state.invocation.unwrap().phase, "running");
    let (output, input) = native_stream::<InvocationsWatchResponse>(StreamSettings {
        items: NonZeroUsize::new(8).unwrap(),
        bytes: NonZeroUsize::new(65536).unwrap(),
    })
    .unwrap();
    let watch = caller.invocations().watch(InvocationsWatchRequest { instance }, output).unwrap();
    let first = input.recv().await.unwrap();
    assert_eq!(first.invocation.as_ref().unwrap().phase, "running");
    drop(first);
    caller.invocations().cancel(InvocationsCancelRequest { instance, reason: "Complete".into() }).await.unwrap();
    assert!(waiting.await.is_err());
    let final_state = watch.await.unwrap();
    assert_eq!(final_state.invocation.as_ref().unwrap().phase, "settled");
    runtime.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn native_status_reads_the_registered_owner() {
    let runtime = Runtime::<Model>::launch((), ()).await.unwrap();
    let nz = |value| NonZeroUsize::new(value).unwrap();
    let _status = runtime
        .services()
        .configure_status(StatusSettings {
            application_name: "Facade Fixture".into(),
            application_version: "1.0.0".into(),
            host_id: "fixture".into(),
            history_items: nz(16),
            history_bytes: nz(65536),
            read_leases: nz(16),
            archive_bytes: nz(262144),
            range_bytes: nz(4096),
            read_timeout: Duration::from_secs(30),
            redaction_bytes: nz(4096),
            contributions: nz(8),
        })
        .unwrap();
    let caller = runtime.for_call(&CallContext::default());
    let status = caller.status().get(StatusGetRequest {}).await.unwrap();
    assert_eq!(status.status.unwrap().identity.unwrap().host_id, "fixture");
    let (output, input) = native_stream::<StatusWatchResponse>(StreamSettings { items: nz(8), bytes: nz(65536) }).unwrap();
    let watching = caller.status().watch(StatusWatchRequest { at: None }, output).unwrap();
    let event = input.recv().await.unwrap();
    assert_eq!(event.status.as_ref().unwrap().identity.as_ref().unwrap().host_id, "fixture");
    drop(event);
    watching.request_cancel();
    assert!(watching.await.is_err());
    let logs = caller
        .status()
        .logs_list(LogsListRequest {
            at: None,
            page: Some(PageRequest {
                page_size: 8,
                max_bytes: 65536,
                ..Default::default()
            }),
        })
        .await
        .unwrap();
    assert!(logs.events.len() <= 8);
    assert!(logs.resolved_at.is_some());
    runtime.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn attached_native_declaration_never_calls_the_wire_codec() {
    let runtime = Runtime::<Model>::launch((), ()).await.unwrap();
    assert!(OperationFactory::native::<clock::Get>().is_err());
    let factory = OperationFactory::protobuf::<Encoded>().unwrap().with_native::<Typed>().unwrap();
    runtime.register_batch(Registration::new(Default::default()).entry(factory)).await.unwrap();
    let encoded = Arc::new(AtomicUsize::new(0));
    let response = runtime.for_call(&CallContext::default()).call::<Typed>(Counted { encoded: encoded.clone(), value: 37 }).await.unwrap();
    assert_eq!(response.value, 37);
    assert_eq!(encoded.load(Ordering::SeqCst), 0);
    runtime.shutdown().await.unwrap();
}
