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
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

// Third Party Imports
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

// Local Imports
use harmos_domain::listeners::{Context as DeliveryContext, Delivery};
use harmos_runtime::operations::{OperationPolicy, RetryPolicy, WireTable};
use harmos_runtime::sidecar::{Refusal, Supervisor};

/* === Constants === */

const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(120),
    attempt_timeout: Duration::from_secs(120),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_runtime::stream(id = "consumption.payload", window = 16)]
#[harmos_runtime::message]
#[derive(Clone, Serialize, Deserialize)]
pub struct Payload {
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

#[harmos_runtime::message]
pub struct Acknowledged {
    pub process_id: u32,
    pub rows: u64,
    pub bytes: u64,
}

#[derive(Default)]
pub struct State {
    rows: AtomicU64,
    bytes: AtomicU64,
    changed: Notify,
}

pub struct Receiver {
    state: Arc<State>,
}

#[harmos_runtime::operation(resources = [State], id = "consumption.progress",
    policy = POLICY,
    response = Acknowledged,
)]
pub struct ProgressDeclaration {
    pub expected: u64,
}

#[harmos_runtime::operation(codec = manual, resources = [State], id = "consumption.consume", response = (),
    policy = POLICY, visibility = Shared)]
#[derive(Clone)]
struct Consumption(Delivery<Listener>);

impl harmos_runtime::Message for Consumption {
    const CODEC: &'static str = <Delivery<Listener> as harmos_runtime::Message>::CODEC;
    fn encode(&self) -> Vec<u8> {
        harmos_runtime::Message::encode(&self.0)
    }
    fn encoded_size(&self) -> usize {
        harmos_runtime::Message::encoded_size(&self.0)
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        <Delivery<Listener> as harmos_runtime::Message>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        <Delivery<Listener> as harmos_runtime::Message>::decode(bytes).map(Self)
    }
}
impl harmos_runtime::MemorySize for Consumption {
    fn memory_size(&self) -> usize {
        harmos_runtime::MemorySize::memory_size(&self.0)
    }
}
impl harmos_runtime::schema::Described for Consumption {
    const NAME: &'static str = <Delivery<Listener> as harmos_runtime::schema::Described>::NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] = <Delivery<Listener> as harmos_runtime::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = <Delivery<Listener> as harmos_runtime::schema::Described>::MARKER;
}

#[harmos_runtime::listener(
    id = "consumption.listener",
    stream = Payload,
    callback = "consumption.consume",
    policy = POLICY
)]
pub struct Listener;

/* === Enums === */

#[harmos_runtime::catalog]
pub enum Callbacks {
    Consume(harmos_runtime::listeners::Callback<Listener>),
    ProgressDeclaration(ProgressDeclaration),
}

#[harmos_runtime::catalog]
pub enum Listeners {
    Listener(Listener),
}

/* === Traits === */

/* === Implementations === */

impl harmos_operations::lifecycle::Policy for Receiver {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos_runtime::plugin(description = "Consumes Host Stream Rows And Acknowledges Verified Payload Bytes.", id = "consumption", provides = Callbacks, requires = (), listeners = Listeners)]
impl harmos_runtime::Plugin for Receiver {
    type Config = ();
    type Context = Supervisor;
    type Resources = State;
    type Error = Refusal;
    fn prepare(_: ()) -> Result<Self, Refusal> {
        Ok(Self { state: Arc::new(State::default()) })
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        Ok(())
    }
    fn resources(&self) -> Arc<State> {
        self.state.clone()
    }
    fn bind_operations(&self, table: &mut WireTable) -> Result<(), harmos_runtime::operations::ServingError> {
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<Consumption>(),
        )?;
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .operation::<ProgressDeclaration>(),
        )
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        Ok(())
    }
}

impl harmos_runtime::Operation for Consumption {
    type Error = Refusal;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let state = ctx.resource::<State>();
        let row = self.0;
        let result: ::core::result::Result<(), Self::Error> = async move {
            if !matches!(row.context(), DeliveryContext::Stream { .. }) {
                return Err(Refusal::of("expected stream delivery"));
            }
            let row = row.row();
            let expected = state.rows.load(Ordering::SeqCst);
            if row.sequence != expected || !row.bytes.iter().all(|byte| *byte == (expected % 251) as u8) {
                return Err(Refusal::of("payload loss, reordering or corruption"));
            }
            state.bytes.fetch_add(row.bytes.len() as u64, Ordering::SeqCst);
            state.rows.store(expected + 1, Ordering::SeqCst);
            state.changed.notify_waiters();
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for ProgressDeclaration {
    type Error = Refusal;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<Acknowledged>, Refusal> {
        let state = ctx.resource::<State>();
        let result: ::core::result::Result<Acknowledged, Self::Error> = async move {
            loop {
                let changed = state.changed.notified();
                let rows = state.rows.load(Ordering::SeqCst);
                if rows >= self.expected {
                    return Ok(Acknowledged {
                        process_id: std::process::id(),
                        rows,
                        bytes: state.bytes.load(Ordering::SeqCst),
                    });
                }
                changed.await;
            }
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
