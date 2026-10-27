/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
// Third Party Imports
use tokio::sync::watch;
use tokio::task::JoinHandle;
// Local Imports
use harmos_domain::catalog::Node;
use harmos_domain::inventory::{contracts, listeners, observers, publications};
use harmos_operations::lifecycle;
use harmos_sdk::operations::{ServingError, WireDeclaration};
use harmos_sdk::plugin::{Declared, Plugin};
use harmos_sidecar::operations::{OperationPolicy, RetryPolicy, WireTable};
use harmos_sidecar::{StreamSpec, Supervisor};

/* === Constants === */

const MAX_CASES: usize = 256;
const SDK_POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(120),
    attempt_timeout: Duration::from_secs(120),
    retries: RetryPolicy::Never,
};
const SDK: [harmos_domain::operations::OperationPolicy; 2] = [SDK_POLICY, SDK_POLICY];

#[used]
static IDENTITY: [u8; contracts::identity_len(
    "sidecar",
    TransportPlugin::ID,
    TransportPlugin::VERSION,
    TransportPlugin::DESCRIPTION,
    "()",
)] = contracts::encode_identity("sidecar", TransportPlugin::ID, TransportPlugin::VERSION, TransportPlugin::DESCRIPTION, "()");

#[used]
static CONTRACTS: [u8; contracts::encoded_tree_len(
    "sidecar",
    TransportPlugin::ID,
    TransportPlugin::PROVIDES,
    TransportPlugin::REQUIRES,
    &SDK,
)] = contracts::encode_tree("sidecar", TransportPlugin::ID, TransportPlugin::PROVIDES, TransportPlugin::REQUIRES, &SDK);

#[used]
static PUBLICATIONS: [u8; publications::encoded_len("sidecar", TransportPlugin::ID, TransportPlugin::STREAMS)] =
    publications::encode("sidecar", TransportPlugin::ID, TransportPlugin::STREAMS);

#[used]
static LISTENERS: [u8; listeners::encoded_len("sidecar", TransportPlugin::ID, &[])] =
    listeners::encode("sidecar", TransportPlugin::ID, &[]);

#[used]
static OBSERVERS: [u8; observers::encoded_tree_len("sidecar", TransportPlugin::ID, &[])] =
    observers::encode_tree("sidecar", TransportPlugin::ID, &[]);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_sdk::operation(
    harmos_path = ::harmos_sdk,
    resources = [Handler],
    id = "bench.start",
    response = (),
    policy = OperationPolicy {
        timeout: Duration::from_secs(120),
        attempt_timeout: Duration::from_secs(120),
        retries: RetryPolicy::Never,
    },
)]
#[derive(Clone, PartialEq)]
pub struct StartOperation {
    pub samples: u32,

    pub payload_bytes: u32,

    pub pace_bytes_per_second: u64,

    // Rows Per `host.publish` Call; Zero And One Publish Row By Row.
    pub batch: u32,
}

#[harmos_sidecar::message(harmos_path = ::harmos_sidecar)]
#[derive(Clone, PartialEq)]
pub struct Done {
    pub accepted: u64,

    pub attempted: u64,
}

#[harmos_sdk::operation(
    harmos_path = ::harmos_sdk,
    resources = [Handler],
    id = "bench.echo",
    response = EchoOperation,
    policy = OperationPolicy {
        timeout: Duration::from_secs(5),
        attempt_timeout: Duration::from_secs(5),
        retries: RetryPolicy::Never,
    },
)]
#[derive(Clone, PartialEq)]
pub struct EchoOperation {
    pub nonce: u64,

    pub payload: Vec<u8>,
}

#[harmos_sidecar::message(harmos_path = ::harmos_sidecar)]
#[derive(Clone, PartialEq)]
pub struct Sample {
    pub payload: Vec<u8>,
}

pub struct TransportPlugin {
    resources: Arc<Handler>,
    retiring: Option<VecDeque<JoinHandle<Result<(), harmos_sidecar::Refusal>>>>,
}

pub struct Handler {
    tasks: Mutex<VecDeque<JoinHandle<Result<(), harmos_sidecar::Refusal>>>>,
    closing: watch::Sender<bool>,
    host: Mutex<Option<Supervisor>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl StreamSpec for Done {
    type Row = Self;
    const ID: &'static str = "bench.done";
    const VERSION: u32 = 1;
    const WINDOW: usize = 1;
}

impl StreamSpec for Sample {
    type Row = Self;
    const ID: &'static str = "reading";
    const VERSION: u32 = 1;
    // A Call Carries At Most One Window Of Rows; Batches Here Reach 100
    const WINDOW: usize = 1024;
}

impl StartOperation {
    pub fn validate(&self) -> Result<(), harmos_sidecar::Refusal> {
        if self.samples == 0
            || self.samples > 2_000_000
            || self.payload_bytes == 0
            || self.payload_bytes > 1024 * 1024
            || u64::from(self.samples) * u64::from(self.payload_bytes) > 8 * 1024 * 1024 * 1024
            || self.pace_bytes_per_second > 4096 * 1024 * 1024
            || self.batch > 4096
        {
            return Err(harmos_sidecar::Refusal::of(
                "expected 1..=2000000 samples, 1..=1048576 bytes/sample, at most 8 GiB/case, 4096 MiB/s and 4096 rows/call",
            ));
        }
        Ok(())
    }
}

impl harmos_sidecar::Operation for EchoOperation {
    type Error = harmos_sidecar::Refusal;

    async fn run(self, _: harmos_sidecar::Context<'_, Self>) -> Result<harmos_sidecar::operations::Response<Self>, Self::Error> {
        Ok(self.into())
    }
}

impl harmos_sidecar::Operation for StartOperation {
    type Error = harmos_sidecar::Refusal;

    async fn run(self, ctx: harmos_sidecar::Context<'_, Self>) -> Result<harmos_sidecar::operations::Response<()>, Self::Error> {
        let resources = ctx.resource::<Handler>();
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.validate()?;
            let mut tasks = resources.tasks.lock().map_err(harmos_sidecar::Refusal::of)?;
            if *resources.closing.borrow() || tasks.len() >= MAX_CASES {
                return Err(harmos_sidecar::Refusal::of("transport fixture pump admission is closed or full"));
            }
            let host = resources
                .host
                .lock()
                .map_err(harmos_sidecar::Refusal::of)?
                .as_ref()
                .cloned()
                .ok_or_else(|| harmos_sidecar::Refusal::of("fixture generation calling context is unavailable"))?;
            let request = self;
            let closing = resources.closing.subscribe();
            tasks.push_back(tokio::spawn(publish(host, request, closing)));
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl lifecycle::Policy for TransportPlugin {
    const INITIALIZATION_POLICY: OperationPolicy = SDK_POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = SDK_POLICY;
}

impl Plugin for TransportPlugin {
    type Config = ();
    type Error = harmos_sidecar::Refusal;
    type Context = Supervisor;
    type Resources = Handler;

    fn prepare(_: ()) -> Result<Self, Self::Error> {
        Ok(Self {
            resources: Arc::new(Handler {
                tasks: Mutex::new(VecDeque::new()),
                closing: watch::channel(false).0,
                host: Mutex::new(None),
            }),
            retiring: None,
        })
    }
    fn resources(&self) -> Arc<Handler> {
        Arc::clone(&self.resources)
    }
    async fn initialize(&mut self, host: &Supervisor) -> Result<(), Self::Error> {
        *self.resources.host.lock().map_err(harmos_sidecar::Refusal::of)? = Some(host.clone());
        Ok(())
    }
    fn bind_operations(&self, table: &mut WireTable) -> Result<(), ServingError> {
        let mut dependencies = harmos_sidecar::Dependencies::new();
        dependencies.insert(Arc::clone(&self.resources)).map_err(|_| ServingError::Catalog)?;
        table.register(harmos_sidecar::RegistrationBatch::new(dependencies).operation::<EchoOperation>().operation::<StartOperation>())
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.resources.closing.send_replace(true);
        if self.retiring.is_none() {
            let tasks = &mut *self.resources.tasks.lock().map_err(harmos_sidecar::Refusal::of)?;
            self.retiring = Some(std::mem::take(tasks));
        }
        let tasks = self.retiring.as_mut().expect("original fixture pumps are retained");
        while let Some(task) = tasks.front_mut() {
            let result = task.await;
            tasks.pop_front();
            result.map_err(harmos_sidecar::Refusal::of)??;
        }
        Ok(())
    }
}

impl Declared for TransportPlugin {
    const ID: &'static str = "throughput-fixture";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const DESCRIPTION: &'static str = "Measures the native stdio transport.";
    const PROVIDES: &'static [Node<WireDeclaration>] =
        &[Node::Leaf(WireDeclaration::of::<EchoOperation>()), Node::Leaf(WireDeclaration::of::<StartOperation>())];
    const REQUIRES: &'static [Node<harmos_domain::plugin::Requirement>] = &[];
    const STREAMS: &'static [publications::Entry] = &[publications::Entry::of::<Sample>(), publications::Entry::of::<Done>()];
    fn markers() -> Result<Vec<&'static [u8]>, harmos_domain::schema::graph::Error> {
        harmos_domain::plugin::schema_markers::<Self>(
            vec![&IDENTITY, &CONTRACTS, &PUBLICATIONS, &LISTENERS, &OBSERVERS],
            &harmos_domain::config::SchemaSettings::default(),
        )
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn publish(host: Supervisor, start: StartOperation, mut closing: watch::Receiver<bool>) -> Result<(), harmos_sidecar::Refusal> {
    start.validate()?;
    let sample = Sample {
        payload: vec![b'x'; start.payload_bytes as usize],
    };
    let began = Instant::now();
    let mut accepted = 0_u64;
    let mut attempted = 0_u64;
    let publisher = host.publisher::<TransportPlugin>();
    let mut batch = publisher.batch::<Sample>();
    while accepted < u64::from(start.samples) {
        if *closing.borrow() {
            return Ok(());
        }
        let rows = u64::from(start.batch.max(1)).min(u64::from(start.samples) - accepted);
        batch.clear();
        for _ in 0..rows {
            batch.push(sample.clone());
        }
        attempted += rows;
        tokio::select! {
            _ = closing.changed() => return Ok(()),
            result = publisher.publish_batch_on::<Sample>(&batch) => result.map_err(harmos_sidecar::Refusal::from)?,
        }
        accepted += rows;
        tokio::select! {
            _ = closing.changed() => return Ok(()),
            _ = pace(&start, accepted, began) => {}
        }
    }
    let done = Done { accepted, attempted };
    tokio::select! {
        _ = closing.changed() => return Ok(()),
        result = publisher.publish_on::<Done>(&done) => result.map_err(harmos_sidecar::Refusal::from)?,
    }
    Ok(())
}

async fn pace(start: &StartOperation, accepted: u64, began: Instant) {
    if start.pace_bytes_per_second == 0 {
        return;
    }
    let emitted = accepted.saturating_mul(u64::from(start.payload_bytes));
    let target = Duration::from_secs_f64(emitted as f64 / start.pace_bytes_per_second as f64);
    if let Some(remaining) = target.checked_sub(began.elapsed()) {
        tokio::time::sleep(remaining).await;
    }
}

/* === Tests === */
