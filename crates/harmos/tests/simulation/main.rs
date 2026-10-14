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
#[path = "../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

mod inverses;
#[path = "../support/mod.rs"]
mod support;
mod atomic_records {
    use super::*;
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct State(i64);
    #[harmos::record(id = "sim.observed", version = 1)]
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Observed {
        before: i64,
        amount: i64,
    }
    #[harmos::transaction(id = "sim.add", version = 1, inverse = Self)]
    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct Add(i64);
    impl Transaction<State> for Add {
        type Error = Infallible;
        fn check(&self, _: &State) -> Result<(), Self::Error> {
            Ok(())
        }
        fn records(&self, state: &State, out: &mut harmos::state::Records) {
            out.push(Observed {
                before: state.0,
                amount: self.0,
            })
            .unwrap();
        }
        fn apply(&self, state: &mut State) {
            state.0 += self.0;
        }
    }
    impl harmos::state::Invert<State> for Add {
        fn invert(&self, _: &State) -> Self {
            Self(-self.0)
        }
    }
    #[harmos::transactions(state = State, error = Infallible)]
    #[derive(Clone, Debug)]
    enum Changes {
        Add(Add),
    }
    #[harmos::records]
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
    fn contract_scenario() {
        {
            super::scenario_runtime::check("watch_share_order", || {
                let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
                scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

        use futures_core::Stream;
        use harmos::{Error, state::FactSource, state::Position, state::Recorded, state::SystemFact};
        let runtime = Runtime::<Model>::builder(State::default(), ())
            .register(Changes::catalog())
            .register(Facts::catalog())
            .runtime()
            .await
            .unwrap();
        let receipt = runtime.state.commit(Add(7)).await.unwrap();
        let captured = runtime.state.state().read(|state| state.0).await.unwrap();
        let fact = SystemFact {
            source: FactSource::Sidecar("test-plugin".into()),
            name: "sample".into(),
            version: 4,
            payload: vec![1, 2],
        };
        runtime.records.publish_system(fact.clone()).await.unwrap();
        let entries = runtime
            .state
            .query_through::<Recorded<Model>>(Position::ORIGIN, captured.position)
            .await
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(captured.value, 7);
        assert_eq!(captured.position, receipt.frontier);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.position)
                .collect::<Vec<_>>(),
            [Position(1), Position(2)]
        );
        assert!(matches!(entries[0].value, Recorded::Transaction(_)));
        assert!(matches!(entries[1].value, Recorded::Record(_)));
        let interior = runtime
            .state
            .query_through::<Recorded<Model>>(Position(1), Position(2))
            .await
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(interior.len(), 1);
        assert!(matches!(
            runtime
                .state
                .query_through::<Recorded<Model>>(Position::ORIGIN, Position(4))
                .await
                .next(),
            Some(Err(Error::FrontierAhead))
        ));
        {
            let mut watching =
                std::pin::pin!(runtime.state.watch::<Recorded<Model>>(Position::ORIGIN));
            for expected in 1..=3 {
                let entry = std::future::poll_fn(|cx| watching.as_mut().poll_next(cx))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(entry.position, Position(expected));
                if expected == 3 {
                    assert!(matches!(entry.value, Recorded::System(value) if value == fact));
                }
            }
        }
        let all = runtime
            .state
            .query::<Recorded<Model>>(Position::ORIGIN)
            .await
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(all.len(), 3);
        runtime.stop().await.unwrap();

});
            });
        }
        {
            super::scenario_runtime::check("query_refuses_evicted", || {
                let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
                scenario_executor
                    .enable_all()
                    .build()
                    .expect("scenario runtime is valid")
                    .block_on(async {
                        use harmos::{Error, state::Position, state::Recorded};
                        let runtime = Runtime::<Model>::builder(State::default(), ())
                            .register(Changes::catalog())
                            .register(Facts::catalog())
                            .tail(1)
                            .runtime()
                            .await
                            .unwrap();
                        let first = runtime.state.commit(Add(1)).await.unwrap();
                        runtime.state.commit(Add(2)).await.unwrap();
                        let entries = runtime
                            .state
                            .query_through::<Recorded<Model>>(Position::ORIGIN, first.frontier)
                            .await
                            .collect::<Vec<_>>();
                        assert_eq!(entries.len(), 1);
                        assert!(matches!(entries[0], Err(Error::BeyondTail)));
                        runtime.stop().await.unwrap();
                    });
            });
        }
        {
            super::scenario_runtime::check("records_survive_reboot", || {
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
                    assert_eq!(
                        (receipt.position, receipt.frontier),
                        (harmos::state::Position(1), harmos::state::Position(2))
                    );
                    sim.assert_replay_equivalent();
                    sim.reboot().unwrap();
                    assert_eq!(sim.state().unwrap().value, State(7));
                    sim.undo(alice(), Focus::all()).unwrap();
                    sim.assert_replay_equivalent();
                    assert_eq!(sim.state().unwrap().value, State(0));
                    assert_eq!(sim.stop().unwrap(), harmos::state::Position(4));
                }
                first.assert_same(&second);
            });
        }
    }
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::convert::Infallible;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
// Third Party Imports

// Local Imports
use harmos::operations::WireTable;
use harmos::operations::{
    Context, Operation, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy,
};
use harmos::sidecar::{Notice, Supervised};
use harmos::simulation::entropy;
use harmos::{
    Application, Builder, DefinitionId, Identified, Runtime, evidence::EvidenceKind,
    runtime::Catalog, runtime::Registration, services::Fault, simulation::SimulatedRuntime,
    state::Focus, state::Kind, state::Transaction,
};
use harmos_sidecar::{Attach, PluginNative, Supervisor};
use support::{Bank, DEPOSIT, Deposit, Ledger, NOTED, Noted, alice, catalog, pulse, streams};

/* === Constants === */
static DIVERGENCE: AtomicUsize = AtomicUsize::new(0);

/* === Macros === */

/* === Type Aliases === */
type IdleManaged = harmos::sidecar::Managed<
    Bank,
    PluginNative<Idle>,
    fn(&harmos::services::Service<Bank>, Notice),
    Lanes,
>;

/* === Structs === */
struct Pending;

struct Answering {
    answer: usize,
}

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

struct TimingItsOwnWait {
    wait_ms: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Sheet {
    title: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
struct Rename(String);

struct Editor;

#[derive(Clone)]
struct SheetCatalog;

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
#[harmos::catalog]
enum Operations {
    Pending(Pending),
    Answering(Answer),
    Drawing(Draw),
    TimingItsOwnWait(TimeWait),
}

#[harmos::lanes]
enum Lanes {
    #[default]
    Open,
    #[lane(serial)]
    Jobs,
}

/* === Traits === */

/* === Implementations === */
impl Identified for Deposit {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new(DEPOSIT.1), DEPOSIT.2)
    }
}

impl Identified for Noted {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new(NOTED.1), NOTED.2)
    }
}

impl OperationDeclaration for Pending {
    type Request = Self;
    type Response = ();
    const ID: &'static str = "sim.pending";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_millis(5),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
}

impl Operation for Pending {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = Pending;
    type Resources = ();
    type Error = Infallible;

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Pending, Infallible> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        _context: Self::Context<'ctx>,
        _request: &mut Pending,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<()>, Infallible> {
        let result: ::core::result::Result<(), Self::Error> =
            async move { std::future::pending().await }.await;
        result.map(::core::convert::Into::into)
    }
}

impl OperationDeclaration for Answering {
    type Request = Self;
    type Response = usize;
    const ID: &'static str = "sim.answering";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_millis(50),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
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
        &mut self,
        _context: Self::Context<'ctx>,
        request: &mut Answering,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<usize>, Infallible> {
        let result: ::core::result::Result<usize, Self::Error> =
            async move { Ok(request.answer) }.await;
        result.map(::core::convert::Into::into)
    }
}

impl OperationDeclaration for Drawing {
    type Request = Self;
    type Response = u64;
    const ID: &'static str = "sim.drawing";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_millis(100),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
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
        &mut self,
        _context: Self::Context<'ctx>,
        request: &mut Drawing,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<u64>, Infallible> {
        let result: ::core::result::Result<u64, Self::Error> = async move {
            let draw = entropy();
            request
                .draws
                .lock()
                .expect("draw lock is healthy")
                .push(draw);
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
        fn decode(
            _id: &DefinitionId,
            _version: u32,
            _bytes: &[u8],
        ) -> Result<Option<DivergentChange>, harmos::state::DecodeError> {
            Ok(None)
        }

        Catalog::Transactions {
            definitions: vec![(Kind::Transaction, DefinitionId::new("sim.divergent"), 1)],
            decode,
            inverses: None,
        }
    }
}

impl OperationDeclaration for TimingItsOwnWait {
    type Request = Self;
    type Response = Duration;
    const ID: &'static str = "sim.timingitsownwait";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_millis(10000),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
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
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut TimingItsOwnWait,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Duration>, Infallible> {
        let result: ::core::result::Result<Duration, Self::Error> = async move {
            let started = context.scope().now();
            tokio::time::sleep(Duration::from_millis(request.wait_ms)).await;
            Ok(context.scope().now() - started)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Transaction<Sheet> for Rename {
    type Error = Infallible;

    fn check(&self, _sheet: &Sheet) -> Result<(), Self::Error> {
        Ok(())
    }

    fn apply(&self, sheet: &mut Sheet) {
        sheet.title.clone_from(&self.0);
    }
}

impl Identified for Rename {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("sheet.rename"), 1)
    }
}

impl Application for Editor {
    type State = Sheet;
    type Transaction = Rename;
    type Record = ();
    type Streams = ();
    type Resources = ();
}

impl Registration<Editor> for SheetCatalog {
    fn register(self) -> Catalog<Editor> {
        fn decode(
            _id: &DefinitionId,
            _version: u32,
            _bytes: &[u8],
        ) -> Result<Option<Rename>, harmos::state::DecodeError> {
            Ok(None)
        }

        Catalog::Transactions {
            definitions: vec![(Kind::Transaction, DefinitionId::new("sheet.rename"), 1)],
            decode,
            inverses: Some(|_change, before| Some(Rename(before.title.clone()))),
        }
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

impl harmos::services::Resident<Bank> for Capturing {
    type Error = String;

    async fn run(
        &mut self,
        mut context: harmos::services::Service<Bank>,
    ) -> Result<(), Fault<String>> {
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
        let draft = evidence
            .open::<Capture>("run", key, ())
            .await
            .map_err(|why| Fault::Environmental(why.to_string()))?;
        draft
            .write(b"one boot's worth")
            .await
            .map_err(|why| Fault::Environmental(why.to_string()))?;
        evidence
            .finalize(&draft)
            .await
            .map_err(|why| Fault::Environmental(why.to_string()))?;

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

impl harmos::state::CatalogCodec for NoRecord {
    fn identify(&self) -> (harmos::DefinitionId, u32) {
        (DefinitionId::new("sim.none"), 1)
    }
    fn encode<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_unit()
    }
}

impl harmos::state::RecordCatalog for NoRecord {
    fn restore(
        _: &DefinitionId,
        _: u32,
        _: &serde_json::Value,
    ) -> Result<Option<Self>, serde_json::Error> {
        Ok(None)
    }
}

impl harmos::state::TransactionCatalog<DivergentState> for DivergentChange {}

impl harmos::state::TransactionCatalog<Sheet> for Rename {}

impl harmos::operations::lifecycle::Policy for Idle {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos::plugin(id = "idle", provides = (), requires = ())]
impl harmos::Plugin for Idle {
    type Context = Supervisor;
    type Config = ();
    type Error = Infallible;
    type Resources = ();
    fn prepare(_: ()) -> Result<Self, Infallible> {
        Ok(Self {
            resources: Arc::new(()),
        })
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Infallible> {
        Ok(())
    }
    fn resources(&self) -> Arc<()> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        _: &mut WireTable<Supervisor>,
    ) -> Result<(), harmos::operations::ServingError> {
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
        let source =
            PluginNative::<Idle>::new(capacity).expect("the idle SDK declaration is valid");
        let definition = harmos::plugin::PluginDefinition {
            id: source.owner().into(),
            version: "1.0.0".parse().unwrap(),
            artifact: harmos::plugin::PluginArtifact::LocalSidecar {
                revision: "simulation".into(),
            },
            configuration: serde_json::to_vec(&()).unwrap(),
            requires: vec![],
            contributions: harmos::plugin::PluginContributions {
                operations: source.operations().unwrap(),
                wire_contracts: source.operation_contracts().unwrap().unwrap(),
                required_contracts: source.required_contracts().unwrap(),
                lifecycle: source.sdk_lifecycle().unwrap(),
                streams: source.publications(),
                observers: source.observers(),
                observer_control: source.observer_control().unwrap(),
                listeners: source.listener_declarations().unwrap().unwrap_or_default(),
                ..Default::default()
            },
        };
        let resident = Supervised::new(
            source,
            (|_: &harmos::services::Service<Bank>, _: Notice| {})
                as fn(&harmos::services::Service<Bank>, Notice),
        );
        let managed = harmos::sidecar::Managed::new(
            resident,
            definition,
            Lanes::Open,
            capacity,
            NonZeroUsize::new(4096).unwrap(),
        )
        .expect("the frozen idle graph is valid");
        Self { managed }
    }
}

impl Clone for IdleResident {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl harmos::services::Resident<Bank> for IdleResident {
    type Error = harmos::sidecar::ManagedError;
    async fn run(
        &mut self,
        context: harmos::services::Service<Bank>,
    ) -> Result<(), Fault<Self::Error>> {
        harmos::services::Resident::run(&mut self.managed, context).await
    }
    async fn stop(&mut self) -> Result<(), Self::Error> {
        harmos::services::Resident::stop(&mut self.managed).await
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        harmos::services::Resident::cleanup(&mut self.managed).await
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

async fn resident(mut context: harmos::services::Service<Bank>) -> Result<(), Fault<&'static str>> {
    context.ready();
    context.stopping().await;
    Ok(())
}

fn scripted(seed: u64) -> SimulatedRuntime<Bank> {
    let mut sim = bank(seed)
        .simulated_service("adapter", resident)
        .simulate()
        .expect("the simulation boots");
    sim.commit(
        alice(),
        Deposit {
            account: support::HOUSE,
            amount: 7,
        },
    )
    .expect("the scripted commit lands");
    sim.record(alice(), Noted("seeded".to_owned()))
        .expect("the scripted record lands");
    sim.publish(pulse(4)).expect("the row catalog is frozen");
    sim.fault("adapter", Fault::Environmental("bus reset"))
        .expect("the running service receives its exit");
    sim.advance_ms(12);
    sim
}

fn scripted_wait(seed: u64) -> Duration {
    let mut sim = bank(seed).simulate().expect("the simulation boots");
    let timed = sim
        .submit_as::<TimingItsOwnWait>(TimingItsOwnWait { wait_ms: 750 })
        .unwrap();
    sim.advance_ms(400);
    sim.advance_ms(350);

    *sim.settle(timed).expect("the scripted attempt answers")
}

fn entropy_run(seed: u64) -> (SimulatedRuntime<Bank>, Vec<u64>) {
    let draws = Arc::new(Mutex::new(Vec::new()));
    let service_draws = Arc::clone(&draws);
    let mut sim = bank(seed)
        .simulated_service(
            "entropy",
            move |mut context: harmos::services::Service<Bank>| {
                let draws = Arc::clone(&service_draws);
                async move {
                    draws.lock().expect("draw lock is healthy").push(entropy());
                    context.ready();
                    context.stopping().await;
                    Ok::<(), Fault<&'static str>>(())
                }
            },
        )
        .simulate()
        .expect("the simulation boots");
    let handle = sim
        .submit_as::<Drawing>(Drawing {
            draws: Arc::clone(&draws),
        })
        .unwrap();
    sim.settle(handle).expect("the entropy job settles");
    let values = draws.lock().expect("draw lock is healthy").clone();
    (sim, values)
}

fn vault(name: &str) -> jsonl_adapter::EvidenceDirectory {
    let directory = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("sim-evidence")
        .join(name);
    std::fs::remove_dir_all(&directory).ok();

    jsonl_adapter::EvidenceDirectory::at(directory).expect("a fresh directory")
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
fn contract_scenario() {
    {
        scenario_runtime::check("simulation_refuses_drop", || {
            let (mirror, _) = support::Mirror::new();
            let refusal = bank(42)
                .attach(mirror)
                .simulate()
                .err()
                .expect("a live fold makes this composition incompatible with simulation");

            assert_eq!(
                refusal.to_string(),
                "this build cannot be simulated after attaching an attached fold"
            );
        });
    }
    {
        scenario_runtime::check("sim_ac001_double", || {
            let first = scripted(42);
            let second = scripted(42);

            first.assert_same(&second);
            assert_eq!(first.trace(), second.trace());
        });
    }
    {
        scenario_runtime::check("sim_ac002_backoff", || {
            let wall = Instant::now();
            let mut sim = bank(7)
                .simulated_service_with(
                    "adapter",
                    harmos::operations::OperationPolicy {
                        restart: harmos::operations::RestartPolicy::OnTransient {
                            credit: 3,
                            sustained_health: Duration::from_millis(250),
                            backoff: harmos::operations::Backoff::Exponential {
                                initial: Duration::from_millis(5),
                                maximum: Duration::from_millis(5),
                                jitter: harmos::operations::Jitter::Deterministic,
                            },
                        },
                        ..harmos::services::POLICY
                    },
                    resident,
                )
                .simulate()
                .expect("the simulation boots");

            sim.fault("adapter", Fault::Environmental("disconnected"))
                .expect("the running service receives its exit");
            assert_eq!(sim.service("adapter").restarts(), 0);
            sim.advance_ms(4);
            assert_eq!(sim.service("adapter").restarts(), 0);
            sim.advance_ms(4);
            assert_eq!(sim.service("adapter").restarts(), 1);
            assert_eq!(
                sim.service("adapter").exits(),
                ["environmental:disconnected"]
            );
            let deadline = sim.submit_as::<Pending>(Pending).unwrap();
            sim.advance_ms(5);
            assert!(matches!(
                sim.settle(deadline),
                Err(harmos::Error::OperationRefused(harmos::Refusal::TimedOut))
            ));
            assert!(
                wall.elapsed() < Duration::from_millis(100),
                "virtual backoff must not sleep on wall time"
            );
        });
    }
    {
        scenario_runtime::check("measures_time_script", || {
            let wall = Instant::now();

            let measured = scripted_wait(11);

            // 751 rather than 750 because the work clock's timer rounds a deadline up
            // to its next one-millisecond tick, so a 750 ms wait wakes on the 751st.
            // The number is the same one every run, which is the property here: the
            // measurement is scripted rather than observed.
            assert_eq!(
                measured,
                Duration::from_millis(751),
                "the scope's clock is the one the script advances"
            );
            assert_eq!(
                measured,
                scripted_wait(11),
                "and it measures the same duration every time the same script is run"
            );
            assert!(
                wall.elapsed() < Duration::from_millis(100),
                "the attempt reported {measured:?} of scripted time in {:?} of wall time, \
         which is exactly the gap `Instant::now()` inside an attempt would have \
         reported instead",
                wall.elapsed()
            );
        });
    }
    {
        scenario_runtime::check("sim_ac003_negative", || {
            DIVERGENCE.store(0, Ordering::SeqCst);
            let mut sim = Runtime::<Divergent>::builder(DivergentState::default(), ())
                .register(DivergentCatalog)
                .entropy_seed(9)
                .simulate()
                .expect("the deliberately divergent simulation boots");
            sim.commit(harmos::state::PrincipalId::new("tester"), DivergentChange)
                .expect("the first application succeeds");

            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                sim.assert_replay_equivalent();
            }));
            assert!(caught.is_err(), "the recovery oracle must catch divergence");
        });
    }
    {
        scenario_runtime::check("reboot_restores_state", || {
            let mut sim = bank(11).simulate().expect("the simulation boots");
            sim.commit(
                alice(),
                Deposit {
                    account: support::HOUSE,
                    amount: 5,
                },
            )
            .expect("the deposit lands");
            assert_eq!(sim.stop().unwrap().0, 1);

            sim.reboot().expect("the in-memory copy recovers");
            assert_eq!(
                sim.state()
                    .expect("state is readable")
                    .value
                    .balance(support::HOUSE),
                5
            );
            sim.undo(alice(), Focus::all())
                .expect("recovery rebuilt the personal undo stack");
            assert_eq!(
                sim.state()
                    .expect("state is readable")
                    .value
                    .balance(support::HOUSE),
                0
            );
        });
    }
    {
        scenario_runtime::check("sim_ac006_entropy", || {
            let (first, first_draws) = entropy_run(99);
            let (second, second_draws) = entropy_run(99);

            assert_eq!(first_draws.len(), 2);
            assert_eq!(first_draws, second_draws);
            first.assert_same(&second);
        });
    }
    {
        scenario_runtime::check("sim_ac007_script", || {
            let mut sim = bank(101)
                .simulated_service("adapter", resident)
                .simulate()
                .expect("the simulation boots");
            sim.commit(
                alice(),
                Deposit {
                    account: support::HOUSE,
                    amount: 3,
                },
            )
            .expect("commit lands");
            sim.record(alice(), Noted("full script".to_owned()))
                .expect("record lands");
            sim.publish(pulse(8)).expect("publish lands");
            let job = sim
                .submit_as::<Answering>(Answering { answer: 13 })
                .unwrap();
            assert_eq!(*sim.settle(job).expect("operation settles"), 13);
            sim.advance_ms(1);
            sim.fault("adapter", Fault::Environmental("reset"))
                .expect("fault is delivered");
            sim.advance_ms(20);
            assert_eq!(sim.stop().unwrap().0, 2);
            sim.reboot().expect("the in-memory source reboots");
            sim.assert_replay_equivalent();

            let state = sim.state().expect("state is readable");
            assert_eq!(state.position.0, 2);
            assert_eq!(state.value.balance(support::HOUSE), 3);
            assert!(sim.service("adapter").restarts() >= 2);

            // The one remaining verb is the round-trip law, and the ledger above
            // cannot answer it: its `apply` counts applications and leaves a zeroed
            // account behind, so a deposit and its withdrawal land on a state that is
            // semantically the old one and structurally not. The editor below is what
            // chapter 7 teaches — a rename that restores exactly the name it replaced.
            let mut editor = Runtime::<Editor>::builder(Sheet::default(), ())
                .register(SheetCatalog)
                .entropy_seed(101)
                .simulate()
                .expect("the editor simulation boots");
            for title in ["draft", "final"] {
                editor
                    .commit(alice(), Rename(title.to_owned()))
                    .expect("commit lands");
            }
            editor
                .undo(alice(), harmos::state::Focus::all())
                .expect("the rename comes back");

            assert_eq!(
                editor.state().expect("state is readable").value.title,
                "draft",
                "the undo committed the name the second rename replaced"
            );
            editor.assert_inverses_restore();
        });
    }
    {
        scenario_runtime::check("sidecar_fault_scripted", || {
            let script = |seed: u64| {
                let policy = harmos::operations::OperationPolicy {
                    restart: harmos::operations::RestartPolicy::OnTransient {
                        credit: 3,
                        sustained_health: Duration::from_millis(250),
                        backoff: harmos::operations::Backoff::Exponential {
                            initial: Duration::from_millis(1),
                            maximum: Duration::from_millis(1),
                            jitter: harmos::operations::Jitter::Deterministic,
                        },
                    },
                    ..harmos::services::POLICY
                };
                let mut sim = bank(seed)
                    .operation_settings(harmos::operations::Settings {
                        preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                        ..Default::default()
                    })
                    .simulated_service_with("meter", policy, IdleResident::new())
                    .simulate()
                    .expect("the simulation boots a sidecar with no process behind it");
                sim.fault("meter", Fault::Environmental("the meter fell over"))
                    .expect("the attached sidecar receives its scripted exit");
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
                first
                    .service("meter")
                    .exits()
                    .iter()
                    .any(|exit| exit.contains("the meter fell over")),
                "the scripted regime is the one the trace records"
            );
            first.assert_same(&second);
        });
    }
    {
        scenario_runtime::check("lifecycle_survives_reboot", || {
            let sim = capturing(11, "lifecycle");

            assert_eq!(
                sim.state()
                    .expect("the runtime is running")
                    .value
                    .balance(support::HOUSE),
                1,
                "the first boot inherited nothing and the second inherited exactly the \
         one evidence a fact finalized — never the draft no fact names"
            );
        });
    }
    {
        scenario_runtime::check("scripted_draft_lifecycle", || {
            let first = capturing(11, "determinism-one");
            let second = capturing(11, "determinism-two");

            first.assert_same(&second);
            assert_eq!(first.trace(), second.trace());
        });
    }
}

// WORK-AC026, on the harness the criterion exists for: an attempt inside a
// seeded simulation measures the time the script advanced, not the time the
// process spent, and the two advances that made it up add.
