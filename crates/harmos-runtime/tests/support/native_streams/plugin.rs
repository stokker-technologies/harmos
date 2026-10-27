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
use harmos_runtime::operations::{Handler, OperationPolicy, RetryPolicy, WireTable};
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

#[harmos_runtime::operation(id = "progress",
    policy = POLICY,
    response = Acknowledged,
)]
pub struct ProgressDeclaration {
    pub expected: u64,
}

struct Consumption {
    state: Arc<State>,
}

struct Reporting {
    state: Arc<State>,
}

#[harmos_runtime::listener(
    id = "consumption.listener",
    stream = Payload,
    callback = "consumption_consume",
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
            .implementation::<harmos_runtime::listeners::Callback<Listener>, Consumption>(),
        )?;
        table.register(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(self.resources()).unwrap();
                dependencies
            })
            .implementation::<ProgressDeclaration, Reporting>(),
        )
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        Ok(())
    }
}

impl Handler for Consumption {
    type Request = <harmos_runtime::listeners::Callback<Listener> as harmos_runtime::OperationDeclaration>::Request;
    type Response = <harmos_runtime::listeners::Callback<Listener> as harmos_runtime::OperationDeclaration>::Response;

    type Error = Refusal;
    type StreamEndpoints = ();
    async fn initialize(context: harmos_runtime::ExecutionContext) -> Result<Self, Refusal> {
        let state = context.resource::<State>().expect("fixture dependency");
        Ok(Self { state })
    }
    async fn run(
        &mut self, _: harmos_runtime::ExecutionContext, row: &mut Delivery<Listener>, _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            if !matches!(row.context(), DeliveryContext::Stream { .. }) {
                return Err(Refusal::of("expected stream delivery"));
            }
            let row = row.row();
            let expected = self.state.rows.load(Ordering::SeqCst);
            if row.sequence != expected || !row.bytes.iter().all(|byte| *byte == (expected % 251) as u8) {
                return Err(Refusal::of("payload loss, reordering or corruption"));
            }
            self.state.bytes.fetch_add(row.bytes.len() as u64, Ordering::SeqCst);
            self.state.rows.store(expected + 1, Ordering::SeqCst);
            self.state.changed.notify_waiters();
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Handler for Reporting {
    type Request = ProgressDeclaration;
    type Response = <ProgressDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = Refusal;
    type StreamEndpoints = ();
    async fn initialize(context: harmos_runtime::ExecutionContext) -> Result<Self, Refusal> {
        let state = context.resource::<State>().expect("fixture registers its dependency");
        Ok(Self { state })
    }
    async fn run(
        &mut self, _: harmos_runtime::ExecutionContext, request: &mut ProgressDeclaration, _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<Acknowledged>, Refusal> {
        let result: ::core::result::Result<Acknowledged, Self::Error> = async move {
            loop {
                let changed = self.state.changed.notified();
                let rows = self.state.rows.load(Ordering::SeqCst);
                if rows >= request.expected {
                    return Ok(Acknowledged {
                        process_id: std::process::id(),
                        rows,
                        bytes: self.state.bytes.load(Ordering::SeqCst),
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
