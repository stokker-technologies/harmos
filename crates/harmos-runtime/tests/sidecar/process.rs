/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Nested Calls Between Host And Sidecar Over The In-Memory Transport

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{HostLanes, assembly, operations, service_status};
use crate::sidecar::support::Bank;
use crate::sidecar::witness::Witness;
use harmos_runtime::operations::{OperationPolicy, RestartPolicy, RetryPolicy, WireTable};
use harmos_runtime::sidecar::{Notice, Refusal, Supervised, Supervisor};
use harmos_runtime::{Runtime, services::ServiceContext, services::ServiceStatus};

/* === Constants === */

const CAPACITY: usize = 2;
const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(10),
    attempt_timeout: Duration::from_secs(10),
    retries: RetryPolicy::Never,
};

// The Echo Request That Pends Until The Caller Settles It.
const PENDING: u64 = 0;

// The Echo Request That Abandons The Generation Mid-Call.
const ABANDON: u64 = u64::MAX;

/* === Macros === */

macro_rules! operation {
    ($name:ident, $id:literal, $visibility:ident) => {
        #[harmos_runtime::operation(resources = [Witness, Supervisor], id = $id, response = Blob, policy = POLICY, visibility = $visibility)]
        #[derive(Clone)]
        pub struct $name {
            pub id: u64,
            pub data: Vec<u8>,
        }

        impl From<Blob> for $name {
            fn from(value: Blob) -> Self {
                Self { id: value.id, data: value.data }
            }
        }
    };
}

operation!(Echo, "process.echo", Private);

operation!(Nested, "process.nested", Host);

/* === Type Aliases === */

type Source = harmos_sidecar::PluginNative<Process>;
type Resident = Supervised<Bank, Source, fn(&ServiceContext<Bank>, Notice)>;

/* === Structs === */

#[harmos_runtime::stream(id = "process.receipt", window = 8)]
#[harmos_runtime::message]
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Blob {
    pub id: u64,
    pub data: Vec<u8>,
}

// The Sidecar: `process.nested` Publishes, Whose Host Handler Calls Back `process.echo`.
struct Process {
    marks: Arc<Witness>,
}

// Work The Echo Handler Owns Until Its Run Future Drops.
struct Work<'a> {
    marks: &'a Witness,
    id: u64,
}

// The Host Publication Handler: It Calls Back Into The Sidecar Before Publishing.
struct Callback {
    _resident: Arc<Resident>,
    echo: Arc<Forced>,
    calls: Arc<AtomicUsize>,
}

#[harmos_runtime::operation(codec = manual, resources = [Callback, harmos_runtime::operations::HostContext, harmos_sdk::operations::publication::PublicationAccess], id = "host.publish", response = (),
    policy = <harmos_operations::streams::Publish as harmos_runtime::OperationDeclaration>::POLICY, visibility = Shared)]
#[derive(Clone)]
struct CallbackPublication(harmos_operations::streams::Publish);

impl harmos_runtime::Message for CallbackPublication {
    const CODEC: &'static str = <harmos_operations::streams::Publish as harmos_runtime::Message>::CODEC;
    fn encode(&self) -> Vec<u8> {
        harmos_runtime::Message::encode(&self.0)
    }
    fn encoded_size(&self) -> usize {
        harmos_runtime::Message::encoded_size(&self.0)
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        <harmos_operations::streams::Publish as harmos_runtime::Message>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        <harmos_operations::streams::Publish as harmos_runtime::Message>::decode(bytes).map(Self)
    }
}
impl harmos_runtime::MemorySize for CallbackPublication {
    fn memory_size(&self) -> usize {
        harmos_runtime::MemorySize::memory_size(&self.0)
    }
}
impl harmos_runtime::schema::Described for CallbackPublication {
    const NAME: &'static str = <harmos_operations::streams::Publish as harmos_runtime::schema::Described>::NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] =
        <harmos_operations::streams::Publish as harmos_runtime::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = <harmos_operations::streams::Publish as harmos_runtime::schema::Described>::MARKER;
}

// The Echo Request The Host Callback Sends Instead Of The Publication Sequence.
#[derive(Default)]
struct Forced(Mutex<Option<u64>>);

struct Fixture {
    runtime: Runtime<Bank>,
    _lease: harmos_runtime::operations::Running<harmos_runtime::operations::resident::Activation>,
    marks: Arc<Witness>,
    echo: Arc<Forced>,
    calls: Arc<AtomicUsize>,
}

struct Finished {
    marks: Arc<Witness>,
    id: u64,
    kind: &'static str,
}

/* === Enums === */

#[harmos_runtime::catalog]
pub enum Operations {
    Echo(Echo),
    Nested(Nested),
}

/* === Traits === */

/* === Implementations === */

impl Drop for Finished {
    fn drop(&mut self) {
        if self.kind == "echo" {
            assert!(self.marks.seen(&format!("echo-dropped-{}", self.id)));
        }
        self.marks.mark(&format!("{}-finalized-{}", self.kind, self.id));
    }
}

impl Drop for Work<'_> {
    fn drop(&mut self) {
        self.marks.mark(&format!("echo-dropped-{}", self.id));
    }
}

impl harmos_operations::lifecycle::Policy for Process {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos_runtime::plugin(id = "process", provides = Operations, requires(harmos_operations::streams::Publish), streams(Blob))]
impl harmos_runtime::Plugin for Process {
    type Context = Supervisor;
    type Config = String;
    type Error = Refusal;
    type Resources = Witness;

    fn prepare(key: String) -> Result<Self, Refusal> {
        let marks = Witness::find(&key).ok_or_else(|| Refusal::of("the story witness is gone"))?;
        Ok(Self { marks })
    }

    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        self.marks.mark("started");
        Ok(())
    }

    fn resources(&self) -> Arc<Witness> {
        Arc::clone(&self.marks)
    }

    fn bind_operations(&self, table: &mut WireTable) -> Result<(), harmos_runtime::operations::ServingError> {
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<Echo>()
            .operation::<Nested>(),
        )
    }

    async fn stop(&mut self, context: &Supervisor) -> Result<(), Refusal> {
        if self.marks.seen("shutdown-publication") {
            let receipt = Blob { id: 47, data: b"shutdown".to_vec() };
            context.publisher::<Process>().publish_on::<Blob>(&receipt).await.map_err(Refusal::from)?;
            self.marks.mark("shutdown-publication-acknowledged");
        }
        self.marks.mark("shutdown");
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Refusal> {
        self.marks.mark("cleaned");
        Ok(())
    }
}

impl harmos_runtime::Operation for Echo {
    type Error = Refusal;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Blob>, Self::Error> {
        let marks = ctx.resource::<Witness>();
        marks.mark("echo-initialized");
        let _finish = Finished {
            marks: marks.clone(),
            id: self.id,
            kind: "echo",
        };
        marks.mark(&format!("echo-prepared-{}", self.id));
        marks.mark(&format!("echo-started-{}", self.id));
        let _work = Work { marks: &marks, id: self.id };
        if self.id == ABANDON {
            ctx.resource::<Supervisor>().abandon();
        }
        if self.id == PENDING || self.id == ABANDON {
            std::future::pending::<()>().await;
        }
        Ok(Blob { id: self.id, data: self.data }.into())
    }
}

impl harmos_runtime::Operation for Nested {
    type Error = Refusal;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Blob>, Self::Error> {
        let marks = ctx.resource::<Witness>();
        marks.mark("nested-initialized");
        let _finish = Finished {
            marks: marks.clone(),
            id: self.id,
            kind: "nested",
        };
        marks.mark(&format!("nested-prepared-{}", self.id));
        ctx.resource::<Supervisor>()
            .publisher::<Process>()
            .publish_on::<Blob>(&Blob { id: self.id, data: self.data.clone() })
            .await
            .map_err(Refusal::from)?;
        Ok(Blob { id: self.id, data: self.data }.into())
    }
}

impl harmos_runtime::Operation for CallbackPublication {
    type Error = harmos_runtime::Error;
    async fn run(mut self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::Response<()>> {
        let owner = ctx.resource::<Callback>();
        owner.calls.fetch_add(1, Ordering::SeqCst);
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let id = owner.echo.forced().unwrap_or_else(|| self.0.sequence.unwrap());
        let echoed = host.borrow().call::<Echo>(Echo::from(Blob { id, data: self.0.payload.clone() })).await?;
        assert_eq!(echoed.data, self.0.payload);
        let access = ctx.resource::<harmos_sdk::operations::publication::PublicationAccess>();
        let publication = harmos_sdk::operations::publication::Publication {
            stream: &self.0.stream,
            schema: self.0.schema,
            sequence: self.0.sequence,
            published_unix_micros: self.0.published_unix_micros,
            context: &mut self.0.context,
            payload: &mut self.0.payload,
            following: &mut self.0.following,
        };
        access.0.publish(ctx.call().clone(), publication).await.map_err(|source| harmos_runtime::Error::OperationFailed {
            operation: "host.publish".into(),
            source: Arc::from(source),
        })?;
        Ok(().into())
    }
}

impl Forced {
    fn store(&self, id: Option<u64>) {
        *self.0.lock().expect("the forced echo is never poisoned") = id;
    }

    fn forced(&self) -> Option<u64> {
        *self.0.lock().expect("the forced echo is never poisoned")
    }
}

impl Fixture {
    async fn new() -> Self {
        Self::with(Witness::open()).await
    }

    async fn with(marks: Arc<Witness>) -> Self {
        let source = Source::new(NonZeroUsize::new(CAPACITY).unwrap()).unwrap();
        let definition = operations::definition(&source, serde_json::to_vec(marks.key()).unwrap());
        let resident: Resident = Supervised::new(source, notices as fn(&ServiceContext<Bank>, Notice));
        let runtime = assembly()
            .operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(NonZeroUsize::new(harmos_sidecar::Frame::LIMIT * 4).unwrap()),
                ..Default::default()
            })
            .runtime()
            .await
            .unwrap();
        let echo = Arc::new(Forced::default());
        let calls = Arc::new(AtomicUsize::new(0));
        runtime
            .operations
            .register(harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new({
                    let mut dependencies = harmos_runtime::Dependencies::new();
                    dependencies
                        .insert(Arc::new(Callback {
                            _resident: Arc::new(resident.clone()),
                            echo: Arc::clone(&echo),
                            calls: Arc::clone(&calls),
                        }))
                        .unwrap();
                    dependencies
                })
                .operation::<CallbackPublication>(),
                HostLanes::Open,
            ))
            .await
            .unwrap();
        runtime
            .operations
            .register_service(
                harmos_runtime::services::ServiceDefinition {
                    id: "process".into(),
                    policy: harmos_runtime::ServicePolicy {
                        restart: RestartPolicy::Never,
                        ..harmos_runtime::services::POLICY
                    },
                },
                move |_: Arc<()>| {
                    harmos_runtime::sidecar::Managed::new_catalog::<Operations>(
                        resident.clone(),
                        definition.clone(),
                        HostLanes::Open,
                        NonZeroUsize::new(CAPACITY).unwrap(),
                        NonZeroUsize::new(4096).unwrap(),
                    )
                    .unwrap()
                },
            )
            .unwrap();
        let lease = runtime.operations.submit_resident("process").await.unwrap();
        service_status(runtime.operations.watch_residents(), "process", |status| matches!(status, ServiceStatus::Running { .. })).await;
        Self {
            runtime,
            _lease: lease,
            marks,
            echo,
            calls,
        }
    }

    async fn finish(self) {
        self.runtime.stop().await.unwrap();
        assert_eq!(self.marks.count("shutdown"), 1);
        assert_eq!(self.marks.count("cleaned"), 1);
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn notices(_: &ServiceContext<Bank>, _: Notice) {}

fn blob(id: u64, bytes: usize) -> Blob {
    Blob { id, data: vec![42; bytes] }
}

#[tokio::test]
async fn nested_calls_reenter() {
    let fixture = Fixture::new().await;
    let request = blob(41, 256 * 1024);

    let result = fixture.runtime.operation(Nested::from(request.clone())).run().await.unwrap();

    assert_eq!(*result, request);
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
    for mark in ["echo-prepared-1", "echo-dropped-1", "echo-finalized-1", "nested-finalized-41"] {
        assert_eq!(fixture.marks.count(mark), 1, "{mark}");
    }
    let rows = fixture.runtime.streams.publication("process.receipt", 1).unwrap();
    assert_eq!(rows.query(..).await.unwrap().count(), 1);
    fixture.finish().await;
}

#[tokio::test]
async fn cancellation_settles_slots() {
    let fixture = Fixture::new().await;
    fixture.echo.store(Some(PENDING));
    let pending = fixture.runtime.operation(Nested::from(blob(42, 512))).start().await.unwrap();

    // Cancelling The Outer Call Settles The Nested Slot It Holds
    fixture.marks.marked("echo-started-0").await;
    pending.cancel();
    assert!(pending.await.is_err());
    fixture.marks.marked("echo-finalized-0").await;
    fixture.marks.marked("nested-finalized-42").await;
    assert_eq!(fixture.marks.count("echo-dropped-0"), 1);
    assert_eq!(fixture.marks.count("echo-finalized-0"), 1);
    assert_eq!(fixture.marks.count("nested-finalized-42"), 1);

    // The Settled Slots Serve The Next Call
    fixture.echo.store(None);
    assert_eq!(*fixture.runtime.operation(Nested::from(blob(43, 512))).run().await.unwrap(), blob(43, 512));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 2);
    fixture.finish().await;
}

#[tokio::test(start_paused = true)]
async fn deadline_settles_nested() {
    let fixture = Fixture::new().await;
    fixture.echo.store(Some(PENDING));
    let pending = fixture.runtime.operation(Nested::from(blob(48, 512))).start().await.unwrap();
    fixture.marks.marked("echo-started-0").await;

    // The Original Handler Deadline Settles The Pending Nested Call
    let result = pending.await;
    assert!(matches!(result, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::TimedOut))));
    fixture.marks.marked("echo-finalized-0").await;
    fixture.marks.marked("nested-finalized-48").await;
    for mark in ["echo-prepared-0", "echo-dropped-0", "echo-finalized-0", "nested-initialized", "nested-finalized-48", "started"] {
        assert_eq!(fixture.marks.count(mark), 1, "{mark}");
    }
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);

    // The Deadline Refuses One Caller; The ServiceContext Serves The Next
    fixture.echo.store(None);
    assert_eq!(*fixture.runtime.operation(Nested::from(blob(49, 512))).run().await.unwrap(), blob(49, 512));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 2);
    fixture.finish().await;
}

#[tokio::test]
async fn disconnect_no_replay() {
    let fixture = Fixture::new().await;
    fixture.echo.store(Some(ABANDON));

    // The Abandoned Generation Fails The Call In Flight
    let outcome = fixture.runtime.operation(Nested::from(blob(46, 512))).run().await;
    assert!(outcome.is_err());
    service_status(fixture.runtime.operations.watch_residents(), "process", |status| matches!(status, ServiceStatus::Inactive { .. }))
        .await;

    // Nothing Replays The Call On Another Generation
    assert_eq!(fixture.marks.count("started"), 1);
    assert_eq!(fixture.marks.count(&format!("echo-started-{ABANDON}")), 1);
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
    assert!(fixture.runtime.stop().await.is_err());
}

#[tokio::test]
async fn shutdown_sdk_publication() {
    let marks = Witness::open();
    marks.mark("shutdown-publication");
    let source = Source::new(NonZeroUsize::new(CAPACITY).unwrap()).unwrap();
    let definition = operations::definition(&source, serde_json::to_vec(marks.key()).unwrap());
    let delivered = Arc::new(AtomicUsize::new(0));
    let expected = Arc::new(Mutex::new(None));
    let notices = Arc::clone(&delivered);
    let generation = Arc::clone(&expected);
    let resident = Supervised::new(source, move |_: &ServiceContext<Bank>, notice: Notice| {
        let Some(publication) = notice.published() else {
            return;
        };
        assert_eq!(publication.owner(), "process");
        assert_eq!(publication.stream(), "process.receipt");
        assert_eq!(publication.schema(), 1);
        assert_eq!(publication.generation(), *generation.lock().unwrap());
        assert_eq!(publication.wire_sequence(), 1);
        assert_eq!(publication.sequence(), 1);
        assert_eq!(publication.missed(), 0);
        assert_eq!(<Blob as harmos_runtime::Message>::decode(publication.payload()).unwrap(), Blob { id: 47, data: b"shutdown".to_vec() });
        notices.fetch_add(1, Ordering::SeqCst);
    });
    let calls = resident.clone();
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            crate::sidecar::publication_registration(Arc::new(resident.clone())),
            HostLanes::Open,
        ))
        .await
        .unwrap();
    runtime
        .operations
        .register_service(
            harmos_runtime::services::ServiceDefinition {
                id: "process".into(),
                policy: harmos_runtime::ServicePolicy {
                    restart: RestartPolicy::Never,
                    ..harmos_runtime::services::POLICY
                },
            },
            move |_: Arc<()>| {
                harmos_runtime::sidecar::Managed::new_catalog::<Operations>(
                    resident.clone(),
                    definition.clone(),
                    HostLanes::Open,
                    NonZeroUsize::new(CAPACITY).unwrap(),
                    NonZeroUsize::new(4096).unwrap(),
                )
                .unwrap()
            },
        )
        .unwrap();
    let _lease = runtime.operations.submit_resident("process").await.unwrap();
    service_status(runtime.operations.watch_residents(), "process", |status| matches!(status, ServiceStatus::Running { .. })).await;
    let native = calls.operations().unwrap();
    let scope = native.scope().unwrap();
    *expected.lock().unwrap() = Some(scope.generation().get());
    let background = scope.publication("process.receipt", 1).unwrap();
    assert_eq!(delivered.load(Ordering::SeqCst), 0);

    // Shutdown Publishes Through The Live Generation Before It Retires
    runtime.stop().await.unwrap();
    assert_eq!(delivered.load(Ordering::SeqCst), 1);
    for mark in ["shutdown-publication-acknowledged", "shutdown", "cleaned"] {
        assert_eq!(marks.count(mark), 1, "{mark}");
    }

    // The Retired Generation Refuses Later Publication
    assert!(scope.publication("process.receipt", 1).is_err());
    let late = Blob { id: 48, data: b"late".to_vec() };
    assert!(background.publish(harmos_runtime::Message::encode(&late), 0).is_err());
    assert_eq!(delivered.load(Ordering::SeqCst), 1);
}

/* === Tests === */
