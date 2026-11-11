/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![cfg(feature = "simulation")]

/* === Modules === */

#[path = "../simulation/inverses.rs"]
mod inverses;

#[path = "../support/simulation.rs"]
mod oracle;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::convert::Infallible;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Third Party Imports
use support::{Bank, Deposit, Ledger, Noted, alice, catalog, pulse, streams};

// Local Imports
use crate::support;
use harmos_runtime::operations::RetryPolicy;
use harmos_runtime::simulation::entropy;
use harmos_runtime::{
    Application, DefinitionId, Identified, Runtime, Setup, runtime::Catalog, runtime::Registration, simulation::SimulatedRuntime,
    state::Focus, state::Kind, state::Transaction,
};

/* === Constants === */

static DIVERGENCE: AtomicUsize = AtomicUsize::new(0);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = ())]
#[derive(Clone)]
struct PendingDeclaration {}

static PENDING_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<PendingDeclaration>(
        "sim.pending",
        harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(30),
            attempt_timeout: Duration::from_millis(5),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    );

const PENDING_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<PendingDeclaration> =
    harmos_runtime::operations::OperationSpec::<PendingDeclaration>::new(&PENDING_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = u64)]
#[derive(Clone)]
struct AnsweringDeclaration {
    answer: u64,
}

static ANSWERING_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<AnsweringDeclaration>(
        "sim.answering",
        harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(30),
            attempt_timeout: Duration::from_millis(50),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    );

const ANSWERING_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<AnsweringDeclaration> =
    harmos_runtime::operations::OperationSpec::<AnsweringDeclaration>::new(&ANSWERING_DECLARATION_DESCRIPTION);

// The Draw Log Is Shared With The Scenario, So The Request Stays In Process.
#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = u64)]
#[derive(Clone)]
struct DrawingDeclaration {
    draws: Arc<Mutex<Vec<u64>>>,
}

static DRAWING_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<DrawingDeclaration>(
        "sim.drawing",
        harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(30),
            attempt_timeout: Duration::from_millis(100),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    );

const DRAWING_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<DrawingDeclaration> =
    harmos_runtime::operations::OperationSpec::<DrawingDeclaration>::new(&DRAWING_DECLARATION_DESCRIPTION);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DivergentState(usize);

#[derive(harmos_runtime::Described, Clone, Debug, serde::Serialize)]
struct DivergentChange;

#[derive(Clone, Debug)]
struct NoRecord;

struct Divergent;

#[derive(Clone)]
struct DivergentCatalog;

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = u64)]
#[derive(Clone)]
struct TimingItsOwnWaitDeclaration {
    wait_ms: u64,
}

static TIMING_ITS_OWN_WAIT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<TimingItsOwnWaitDeclaration>(
        "sim.timingitsownwait",
        harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(30),
            attempt_timeout: Duration::from_millis(10000),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    );

const TIMING_ITS_OWN_WAIT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<TimingItsOwnWaitDeclaration> =
    harmos_runtime::operations::OperationSpec::<TimingItsOwnWaitDeclaration>::new(&TIMING_ITS_OWN_WAIT_DECLARATION_DESCRIPTION);

/* === Enums === */

fn sim_operations_catalog() -> harmos_runtime::operations::Catalog {
    sim_operations_catalog_batch().named(concat!(module_path!(), "::SimOperations"))
}

fn sim_operations_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(pending_declaration_operation().expect("representable operation"));
    batch = batch.entry(answering_declaration_operation().expect("representable operation"));
    batch = batch.entry(drawing_declaration_operation().expect("representable operation"));
    batch = batch.entry(timing_its_own_wait_declaration_operation().expect("representable operation"));
    batch
}

/* === Traits === */

/* === Implementations === */

async fn pending_declaration_handler(
    _request: PendingDeclaration, context: harmos_runtime::operations::FunctionContext<PendingDeclaration>,
) -> Result<harmos_runtime::Response<()>, Infallible> {
    let host = context.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    host.borrow().scope().clock().sleep(Duration::from_secs(30)).await;
    Ok(().into())
}

fn pending_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        PENDING_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        pending_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

async fn answering_declaration_handler(
    request: AnsweringDeclaration, _context: harmos_runtime::operations::FunctionContext<AnsweringDeclaration>,
) -> Result<harmos_runtime::operations::Response<u64>, Infallible> {
    let host = _context.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let _context = host.borrow();
    let result: ::core::result::Result<u64, Infallible> = async move { Ok(request.answer) }.await;
    result.map(::core::convert::Into::into)
}

fn answering_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        ANSWERING_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        answering_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

impl harmos_runtime::Message for DrawingDeclaration {
    const CODEC: &'static str = "fixture.native";
    fn encode(&self) -> Vec<u8> {
        unreachable!("the in-process draw request is never encoded")
    }
    fn encoded_size(&self) -> usize {
        unreachable!("the in-process draw request is never encoded")
    }
    fn decoding_memory(_: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        Err(harmos_runtime::MessageError::borrowed("the in-process draw request is never decoded"))
    }
    fn decode(_: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        Err(harmos_runtime::MessageError::borrowed("the in-process draw request is never decoded"))
    }
}

impl harmos_runtime::MemorySize for DrawingDeclaration {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl harmos_runtime::schema::Described for DrawingDeclaration {
    const NAME: &'static str = concat!(module_path!(), "::DrawingDeclaration");
    const FIELDS: &'static [harmos_runtime::schema::Field] = &[];
    const MARKER: &'static [u8] = &harmos_runtime::schema::declaration::encode::<
        { harmos_runtime::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

async fn drawing_declaration_handler(
    request: DrawingDeclaration, _: harmos_runtime::operations::FunctionContext<DrawingDeclaration>,
) -> Result<harmos_runtime::Response<u64>, Infallible> {
    let draw = entropy();
    request.draws.lock().expect("draw lock is healthy").push(draw);
    Ok(draw.into())
}

fn drawing_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        DRAWING_DECLARATION_OPERATION,
        const { &[] },
        drawing_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

impl Transaction<DivergentState> for DivergentChange {
    type Error = Infallible;

    fn check(&self, _state: &DivergentState) -> Result<(), Self::Error> {
        Ok(())
    }

    fn apply(&self, state: &mut DivergentState) {
        state.0 += DIVERGENCE.fetch_add(1, Ordering::SeqCst);
    }
}

impl Identified for DivergentChange {
    const ID: &'static str = "sim.divergent";
}

impl harmos_runtime::Lifecycle for Divergent {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl Application for Divergent {
    type State = DivergentState;
    type Transaction = DivergentChange;
    type Record = NoRecord;
}

impl Registration<Divergent> for DivergentCatalog {
    fn register(self) -> Catalog<Divergent> {
        fn decode(_id: &DefinitionId, _version: u32, _bytes: &[u8]) -> Result<Option<DivergentChange>, harmos_runtime::state::DecodeError> {
            Ok(None)
        }

        Catalog::Transactions {
            states: None,
            records: None,
            definitions: vec![(Kind::Transaction, DefinitionId::new("sim.divergent"), 1)],
            decode: std::sync::Arc::new(decode),
            inverses: None,
        }
    }
}

async fn timing_its_own_wait_declaration_handler(
    request: TimingItsOwnWaitDeclaration, context: harmos_runtime::operations::FunctionContext<TimingItsOwnWaitDeclaration>,
) -> Result<harmos_runtime::operations::Response<u64>, Infallible> {
    let host = context.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let result: ::core::result::Result<u64, Infallible> = async move {
        let started = context.scope().now();
        context.scope().clock().sleep(Duration::from_millis(request.wait_ms)).await;
        Ok(u64::try_from((context.scope().now() - started).as_nanos()).expect("the scripted wait fits in u64 nanoseconds"))
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn timing_its_own_wait_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        TIMING_ITS_OWN_WAIT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        timing_its_own_wait_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

impl harmos_runtime::state::CatalogCodec for NoRecord {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        (DefinitionId::new("sim.none"), 1)
    }
    fn encode<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_unit()
    }
}

impl harmos_runtime::state::TransactionCatalog<DivergentState> for DivergentChange {}

/* === Public Functions === */

/* === Private Functions === */

fn bank(seed: u64, configure: impl Fn(&mut Setup<Bank>) + Send + Sync + 'static) -> SimulatedRuntime<Bank> {
    Runtime::<Bank>::simulate_with(Ledger::default(), (), seed, move |setup| {
        setup.catalog(sim_operations_catalog(), crate::support::SERIAL);
        setup.settings().register(catalog()).register(streams());
        configure(setup);
    })
    .expect("the simulation boots")
}

async fn resident(mut context: harmos_runtime::services::ServiceContext<Bank>) -> Result<(), std::io::Error> {
    context.ready();
    context.stopping().await;
    Ok(())
}

fn scripted(seed: u64) -> SimulatedRuntime<Bank> {
    oracle::scripted(bank(seed, oracle::configure))
}

fn scripted_wait(seed: u64) -> Duration {
    let mut sim = bank(seed, |_| {});
    let timed = sim.submit::<TimingItsOwnWaitDeclaration>(TimingItsOwnWaitDeclaration { wait_ms: 750 }).unwrap();
    sim.advance_ms(400);
    sim.advance_ms(350);

    Duration::from_nanos(*sim.settle(timed).expect("the scripted attempt answers"))
}

fn entropy_run(seed: u64) -> (SimulatedRuntime<Bank>, Vec<u64>) {
    let draws = Arc::new(Mutex::new(Vec::new()));
    let service_draws = Arc::clone(&draws);
    let mut sim = bank(seed, move |setup| {
        let service_draws = Arc::clone(&service_draws);
        setup.settings().simulated_service("entropy", move |mut context: harmos_runtime::services::ServiceContext<Bank>| {
            let draws = Arc::clone(&service_draws);
            async move {
                draws.lock().expect("draw lock is healthy").push(entropy());
                context.ready();
                context.stopping().await;
                Ok::<(), std::io::Error>(())
            }
        });
    });
    let handle = sim.submit::<DrawingDeclaration>(DrawingDeclaration { draws: Arc::clone(&draws) }).unwrap();
    sim.settle(handle).expect("the entropy job settles");
    let values = draws.lock().expect("draw lock is healthy").clone();
    (sim, values)
}

// The Feature Only Adds: A Spawned Start Stays Send Under Simulation.
#[tokio::test]
async fn start_stays_send() {
    let runtime = tokio::spawn(Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.catalog(sim_operations_catalog(), crate::support::SERIAL);
        setup.settings().register(catalog()).register(streams());
    }))
    .await
    .expect("the spawned start completes")
    .expect("the bank runtime starts");
    runtime.shutdown().await.unwrap();
}

mod atomic_records {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports
    use serde::{Deserialize, Serialize};

    // Local Imports
    use super::*;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct State(i64);

    #[derive(harmos_runtime::Described, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Observed {
        before: i64,
        amount: i64,
    }

    #[harmos_runtime::transaction(id = "sim.add",  inverse = Self)]
    #[derive(harmos_runtime::Described, Clone, Debug, Serialize, Deserialize)]
    struct Add(i64);

    struct Model;

    /* === Enums === */

    #[harmos_runtime::transactions(state = State, error = Infallible)]
    #[derive(Clone, Debug)]
    enum Changes {
        Add(Add),
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Facts {
        Observed(Observed),
    }
    impl From<Observed> for Facts {
        fn from(value: Observed) -> Self {
            Self::Observed(value)
        }
    }
    impl harmos_runtime::state::CatalogCodec for Facts {
        fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
            match self {
                Self::Observed(_) => <Observed as harmos_runtime::state::Record>::ENTRY.definition(),
            }
        }
        fn encode<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
            match self {
                Self::Observed(value) => serde::Serialize::serialize(value, serializer),
            }
        }
    }
    fn facts_registry() -> harmos_runtime::state::RecordRegistry<Facts> {
        harmos_runtime::state::RecordRegistry::new().entry(<Observed as harmos_runtime::state::Record>::ENTRY, Facts::Observed, |value| {
            let Facts::Observed(value) = value;
            Some(value)
        })
    }

    /* === Traits === */

    /* === Implementations === */

    impl harmos_runtime::state::Record for Observed {
        const ENTRY: harmos_runtime::state::RecordEntry<Self> = harmos_runtime::state::RecordEntry::new("sim.observed");
    }

    impl Transaction<State> for Add {
        type Error = Infallible;
        fn check(&self, _: &State) -> Result<(), Self::Error> {
            Ok(())
        }
        fn records(&self, state: &State, out: &mut harmos_runtime::state::Records) {
            out.push(Observed { before: state.0, amount: self.0 }).unwrap();
        }
        fn apply(&self, state: &mut State) {
            state.0 += self.0;
        }
    }

    impl harmos_runtime::state::Invertible<State> for Add {
        type Inverse = Self;

        fn inverse(&self, _: &State) -> Self::Inverse {
            Self(-self.0)
        }
    }

    impl harmos_runtime::Lifecycle for Model {
        type Config = ();
        type Error = std::convert::Infallible;
        type Context = harmos_runtime::Startup<Self>;

        async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
            Ok(Self)
        }
    }

    impl Application for Model {
        type State = State;
        type Transaction = Changes;
        type Record = Facts;
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */

    #[test]
    fn records_survive_reboot() {
        let make = || {
            Runtime::<Model>::simulate_with(State::default(), (), 17, |setup| {
                setup.settings().register(Changes::catalog()).register(facts_registry());
            })
            .unwrap()
        };
        let mut first = make();
        let mut second = make();
        for sim in [&mut first, &mut second] {
            let receipt = sim.commit(alice(), Add(7)).unwrap();
            assert_eq!((receipt.position, receipt.frontier), (harmos_runtime::state::Position(1), harmos_runtime::state::Position(2)));
            sim.assert_replay_equivalent();
            sim.reboot().unwrap();
            assert_eq!(sim.state().unwrap().value, State(7));
            sim.undo(alice(), Focus::all()).unwrap();
            sim.assert_replay_equivalent();
            assert_eq!(sim.state().unwrap().value, State(0));
            assert_eq!(sim.stop().unwrap(), harmos_runtime::state::Position(4));
        }
        first.assert_same(&second);
    }
}

/* === Tests === */

#[test]
fn attached_fold_refused() {
    let refusal = Runtime::<Bank>::simulate_with(Ledger::default(), (), 42, move |setup| {
        setup.settings().attach(support::Mirror::new().0);
    })
    .err()
    .expect("a live fold makes this composition incompatible with simulation");

    assert!(refusal.to_string().contains("attached fold"), "{refusal}");
}

#[test]
fn seed_replays_identically() {
    for seed in [0, 1, 7, 42, 99] {
        let first = scripted(seed);
        let second = scripted(seed);

        first.assert_same(&second);
        assert_eq!(first.trace(), second.trace());
    }
}

#[test]
fn backoff_delays_restart() {
    let mut sim = bank(7, |setup| {
        setup.settings().simulated_service_with(
            "adapter",
            harmos_runtime::ServicePolicy {
                restart: harmos_runtime::operations::RestartPolicy::OnFailure {
                    credit: 3,
                    sustained_health: Duration::from_millis(250),
                    backoff: harmos_runtime::operations::Backoff::Exponential {
                        initial: Duration::from_millis(5),
                        maximum: Duration::from_millis(5),
                        jitter: harmos_runtime::operations::Jitter::Deterministic,
                    },
                },
                ..harmos_runtime::services::POLICY
            },
            resident,
        );
    });

    sim.fault("adapter", "disconnected").expect("the running service receives its exit");
    assert_eq!(sim.service("adapter").restarts(), 0);
    sim.advance_ms(4);
    assert_eq!(sim.service("adapter").restarts(), 0);
    sim.advance_ms(4);
    assert_eq!(sim.service("adapter").restarts(), 1);
    assert_eq!(sim.service("adapter").exits(), ["environmental:disconnected"]);
}

#[test]
fn attempt_deadline_expires() {
    let mut sim = bank(7, |_| {});
    let deadline = sim.submit::<PendingDeclaration>(PendingDeclaration {}).unwrap();
    sim.advance_ms(5);
    assert!(matches!(sim.settle(deadline), Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::TimedOut { .. }))));
}

#[test]
fn scope_clock_scripted() {
    let measured = scripted_wait(11);

    // The Host Timer Queue Preserves the Exact Scripted Deadline
    assert_eq!(measured, Duration::from_millis(750), "the scope's clock is the one the script advances");
    assert_eq!(measured, scripted_wait(11), "and it measures the same duration every time the same script is run");
}

#[test]
fn oracle_catches_divergence() {
    DIVERGENCE.store(0, Ordering::SeqCst);
    let mut sim = Runtime::<Divergent>::simulate_with(DivergentState::default(), (), 9, |setup| {
        setup.settings().register(DivergentCatalog);
    })
    .expect("the deliberately divergent simulation boots");
    sim.commit(harmos_runtime::state::PrincipalId::new("tester"), DivergentChange).expect("the first application succeeds");

    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sim.assert_replay_equivalent();
    }));
    assert!(caught.is_err(), "the recovery oracle must catch divergence");
}

#[test]
fn entropy_draws_seeded() {
    let (first, first_draws) = entropy_run(99);
    let (second, second_draws) = entropy_run(99);

    assert_eq!(first_draws.len(), 2);
    assert_eq!(first_draws, second_draws);
    first.assert_same(&second);
}

#[test]
fn full_script_replays() {
    let mut sim = bank(101, oracle::configure);
    sim.commit(alice(), Deposit { account: support::HOUSE, amount: 3 }).expect("commit lands");
    sim.record(alice(), Noted("full script".to_owned())).expect("record lands");
    sim.publish(pulse(8)).expect("publish lands");
    let job = sim.submit::<AnsweringDeclaration>(AnsweringDeclaration { answer: 13 }).unwrap();
    assert_eq!(*sim.settle(job).expect("operation settles"), 13);
    sim.advance_ms(1);
    sim.fault("adapter", "reset").expect("fault is delivered");
    sim.advance_ms(20);
    assert_eq!(sim.stop().unwrap().0, 2);
    sim.reboot().expect("the in-memory source reboots");
    sim.assert_replay_equivalent();

    let state = sim.state().expect("state is readable");
    assert_eq!(state.position.0, 2);
    assert_eq!(state.value.balance(support::HOUSE), 3);
    assert!(sim.service("adapter").restarts() >= 2);
}
