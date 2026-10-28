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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::operations::{Calling, Lanes, Managed};
use harmos_domain::listeners::{Context as DeliveryContext, Delivery, Listener};
use harmos_runtime::Message;
use harmos_runtime::operations::{OperationDeclaration, OperationPolicy, RetryPolicy};
use harmos_sidecar::{Refusal, Supervisor};

/* === Constants === */

static NEXT_SEED: AtomicUsize = AtomicUsize::new(1);
static SEEDS: Mutex<std::collections::BTreeMap<usize, Weak<Probe>>> = Mutex::new(std::collections::BTreeMap::new());

/* === Macros === */

/* === Type Aliases === */

// The Callback Handler Is The Delivery Of Its Listener.
type Observe = harmos_runtime::listeners::Callback<Observed>;

/* === Structs === */

#[derive(Default)]
struct Probe {
    total: AtomicUsize,
    active: AtomicUsize,
    entered: tokio::sync::Notify,
    cleaned: AtomicBool,
}

struct Seed {
    id: usize,
}

struct Receiver {
    probe: Arc<Probe>,
}

// The Listener Declaration Whose Delivery Is The Callback Request.
struct Observed;

struct InFlight<'a>(&'a Probe);

#[harmos_runtime::message]
struct Observation {
    value: u32,
}

/* === Enums === */

#[harmos_runtime::catalog]
enum Callbacks {
    Observe(Observing),
}

/* === Traits === */

/* === Implementations === */

impl Seed {
    fn new(probe: &Arc<Probe>) -> Self {
        let id = NEXT_SEED.try_update(Ordering::SeqCst, Ordering::SeqCst, |id| id.checked_add(1)).unwrap();
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

impl harmos_operations::lifecycle::Policy for Receiver {
    const INITIALIZATION_POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

impl harmos_runtime::Lifecycle for Receiver {
    type Config = usize;
    type Context = Supervisor;
    type Error = Refusal;

    async fn prepare(id: usize, setup: &mut harmos_sdk::Setup<Self>) -> Result<Self, Self::Error> {
        let probe = SEEDS
            .lock()
            .unwrap()
            .remove(&id)
            .and_then(|probe| probe.upgrade())
            .ok_or_else(|| Refusal::of("original callback resources unavailable"))?;
        let plugin = Self { probe };
        setup.resource::<Probe>(plugin.probe.clone());
        Ok(plugin)
    }

    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Self::Error> {
        Ok(())
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

#[harmos_macros::plugin(harmos_path = ::harmos_runtime, description = "Native Callback Transport; Source Feed Ownership Is Tested By The Record Manager.", id = "listener", requires = (), listeners = ())]
impl harmos_runtime::Plugin for Receiver {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[Callbacks::CATALOG];
}

impl Listener for Observed {
    type Row = Observation;
    const CALLBACK: &'static str = "listener.observe";
    const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_millis(100),
        retries: RetryPolicy::Never,
    };
}

#[harmos_runtime::operation(codec = manual, resources = [Probe], id = "listener.observe", response = (),
    policy = Observed::POLICY, visibility = Shared)]
#[derive(Clone)]
struct Observing(Delivery<Observed>);

impl harmos_runtime::Message for Observing {
    const CODEC: &'static str = <Delivery<Observed> as harmos_runtime::Message>::CODEC;
    fn encode(&self) -> Vec<u8> {
        harmos_runtime::Message::encode(&self.0)
    }
    fn encoded_size(&self) -> usize {
        harmos_runtime::Message::encoded_size(&self.0)
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        <Delivery<Observed> as harmos_runtime::Message>::decoding_memory(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        <Delivery<Observed> as harmos_runtime::Message>::decode(bytes).map(Self)
    }
}
impl harmos_runtime::MemorySize for Observing {
    fn memory_size(&self) -> usize {
        harmos_runtime::MemorySize::memory_size(&self.0)
    }
}
impl harmos_runtime::schema::Described for Observing {
    const NAME: &'static str = <Delivery<Observed> as harmos_runtime::schema::Described>::NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] = <Delivery<Observed> as harmos_runtime::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = <Delivery<Observed> as harmos_runtime::schema::Described>::MARKER;
}

impl harmos_runtime::Operation for Observing {
    type Error = Refusal;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Refusal> {
        let resources = ctx.resource::<Probe>();
        let request = self.0;
        let result: ::core::result::Result<(), Self::Error> = async move {
            assert_eq!(
                request.context(),
                DeliveryContext::Record {
                    position: harmos_runtime::state::Position(1)
                }
            );
            resources.active.fetch_add(1, Ordering::SeqCst);
            resources.entered.notify_one();
            let _active = InFlight(&resources);
            if request.row().value == 0 {
                std::future::pending::<()>().await;
            }
            if request.row().value == 99 {
                return Err(Refusal::of("the reaction refused"));
            }
            resources.total.fetch_add(request.row().value as usize, Ordering::SeqCst);
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn delivered(value: u32) -> Delivery<Observed> {
    Delivery::new(
        DeliveryContext::Record {
            position: harmos_runtime::state::Position(1),
        },
        Observation { value },
    )
    .unwrap()
}

#[tokio::test]
async fn callback_outcomes_cleanup() {
    let probe = Arc::new(Probe::default());
    let seed = Seed::new(&probe);
    let capacity = std::num::NonZeroUsize::new(4).unwrap();
    let source = harmos_sidecar::PluginNative::<Receiver>::new(capacity).unwrap();
    let mut fixture =
        Managed::attach(source, serde_json::to_vec(&seed.id).unwrap(), Lanes::Open, capacity, vec![], std::convert::identity).await;
    assert!(
        fixture.runtime.operations.submit_encoded(&format!("listener.{}", Observe::ID), delivered(7).encode()).is_err(),
        "an unstaged callback is unavailable"
    );
    fixture.initialize().await.unwrap();
    super::publications::ready(fixture.ready().clone()).await;
    let operations = fixture.runtime.operations.clone();
    fixture.call::<Observe>(delivered(7)).await.unwrap();
    assert_eq!(probe.total.load(Ordering::SeqCst), 7);

    for name in ["unknown", "harmos.record"] {
        assert!(operations.submit_encoded(name, delivered(1).encode()).is_err(), "unknown and retired dispatch cannot reach a callback");
    }
    for payload in [vec![0xff], delivered(99).encode()] {
        let error = operations.submit_encoded(&format!("listener.{}", Observe::ID), payload).unwrap().await.unwrap_err();
        assert!(
            matches!(error, harmos_runtime::Error::OperationFailed { .. }),
            "malformed and refused callbacks retain operation failures: {error}"
        );
    }
    assert_eq!(probe.total.load(Ordering::SeqCst), 7);
    assert!(matches!(
        fixture.call::<Observe>(delivered(0)).await,
        Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::TimedOut))
    ));
    fixture.call::<Observe>(delivered(3)).await.unwrap();
    assert_eq!(probe.active.load(Ordering::SeqCst), 0);
    assert_eq!(probe.total.load(Ordering::SeqCst), 10);

    let stalled = tokio::spawn({
        let operations = operations.clone();
        async move {
            let invocation = operations.submit_encoded(&format!("listener.{}", Observe::ID), delivered(0).encode()).unwrap();
            Calling(invocation).await
        }
    });
    while probe.active.load(Ordering::SeqCst) == 0 {
        probe.entered.notified().await;
    }
    stalled.abort();
    let _ = stalled.await;
    fixture.call::<Observe>(delivered(2)).await.unwrap();
    assert_eq!(probe.total.load(Ordering::SeqCst), 12);
    fixture.shutdown().await.unwrap();
    fixture.runtime.shutdown().await.unwrap();
    assert_eq!(probe.active.load(Ordering::SeqCst), 0);
    assert!(probe.cleaned.load(Ordering::SeqCst));
}

/* === Tests === */
