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
use std::convert::Infallible;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::listener::Listener;
use harmos::observers::{ObserverActivation, ObserverBackend, ObserverDefinition};
use harmos::operations::{
    Context, OperationInstance, OperationOutcome, OperationPhase, OperationPolicy, RestartPolicy,
    RetryPolicy,
};
use schematic_app::{
    app::Runtime,
    lanes::Lanes,
    state::{Point, Rotation, Schematic, SymbolId},
    transactions::symbol::PlaceSymbol,
};

/* === Constants === */
const EDITS: u64 = 64;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Default)]
struct Trace {
    signal: OnceLock<Instant>,
    frontiers: OnceLock<(u64, u64)>,
    operation_stop: [OnceLock<Duration>; 2],
    observer_stop: OnceLock<Duration>,
    started: Notify,
    observer_ready: Notify,
    observer_stopped: AtomicUsize,
    runs: [AtomicUsize; 2],
    stopped: [AtomicUsize; 2],
    finalized: [AtomicUsize; 2],
}

#[harmos::operation(
    request = bool,
    response = (),
    id = "shutdown-exercise.hold",
    policy = OperationPolicy {
        timeout: Duration::from_secs(300),
        attempt_timeout: Duration::from_secs(300),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    }
)]
struct HoldDeclaration;

struct Hold {
    trace: Arc<Trace>,
    slot: Option<usize>,
}

struct Watch;

pub(super) struct Verified {
    pub(super) position: harmos::state::Position,
    pub(super) state: Schematic,
    pub(super) timings: serde_json::Value,
}

pub(super) struct Workload {
    listener: Listener,
    expected: (harmos::state::Position, Schematic),
    observer: OperationInstance<harmos::operations::resident::Activation>,
    trace: Arc<Trace>,
    running: OperationInstance<HoldDeclaration>,
    queued: OperationInstance<HoldDeclaration>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl ObserverBackend for Watch {
    type Resources = Trace;

    fn definition(&self) -> harmos::Result<ObserverDefinition> {
        Ok(ObserverDefinition {
            id: "shutdown-exercise.watch".into(),
            interval: Duration::from_millis(5),
            streams: Vec::new(),
            demand_keys: Vec::new(),
        })
    }

    async fn run(
        &self,
        trace: Arc<Trace>,
        mut activation: ObserverActivation,
    ) -> harmos::Result<()> {
        activation.ready();
        trace.observer_ready.notify_one();
        activation.stopping().await;
        if let Some(signal) = trace.signal.get() {
            let _ = trace.observer_stop.set(signal.elapsed());
        }
        trace.observer_stopped.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[harmos::operation(declaration = HoldDeclaration)]
impl Hold {
    async fn initialize(trace: Arc<Trace>, _: Context<'_>) -> Result<Self, Infallible> {
        Ok(Self { trace, slot: None })
    }

    async fn run(&mut self, _: Context<'_>, queued: &mut bool) -> Result<(), Infallible> {
        self.slot = Some(usize::from(*queued));
        self.trace.runs[self.slot.expect("running slot")].fetch_add(1, Ordering::SeqCst);
        self.trace.started.notify_one();
        std::future::pending().await
    }

    async fn stop(&mut self) -> Result<(), Infallible> {
        if let Some(signal) = self.trace.signal.get() {
            let _ = self.trace.operation_stop[self.slot.unwrap_or(1)].set(signal.elapsed());
        }
        self.trace.stopped[self.slot.unwrap_or(1)].fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Infallible> {
        self.trace.finalized[self.slot.unwrap_or(1)].fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl Workload {
    pub(super) async fn start(runtime: &Runtime) -> crate::Result<Self> {
        let trace = Arc::new(Trace::default());
        runtime.operations.register_operation::<Hold>(
            "shutdown-exercise",
            Lanes::Analysis,
            trace.clone(),
        )?;
        let running = runtime.operations.submit_as::<HoldDeclaration>(false)?;
        trace.started.notified().await;
        let queued = runtime.operations.submit_as::<HoldDeclaration>(true)?;
        runtime.operations.register_observer(Watch)?;
        let observer = runtime
            .operations
            .submit_with_resources_at("shutdown-exercise.watch", trace.clone())
            .await?;
        trace.observer_ready.notified().await;
        // Bound Accepted Edits Without Waiting For A Storage Barrier
        let base = runtime
            .state
            .state()
            .read(|state| state.symbols.keys().next_back().map_or(0, |id| id.0))
            .await?
            .value;
        base.checked_add(EDITS)
            .ok_or(harmos::Error::<Infallible>::ResourceExhausted {
                resource: "shutdown exercise symbol identities",
            })?;
        for offset in 1..=EDITS {
            let id = base + offset;
            runtime
                .state
                .commit(PlaceSymbol {
                    symbol: SymbolId(id),
                    reference: format!("R{id}"),
                    at: Point::default(),
                    rotation: Rotation::Zero,
                })
                .await?;
        }
        let expected = runtime.state.state().read(Clone::clone).await?;
        let mut listener = Listener::new(runtime)?;
        if let Err(error) = listener.start().await {
            listener.shutdown().await?;
            return Err(error.into());
        }

        let mut workload = Self {
            listener,
            expected: (expected.position, expected.value),
            observer,
            trace,
            running,
            queued,
        };
        if workload.running.status().phase() != OperationPhase::Running
            || workload.queued.status().phase() != OperationPhase::Queued
        {
            workload.listener.shutdown().await?;
            return Err(harmos::Error::<Infallible>::NotReady {
                domain: "shutdown exercise operations",
            }
            .into());
        }
        Ok(workload)
    }

    pub(super) fn signal(&self, received: Instant, runtime: &Runtime) {
        let _ = self.trace.signal.set(received);
        let _ = self
            .trace
            .frontiers
            .set((runtime.state.applied().0, runtime.state.stored().0));
    }

    pub(super) async fn verify(mut self, settlement: Duration) -> harmos::Result<Verified> {
        let listener = self.listener.verify();
        self.listener.shutdown().await?;
        listener?;
        if !self.observer.status().phase().is_terminal()
            || self.trace.observer_stopped.load(Ordering::SeqCst) != 1
        {
            return Err(harmos::Error::NotReady {
                domain: "shutdown exercise observer",
            });
        }
        self.observer.release().await?;
        for instance in [&self.running, &self.queued] {
            instance.settle().await?;
            let status = instance.status();
            if status.phase() != OperationPhase::Settled
                || status.outcome() != Some(OperationOutcome::Cancelled)
            {
                return Err(harmos::Error::NotReady {
                    domain: "shutdown exercise settlement",
                });
            }
        }
        // Queued Operations Never Initialize Or Acquire Cleanup Resources
        for (slot, runs) in [1, 0].into_iter().enumerate() {
            if self.trace.runs[slot].load(Ordering::SeqCst) != runs
                || self.trace.stopped[slot].load(Ordering::SeqCst) != runs
                || self.trace.finalized[slot].load(Ordering::SeqCst) != runs
            {
                return Err(harmos::Error::NotReady {
                    domain: "shutdown exercise cleanup",
                });
            }
        }
        let Some(&(accepted, stored)) = self.trace.frontiers.get() else {
            return Err(harmos::Error::NotReady {
                domain: "shutdown exercise storage frontiers",
            });
        };
        let pending = accepted
            .checked_sub(stored)
            .filter(|pending| *pending > 0 && *pending <= EDITS)
            .ok_or(harmos::Error::NotReady {
                domain: "shutdown exercise storage backlog",
            })?;
        let stops = self.trace.operation_stop.each_ref().map(OnceLock::get);
        let (Some(running), None, Some(observer)) =
            (stops[0], stops[1], self.trace.observer_stop.get())
        else {
            return Err(harmos::Error::NotReady {
                domain: "shutdown exercise timing",
            });
        };
        if [running, observer]
            .into_iter()
            .any(|time| *time > settlement)
        {
            return Err(harmos::Error::NotReady {
                domain: "shutdown exercise timing order",
            });
        }
        Ok(Verified {
            position: self.expected.0,
            state: self.expected.1,
            timings: serde_json::json!({
                "event": "shutdown-timings",
                "accepted_position": accepted,
                "stored_position": stored,
                "pending_entries": pending,
                "operation_stop_ns": running.as_nanos(),
                "observer_stop_ns": observer.as_nanos(),
                "settlement_ns": settlement.as_nanos(),
            }),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
