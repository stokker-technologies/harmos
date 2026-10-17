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
use harmos_runtime::operations::{
    Operation, OperationPolicy, RestartPolicy, RetryPolicy, WireTable,
};
use harmos_runtime::sidecar::{Refusal, Supervisor};

/* === Constants === */
const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(30),
    attempt_timeout: Duration::from_secs(30),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
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

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "consumption.progress",
    version = 1,
    policy = POLICY,
    response = Acknowledged,
)]
pub struct Progress {
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
    callback = "consumption.consume",
    policy = POLICY
)]
pub struct Listener;

/* === Enums === */
#[harmos_runtime::catalog]
pub enum Callbacks {
    Consume(Delivery<Listener>),
    Progress(Progress),
}

#[harmos_runtime::catalog]
pub enum Listeners {
    Listener(Listener),
}

/* === Traits === */

/* === Implementations === */
impl harmos_runtime::operations::lifecycle::Policy for Receiver {
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
        Ok(Self {
            state: Arc::new(State::default()),
        })
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        Ok(())
    }
    fn resources(&self) -> Arc<State> {
        self.state.clone()
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos_runtime::operations::ServingError> {
        table.bind_operation::<Consumption>(self.resources())?;
        table.bind_operation::<Reporting>(self.resources())
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        Ok(())
    }
}

impl Operation for Consumption {
    type Declaration = Delivery<Listener>;
    type Resources = State;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(state: Arc<State>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { state })
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        row: &mut Delivery<Listener>,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            if !matches!(row.context(), DeliveryContext::Stream { .. }) {
                return Err(Refusal::of("expected stream delivery"));
            }
            let row = row.row();
            let expected = self.state.rows.load(Ordering::SeqCst);
            if row.sequence != expected
                || !row.bytes.iter().all(|byte| *byte == (expected % 251) as u8)
            {
                return Err(Refusal::of("payload loss, reordering or corruption"));
            }
            self.state
                .bytes
                .fetch_add(row.bytes.len() as u64, Ordering::SeqCst);
            self.state.rows.store(expected + 1, Ordering::SeqCst);
            self.state.changed.notify_waiters();
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Operation for Reporting {
    type Declaration = Progress;
    type Resources = State;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(state: Arc<State>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { state })
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut Progress,
        _endpoints: &mut Self::Endpoints,
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
