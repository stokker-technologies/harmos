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
#[path = "../support/mod.rs"]
mod support;
#[path = "../support/vault.rs"]
mod vault;
mod atomic_records {
    use super::*;
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct State(i64);
    #[harmos_runtime::record(id = "sim.observed")]
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Observed {
        before: i64,
        amount: i64,
    }
    #[harmos_runtime::transaction(id = "sim.add",  inverse = Self)]
    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct Add(i64);
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
    #[harmos_runtime::transactions(state = State, error = Infallible)]
    #[derive(Clone, Debug)]
    enum Changes {
        Add(Add),
    }
    #[harmos_runtime::records]
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Facts {
        Observed(Observed),
    }
    struct Model;
    impl harmos_runtime::Lifecycle for Model {
        type Config = ();
        type Error = std::convert::Infallible;
        type Context = harmos_runtime::Startup<Self>;

        async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
            Ok(Self)
        }

        async fn initialize(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
            Ok(())
        }

        async fn stop(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
            Ok(())
        }

        async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
            Ok(())
        }
    }

    impl Application for Model {
        type Lanes = Lanes;
        type State = State;
        type Transaction = Changes;
        type Record = Facts;
        type Streams = ();
    }

    #[test]
    fn records_survive_reboot() {
        let make = || {
            Runtime::<Model>::simulate_with(State::default(), (), 17, |setup| {
                setup.settings().register(Changes::catalog()).register(Facts::catalog());
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

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::convert::Infallible;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos_runtime::operations::{OperationPolicy, RetryPolicy};
use harmos_runtime::sidecar::{Notice, Supervised};
use harmos_runtime::simulation::entropy;
use harmos_runtime::{
    Application, DefinitionId, Identified, Runtime, Setup, evidence::EvidenceKind, runtime::Catalog, runtime::Registration,
    simulation::SimulatedRuntime, state::Focus, state::Kind, state::Transaction,
};
use harmos_sidecar::{Attach, PluginNative, Supervisor};
use support::{Bank, Deposit, Ledger, Noted, alice, catalog, pulse, streams};

/* === Constants === */

static DIVERGENCE: AtomicUsize = AtomicUsize::new(0);

/* === Macros === */

/* === Type Aliases === */

type IdleManaged =
    harmos_runtime::sidecar::Managed<Bank, PluginNative<Idle>, fn(&harmos_runtime::services::ServiceContext<Bank>, Notice), Lanes>;

/* === Structs === */

#[harmos_runtime::operation(id = "sim.pending",
    response = (),
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_millis(5),
        retries: RetryPolicy::Never,
    }
)]
struct PendingDeclaration {}

#[harmos_runtime::operation(resources = [harmos_runtime::operations::HostContext], id = "sim.answering",
    response = u64,
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_millis(50),
        retries: RetryPolicy::Never,
    }
)]
struct AnsweringDeclaration {
    answer: u64,
}

// The Draw Log Is Shared With The Scenario, So The Request Stays In Process.
#[harmos_runtime::operation(codec = manual, id = "sim.drawing",
    response = u64,
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_millis(100),
        retries: RetryPolicy::Never,
    }
)]
#[derive(Clone)]
struct DrawingDeclaration {
    draws: Arc<Mutex<Vec<u64>>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DivergentState(usize);

#[derive(Clone, Debug, serde::Serialize)]
struct DivergentChange;

#[derive(Clone, Debug)]
struct NoRecord;

struct Divergent;

#[derive(Clone)]
struct DivergentCatalog;

#[harmos_runtime::operation(resources = [harmos_runtime::operations::HostContext], id = "sim.timingitsownwait",
    response = u64,
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_millis(10000),
        retries: RetryPolicy::Never,
    }
)]
struct TimingItsOwnWaitDeclaration {
    wait_ms: u64,
}

struct Capture;

#[derive(Clone)]
struct Capturing;

struct Idle;

struct IdleResident {
    managed: IdleManaged,
}

/* === Enums === */

use support::Lanes;

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Operation for PendingDeclaration {
    type Error = Infallible;

    async fn run(self, _: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<()>, Self::Error> {
        std::future::pending().await
    }
}

impl harmos_runtime::operations::Operation for AnsweringDeclaration {
    type Error = Infallible;

    async fn run(self, _context: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<u64>, Infallible> {
        let host = _context.resource::<harmos_runtime::operations::HostContext>();
        let _context = host.borrow();
        let result: ::core::result::Result<u64, Self::Error> = async move { Ok(self.answer) }.await;
        result.map(::core::convert::Into::into)
    }
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

impl harmos_runtime::Operation for DrawingDeclaration {
    type Error = Infallible;

    async fn run(self, _: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<u64>, Self::Error> {
        let draw = entropy();
        self.draws.lock().expect("draw lock is healthy").push(draw);
        Ok(draw.into())
    }
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
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("sim.divergent"), 1)
    }
}
impl harmos_runtime::Lifecycle for Divergent {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }

    async fn initialize(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn stop(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl Application for Divergent {
    type Lanes = Lanes;
    type State = DivergentState;
    type Transaction = DivergentChange;
    type Record = NoRecord;
    type Streams = ();
}

impl Registration<Divergent> for DivergentCatalog {
    fn register(self) -> Catalog<Divergent> {
        fn decode(_id: &DefinitionId, _version: u32, _bytes: &[u8]) -> Result<Option<DivergentChange>, harmos_runtime::state::DecodeError> {
            Ok(None)
        }

        Catalog::Transactions {
            definitions: vec![(Kind::Transaction, DefinitionId::new("sim.divergent"), 1)],
            decode,
            inverses: None,
        }
    }
}

impl harmos_runtime::operations::Operation for TimingItsOwnWaitDeclaration {
    type Error = Infallible;

    async fn run(self, context: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<u64>, Infallible> {
        let host = context.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let result: ::core::result::Result<u64, Self::Error> = async move {
            let started = context.scope().now();
            tokio::time::sleep(Duration::from_millis(self.wait_ms)).await;
            Ok(u64::try_from((context.scope().now() - started).as_nanos()).expect("the scripted wait fits in u64 nanoseconds"))
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Identified for Capture {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("capture"), 1)
    }
}

impl EvidenceKind for Capture {
    type Details = ();
}

impl harmos_runtime::services::Resident<Bank> for Capturing {
    type Error = std::io::Error;

    async fn run(&mut self, mut context: harmos_runtime::services::ServiceContext<Bank>) -> Result<(), std::io::Error> {
        let evidence = context.evidence().clone();
        let inherited = evidence.list("run").len() as i64;
        context
            .state()
            .commit(Deposit {
                account: support::HOUSE,
                amount: inherited.max(0),
            })
            .await
            .ok();

        let key = format!("boot-{inherited}.log");
        let draft = evidence.open::<Capture>("run", key, ()).await.map_err(std::io::Error::other)?;
        draft.write(b"one boot's worth").await.map_err(std::io::Error::other)?;
        evidence.finalize(&draft).await.map_err(std::io::Error::other)?;

        // A second draft is deliberately left open: no fact names it, so the
        // next boot must not see it.
        evidence
            .open::<Capture>("run", "unfinished.log", ())
            .await
            .map_err(std::io::Error::other)?
            .write(b"half")
            .await
            .map_err(std::io::Error::other)?;

        context.ready();
        context.stopping().await;

        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl harmos_runtime::state::CatalogCodec for NoRecord {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        (DefinitionId::new("sim.none"), 1)
    }
    fn encode<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_unit()
    }
}

impl harmos_runtime::state::RecordCatalog for NoRecord {
    fn restore(_: &DefinitionId, _: u32, _: &serde_json::Value) -> Result<Option<Self>, serde_json::Error> {
        Ok(None)
    }
}

impl harmos_runtime::state::TransactionCatalog<DivergentState> for DivergentChange {}

impl harmos_operations::lifecycle::Policy for Idle {
    const INITIALIZATION_POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

impl harmos_runtime::Lifecycle for Idle {
    type Context = Supervisor;
    type Config = ();
    type Error = Infallible;
    async fn prepare(_: (), _: &mut Setup<Self>) -> Result<Self, Infallible> {
        Ok(Self)
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Infallible> {
        Ok(())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Infallible> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
}

#[harmos_runtime::sidecar(id = "idle", requires = ())]
impl harmos_runtime::Plugin for Idle {
    const CATALOGS: &'static [harmos_runtime::operations::Catalog] = &[];
}

impl IdleResident {
    fn new() -> Self {
        let capacity = NonZeroUsize::new(1).unwrap();
        let source = PluginNative::<Idle>::new(capacity).expect("the idle SDK declaration is valid");
        let definition = harmos_runtime::plugin::PluginDefinition {
            id: source.owner().into(),
            version: "1.0.0".parse().unwrap(),
            artifact: harmos_runtime::plugin::PluginArtifact::LocalSidecar { revision: "simulation".into() },
            configuration: serde_json::to_vec(&()).unwrap(),
            requires: vec![],
            contributions: harmos_runtime::plugin::PluginContributions {
                operations: source.operations().unwrap(),
                wire_contracts: source.operation_contracts().unwrap().unwrap(),
                required_contracts: source.required_contracts().unwrap(),
                lifecycle: source.sdk_lifecycle().unwrap(),
                streams: source.publications(),
                observers: source.observers(),
                listeners: source.listener_declarations().unwrap().unwrap_or_default(),
                ..Default::default()
            },
        };
        let resident = Supervised::new(
            source,
            (|_: &harmos_runtime::services::ServiceContext<Bank>, _: Notice| {})
                as fn(&harmos_runtime::services::ServiceContext<Bank>, Notice),
        );
        let managed = harmos_runtime::sidecar::Managed::new(resident, definition, Lanes::Open, capacity, NonZeroUsize::new(4096).unwrap())
            .expect("the frozen idle graph is valid");
        Self { managed }
    }
}

impl Clone for IdleResident {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl harmos_runtime::services::Resident<Bank> for IdleResident {
    type Error = harmos_runtime::sidecar::ManagedError;
    async fn run(&mut self, context: harmos_runtime::services::ServiceContext<Bank>) -> Result<(), Self::Error> {
        harmos_runtime::services::Resident::run(&mut self.managed, context).await
    }
    async fn stop(&mut self) -> Result<(), Self::Error> {
        harmos_runtime::services::Resident::stop(&mut self.managed).await
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        harmos_runtime::services::Resident::cleanup(&mut self.managed).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[harmos_runtime::catalog]
enum SimOperations {
    Pending(PendingDeclaration),
    Answering(AnsweringDeclaration),
    Drawing(DrawingDeclaration),
    Timing(TimingItsOwnWaitDeclaration),
}

fn bank(seed: u64, configure: impl Fn(&mut Setup<Bank>) + Send + Sync + 'static) -> SimulatedRuntime<Bank> {
    Runtime::<Bank>::simulate_with(Ledger::default(), (), seed, move |setup| {
        setup.operations::<SimOperations>(Lanes::Serial);
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

fn vault(name: &str) -> vault::Directory {
    let prefix = format!("harmos-sim-evidence-{name}-");
    let directory = tempfile::Builder::new().prefix(&prefix).tempdir().expect("the temporary directory is writable");
    vault::Directory::at(&Arc::new(directory))
}

fn capturing(seed: u64, name: &str) -> SimulatedRuntime<Bank> {
    let directory = vault(name);
    let mut sim = bank(seed, move |setup| {
        setup.settings().simulated_evidence(directory.clone(), [Capture::definition()]).simulated_service("recorder", Capturing);
    });
    sim.advance_ms(1);
    sim.reboot().expect("the recorded facts replay");
    sim.advance_ms(1);
    sim
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
    assert!(matches!(sim.settle(deadline), Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::TimedOut))));
}

#[test]
fn scope_clock_scripted() {
    let measured = scripted_wait(11);

    // 751 rather than 750 because the work clock's timer rounds a deadline up
    // to its next one-millisecond tick, so a 750 ms wait wakes on the 751st.
    // The number is the same one every run, which is the property here: the
    // measurement is scripted rather than observed.
    assert_eq!(measured, Duration::from_millis(751), "the scope's clock is the one the script advances");
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

#[test]
fn sidecar_fault_scripted() {
    let script = |seed: u64| {
        let policy = harmos_runtime::ServicePolicy {
            restart: harmos_runtime::operations::RestartPolicy::OnFailure {
                credit: 3,
                sustained_health: Duration::from_millis(250),
                backoff: harmos_runtime::operations::Backoff::Exponential {
                    initial: Duration::from_millis(1),
                    maximum: Duration::from_millis(1),
                    jitter: harmos_runtime::operations::Jitter::Deterministic,
                },
            },
            ..harmos_runtime::services::POLICY
        };
        let mut sim = bank(seed, move |setup| {
            setup
                .settings()
                .operation_settings(harmos_runtime::operations::Settings {
                    preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                    ..Default::default()
                })
                .simulated_service_with("meter", policy, IdleResident::new());
        });
        sim.fault("meter", "the meter fell over").expect("the attached sidecar receives its scripted exit");
        sim.advance_ms(8);
        sim
    };

    let first = script(11);
    let second = script(11);

    assert!(
        first.service("meter").restarts() >= 1,
        "a scripted sidecar fault spends a credit like any other: {:?}",
        first.service("meter").exits()
    );
    assert!(
        first.service("meter").exits().iter().any(|exit| exit.contains("the meter fell over")),
        "the scripted regime is the one the trace records"
    );
    first.assert_same(&second);
}

#[test]
fn finalized_evidence_inherited() {
    let sim = capturing(11, "lifecycle");

    assert_eq!(
        sim.state().expect("the runtime is running").value.balance(support::HOUSE),
        1,
        "the first boot inherited nothing and the second inherited exactly the \
         one evidence a fact finalized — never the draft no fact names"
    );
}

#[test]
fn evidence_script_deterministic() {
    let first = capturing(11, "determinism-one");
    let second = capturing(11, "determinism-two");

    first.assert_same(&second);
    assert_eq!(first.trace(), second.trace());
}

// The Feature Only Adds: A Spawned Start Stays Send Under Simulation.
#[tokio::test]
async fn start_stays_send() {
    let runtime = tokio::spawn(Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.operations::<SimOperations>(Lanes::Serial);
        setup.settings().register(catalog()).register(streams());
    }))
    .await
    .expect("the spawned start completes")
    .expect("the bank runtime starts");
    runtime.shutdown().await.unwrap();
}
