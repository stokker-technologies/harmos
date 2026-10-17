/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::operations::{Calling, Lanes, Managed};
use harmos::Message;
use harmos::operations::{
    Operation, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, Visibility,
    WireOperation, WireTable,
};
use harmos_domain::listeners::{Context as DeliveryContext, Delivery};
use harmos_sidecar::{Refusal, Supervisor};

/* === Constants === */
static NEXT_SEED: AtomicUsize = AtomicUsize::new(1);
static SEEDS: Mutex<std::collections::BTreeMap<usize, Weak<Probe>>> =
    Mutex::new(std::collections::BTreeMap::new());

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Default)]
struct Probe {
    total: AtomicUsize,
    active: AtomicUsize,
    cleaned: AtomicBool,
}

struct Seed {
    id: usize,
}

struct Receiver {
    probe: Arc<Probe>,
}

struct Observe;

struct InFlight<'a>(&'a Probe);

#[harmos::message]
struct Observation {
    value: u32,
}

/* === Enums === */
#[harmos::catalog]
enum Callbacks {
    Observe(Observe),
}

/* === Traits === */

/* === Implementations === */
impl Seed {
    fn new(probe: &Arc<Probe>) -> Self {
        let id = NEXT_SEED
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |id| id.checked_add(1))
            .unwrap();
        let mut seeds = SEEDS.lock().unwrap();
        assert!(seeds.len() < 64, "fixture handoff stays bounded");
        assert!(seeds.insert(id, Arc::downgrade(probe)).is_none());
        Self { id }
    }
}

impl Drop for Seed {
    fn drop(&mut self) {
        SEEDS.lock().unwrap().remove(&self.id);
    }
}

impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

impl harmos::operations::lifecycle::Policy for Receiver {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos::plugin(description = "Native Callback Transport; Source Feed Ownership Is Tested By The Record Manager.", id = "listener", provides = Callbacks, requires = (), listeners = ())]
impl harmos::Plugin for Receiver {
    type Config = usize;
    type Context = Supervisor;
    type Resources = Probe;
    type Error = Refusal;
    fn prepare(id: usize) -> Result<Self, Self::Error> {
        let probe = SEEDS
            .lock()
            .unwrap()
            .remove(&id)
            .and_then(|probe| probe.upgrade())
            .ok_or_else(|| Refusal::of("original callback resources unavailable"))?;
        Ok(Self { probe })
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Self::Error> {
        Ok(())
    }
    fn resources(&self) -> Arc<Probe> {
        self.probe.clone()
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos::operations::ServingError> {
        table.bind_operation::<Observing>(self.resources())
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        assert_eq!(self.probe.active.load(Ordering::SeqCst), 0);
        self.probe.cleaned.store(true, Ordering::SeqCst);
        Ok(())
    }
}

impl OperationDeclaration for Observe {
    type Request = Delivery<Observation>;
    type Response = ();
    const ID: &'static str = "probe.observe";
    const VISIBILITY: Visibility = Visibility::Host;
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_millis(100),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
}

impl WireOperation for Observe {
    const VERSION: u32 = 1;
}

struct Observing {
    resources: Arc<Probe>,
}

impl Operation for Observing {
    type Declaration = Observe;
    type Resources = Probe;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(
        resources: Arc<Probe>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Refusal> {
        Ok(Self { resources })
    }
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut Delivery<Observation>,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<()>, Refusal> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            assert_eq!(
                request.context(),
                DeliveryContext::Record {
                    position: harmos::state::Position(1)
                }
            );
            self.resources.active.fetch_add(1, Ordering::SeqCst);
            let _active = InFlight(&self.resources);
            if request.row().value == 0 {
                std::future::pending::<()>().await;
            }
            if request.row().value == 99 {
                return Err(Refusal::of("the reaction refused"));
            }
            self.resources
                .total
                .fetch_add(request.row().value as usize, Ordering::SeqCst);
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn delivered(value: u32) -> Delivery<Observation> {
    Delivery::new(
        DeliveryContext::Record {
            position: harmos::state::Position(1),
        },
        Observation { value },
    )
    .unwrap()
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("callback_outcomes_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let probe = Arc::new(Probe::default());
                    let seed = Seed::new(&probe);
                    let capacity = std::num::NonZeroUsize::new(4).unwrap();
                    let source = harmos_sidecar::PluginNative::<Receiver>::new(capacity).unwrap();
                    let mut fixture = Managed::attach(
                        source,
                        serde_json::to_vec(&seed.id).unwrap(),
                        Lanes::NativeOpen,
                        capacity,
                        vec![],
                        std::convert::identity,
                    )
                    .await;
                    assert!(
                        fixture
                            .runtime
                            .operations
                            .submit_encoded(Observe::ID, delivered(7).encode_to_vec())
                            .is_err(),
                        "an unstaged callback is unavailable"
                    );
                    fixture.initialize().await.unwrap();
                    super::publications::ready(fixture.ready().clone()).await;
                    let operations = fixture.runtime.operations.clone();
                    fixture.call(delivered(7)).await.unwrap();
                    assert_eq!(probe.total.load(Ordering::SeqCst), 7);

                    for name in ["unknown", "harmos.record"] {
                        assert!(
                            operations
                                .submit_encoded(name, delivered(1).encode_to_vec())
                                .is_err(),
                            "unknown and retired dispatch cannot reach a callback"
                        );
                    }
                    for payload in [vec![0xff], delivered(99).encode_to_vec()] {
                        let error = operations
                            .submit_encoded(Observe::ID, payload)
                            .unwrap()
                            .await
                            .unwrap_err();
                        assert!(
                            matches!(error, harmos::Error::OperationFailed { .. }),
                            "malformed and refused callbacks retain operation failures: {error}"
                        );
                    }
                    assert_eq!(probe.total.load(Ordering::SeqCst), 7);
                    assert!(matches!(
                        fixture.call(delivered(0)).await,
                        Err(harmos::Error::OperationRefused(harmos::Refusal::TimedOut))
                    ));
                    fixture.call(delivered(3)).await.unwrap();
                    assert_eq!(probe.active.load(Ordering::SeqCst), 0);
                    assert_eq!(probe.total.load(Ordering::SeqCst), 10);

                    let stalled = tokio::spawn({
                        let operations = operations.clone();
                        async move {
                            let invocation = operations
                                .submit_encoded(Observe::ID, delivered(0).encode_to_vec())
                                .unwrap();
                            Calling(invocation).await
                        }
                    });
                    tokio::time::timeout(Duration::from_secs(2), async {
                        while probe.active.load(Ordering::SeqCst) == 0 {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    stalled.abort();
                    let _ = stalled.await;
                    fixture.call(delivered(2)).await.unwrap();
                    assert_eq!(probe.total.load(Ordering::SeqCst), 12);
                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();
                    assert_eq!(probe.active.load(Ordering::SeqCst), 0);
                    assert!(probe.cleaned.load(Ordering::SeqCst));
                });
        });
    }
}

/* === Tests === */
