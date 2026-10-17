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
    #[harmos_runtime::record(id = "sim.observed", version = 1)]
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Observed {
        before: i64,
        amount: i64,
    }
    #[harmos_runtime::transaction(id = "sim.add", version = 1, inverse = Self)]
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
    impl harmos_runtime::state::Invert<State> for Add {
        fn invert(&self, _: &State) -> Self {
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
    impl Application for Model {
        type State = State;
        type Transaction = Changes;
        type Record = Facts;
        type Streams = ();
        type Resources = ();
    }

    #[test]
    fn records_survive_reboot() {
        let make = || {
            Runtime::<Model>::builder(State::default(), ())
                .register(Changes::catalog())
                .register(Facts::catalog())
                .entropy_seed(17)
                .simulate()
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
use harmos_runtime::operations::WireTable;
use harmos_runtime::operations::{Context, Operation, OperationPolicy, RestartPolicy, RetryPolicy};
use harmos_runtime::sidecar::{Notice, Supervised};
use harmos_runtime::simulation::entropy;
use harmos_runtime::{
    Application, Builder, DefinitionId, Identified, Runtime, evidence::EvidenceKind, runtime::Catalog, runtime::Registration,
    services::Fault, simulation::SimulatedRuntime, state::Focus, state::Kind, state::Transaction,
};
use harmos_sidecar::{Attach, PluginNative, Supervisor};
use support::{Bank, Deposit, Ledger, Noted, alice, catalog, pulse, streams};

/* === Constants === */
static DIVERGENCE: AtomicUsize = AtomicUsize::new(0);

/* === Macros === */

/* === Type Aliases === */
type IdleManaged = harmos_runtime::sidecar::Managed<Bank, PluginNative<Idle>, fn(&harmos_runtime::services::Service<Bank>, Notice), Lanes>;

/* === Structs === */
#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "sim.pending",
    version = 1,
    response = (),
    policy = OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_millis(5),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    }
)]
struct Pending {}

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "sim.answering",
    version = 1,
    response = u64,
    policy = OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_millis(50),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    }
)]
struct Answering {
    answer: u64,
}

// The Draw Log Is Shared With The Scenario, So The Request Stays In Process.
#[harmos_runtime::operation(
    id = "sim.drawing",
    version = 1,
    response = u64,
    policy = OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_millis(100),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    }
)]
struct Drawing {
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

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "sim.timingitsownwait",
    version = 1,
    response = u64,
    policy = OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_millis(10000),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    }
)]
struct TimingItsOwnWait {
    wait_ms: u64,
}

struct Capture;

#[derive(Clone)]
struct Capturing;

struct Idle {
    resources: Arc<()>,
}

struct IdleResident {
    managed: IdleManaged,
}

/* === Enums === */
#[harmos_runtime::catalog]
enum Operations {
    Pending(Pending),
    Answering(Answer),
    Drawing(Draw),
    TimingItsOwnWait(TimeWait),
}

#[harmos_runtime::lanes]
enum Lanes {
    #[default]
    Open,
    #[lane(serial)]
    Jobs,
}

/* === Traits === */

/* === Implementations === */
impl Operation for Pending {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = Pending;
    type Resources = ();
    type Error = Infallible;

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Pending, Infallible> {
        Ok(Self {})
    }

    async fn run<'ctx>(
        &mut self, _context: Self::Context<'ctx>, _request: &mut Pending, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Infallible> {
        let result: ::core::result::Result<(), Self::Error> = async move { std::future::pending().await }.await;
        result.map(::core::convert::Into::into)
    }
}

struct Answer;

impl Operation for Answer {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = Answering;
    type Resources = ();
    type Error = Infallible;

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, Infallible> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self, _context: Self::Context<'ctx>, request: &mut Answering, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<u64>, Infallible> {
        let result: ::core::result::Result<u64, Self::Error> = async move { Ok(request.answer) }.await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Message for Drawing {
    fn encode_to_vec(&self) -> Vec<u8> {
        unreachable!("the in-process draw request is never encoded")
    }
    fn encoded_len(&self) -> usize {
        unreachable!("the in-process draw request is never encoded")
    }
    fn decoding_memory(_: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        Err(harmos_runtime::MessageError::borrowed("the in-process draw request is never decoded"))
    }
    fn decode(_: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        Err(harmos_runtime::MessageError::borrowed("the in-process draw request is never decoded"))
    }
}

impl harmos_runtime::MemorySize for Drawing {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl harmos_runtime::schema::Described for Drawing {
    const NAME: &'static str = concat!(module_path!(), "::Drawing");
    const FIELDS: &'static [harmos_runtime::schema::Field] = &[];
    const MARKER: &'static [u8] = &harmos_runtime::schema::declaration::encode::<
        { harmos_runtime::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

struct Draw;

impl Operation for Draw {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = Drawing;
    type Resources = ();
    type Error = Infallible;

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, Infallible> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self, _context: Self::Context<'ctx>, request: &mut Drawing, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<u64>, Infallible> {
        let result: ::core::result::Result<u64, Self::Error> = async move {
            let draw = entropy();
            request.draws.lock().expect("draw lock is healthy").push(draw);
            Ok(draw)
        }
        .await;
        result.map(::core::convert::Into::into)
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

impl Application for Divergent {
    type State = DivergentState;
    type Transaction = DivergentChange;
    type Record = NoRecord;
    type Streams = ();
    type Resources = ();
}

impl Registration<Divergent> for DivergentCatalog {
    fn register(self) -> Catalog<Divergent> {
        fn decode(_id: &DefinitionId, _version: u32, _bytes: &[u8]) -> Result<Option<DivergentChange>, harmos_runtime::state::DecodeError> {
            Ok(None)
        }

        Catalog::Transactions { definitions: vec![(Kind::Transaction, DefinitionId::new("sim.divergent"), 1)], decode, inverses: None }
    }
}

struct TimeWait;

impl Operation for TimeWait {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = TimingItsOwnWait;
    type Resources = ();
    type Error = Infallible;

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, Infallible> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, request: &mut TimingItsOwnWait, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<u64>, Infallible> {
        let result: ::core::result::Result<u64, Self::Error> = async move {
            let started = context.scope().now();
            tokio::time::sleep(Duration::from_millis(request.wait_ms)).await;
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
    type Error = String;

    async fn run(&mut self, mut context: harmos_runtime::services::Service<Bank>) -> Result<(), Fault<String>> {
        let evidence = context.evidence().clone();
        let inherited = evidence.list("run").len() as i64;
        context.state().commit(Deposit { account: support::HOUSE, amount: inherited.max(0) }).await.ok();

        let key = format!("boot-{inherited}.log");
        let draft = evidence.open::<Capture>("run", key, ()).await.map_err(|why| Fault::Environmental(why.to_string()))?;
        draft.write(b"one boot's worth").await.map_err(|why| Fault::Environmental(why.to_string()))?;
        evidence.finalize(&draft).await.map_err(|why| Fault::Environmental(why.to_string()))?;

        // A second draft is deliberately left open: no fact names it, so the
        // next boot must not see it.
        evidence
            .open::<Capture>("run", "unfinished.log", ())
            .await
            .map_err(|why| Fault::Environmental(why.to_string()))?
            .write(b"half")
            .await
            .map_err(|why| Fault::Environmental(why.to_string()))?;

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

impl harmos_runtime::operations::lifecycle::Policy for Idle {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_runtime::plugin(id = "idle", provides = (), requires = ())]
impl harmos_runtime::Plugin for Idle {
    type Context = Supervisor;
    type Config = ();
    type Error = Infallible;
    type Resources = ();
    fn prepare(_: ()) -> Result<Self, Infallible> {
        Ok(Self { resources: Arc::new(()) })
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Infallible> {
        Ok(())
    }
    fn resources(&self) -> Arc<()> {
        self.resources.clone()
    }
    fn bind_operations(&self, _: &mut WireTable<Supervisor>) -> Result<(), harmos_runtime::operations::ServingError> {
        Ok(())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Infallible> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
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
            (|_: &harmos_runtime::services::Service<Bank>, _: Notice| {}) as fn(&harmos_runtime::services::Service<Bank>, Notice),
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
    async fn run(&mut self, context: harmos_runtime::services::Service<Bank>) -> Result<(), Fault<Self::Error>> {
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
fn bank(seed: u64) -> Builder<Bank> {
    Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(streams())
        .register(Lanes::catalog())
        .operations::<Operations, _>("simulation", Lanes::Jobs, Arc::new(()))
        .entropy_seed(seed)
}

async fn resident(mut context: harmos_runtime::services::Service<Bank>) -> Result<(), Fault<&'static str>> {
    context.ready();
    context.stopping().await;
    Ok(())
}

fn scripted(seed: u64) -> SimulatedRuntime<Bank> {
    oracle::scripted(bank(seed))
}

fn scripted_wait(seed: u64) -> Duration {
    let mut sim = bank(seed).simulate().expect("the simulation boots");
    let timed = sim.submit(TimingItsOwnWait { wait_ms: 750 }).unwrap();
    sim.advance_ms(400);
    sim.advance_ms(350);

    Duration::from_nanos(*sim.settle(timed).expect("the scripted attempt answers"))
}

fn entropy_run(seed: u64) -> (SimulatedRuntime<Bank>, Vec<u64>) {
    let draws = Arc::new(Mutex::new(Vec::new()));
    let service_draws = Arc::clone(&draws);
    let mut sim = bank(seed)
        .simulated_service("entropy", move |mut context: harmos_runtime::services::Service<Bank>| {
            let draws = Arc::clone(&service_draws);
            async move {
                draws.lock().expect("draw lock is healthy").push(entropy());
                context.ready();
                context.stopping().await;
                Ok::<(), Fault<&'static str>>(())
            }
        })
        .simulate()
        .expect("the simulation boots");
    let handle = sim.submit(Drawing { draws: Arc::clone(&draws) }).unwrap();
    sim.settle(handle).expect("the entropy job settles");
    let values = draws.lock().expect("draw lock is healthy").clone();
    (sim, values)
}

fn vault(name: &str) -> vault::Directory {
    let directory = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("sim-evidence").join(name);
    std::fs::remove_dir_all(&directory).ok();

    vault::Directory::at(directory)
}

fn capturing(seed: u64, name: &str) -> SimulatedRuntime<Bank> {
    let mut sim = bank(seed)
        .simulated_evidence(vault(name), [Capture::definition()])
        .simulated_service("recorder", Capturing)
        .simulate()
        .expect("the simulation boots");
    sim.advance_ms(1);
    sim.reboot().expect("the recorded facts replay");
    sim.advance_ms(1);
    sim
}

/* === Tests === */

#[test]
fn attached_fold_refused() {
    let (mirror, _) = support::Mirror::new();
    let refusal = bank(42).attach(mirror).simulate().err().expect("a live fold makes this composition incompatible with simulation");

    assert_eq!(refusal.to_string(), "this build cannot be simulated after attaching an attached fold");
}

#[test]
fn seed_replays_identically() {
    let first = scripted(42);
    let second = scripted(42);

    first.assert_same(&second);
    assert_eq!(first.trace(), second.trace());
}

#[test]
fn backoff_delays_restart() {
    let mut sim = bank(7)
        .simulated_service_with(
            "adapter",
            harmos_runtime::operations::OperationPolicy {
                restart: harmos_runtime::operations::RestartPolicy::OnTransient {
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
        )
        .simulate()
        .expect("the simulation boots");

    sim.fault("adapter", Fault::Environmental("disconnected")).expect("the running service receives its exit");
    assert_eq!(sim.service("adapter").restarts(), 0);
    sim.advance_ms(4);
    assert_eq!(sim.service("adapter").restarts(), 0);
    sim.advance_ms(4);
    assert_eq!(sim.service("adapter").restarts(), 1);
    assert_eq!(sim.service("adapter").exits(), ["environmental:disconnected"]);
}

#[test]
fn attempt_deadline_expires() {
    let mut sim = bank(7).simulate().expect("the simulation boots");
    let deadline = sim.submit(Pending {}).unwrap();
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
    let mut sim = Runtime::<Divergent>::builder(DivergentState::default(), ())
        .register(DivergentCatalog)
        .entropy_seed(9)
        .simulate()
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
    let mut sim = bank(101).simulated_service("adapter", resident).simulate().expect("the simulation boots");
    sim.commit(alice(), Deposit { account: support::HOUSE, amount: 3 }).expect("commit lands");
    sim.record(alice(), Noted("full script".to_owned())).expect("record lands");
    sim.publish(pulse(8)).expect("publish lands");
    let job = sim.submit(Answering { answer: 13 }).unwrap();
    assert_eq!(*sim.settle(job).expect("operation settles"), 13);
    sim.advance_ms(1);
    sim.fault("adapter", Fault::Environmental("reset")).expect("fault is delivered");
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
        let policy = harmos_runtime::operations::OperationPolicy {
            restart: harmos_runtime::operations::RestartPolicy::OnTransient {
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
        let mut sim = bank(seed)
            .operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                ..Default::default()
            })
            .simulated_service_with("meter", policy, IdleResident::new())
            .simulate()
            .expect("the simulation boots a sidecar with no process behind it");
        sim.fault("meter", Fault::Environmental("the meter fell over")).expect("the attached sidecar receives its scripted exit");
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
    let runtime = tokio::spawn(bank(7).runtime()).await.expect("the spawned start completes").expect("the bank runtime starts");
    runtime.stop().await.unwrap();
}
