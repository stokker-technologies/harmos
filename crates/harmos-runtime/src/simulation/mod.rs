/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod clock;
mod entropy;
pub(crate) mod executor;

/* === Re-Exports === */

pub use entropy::entropy;

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
// Third Party Imports
use tokio::sync::broadcast;
// Local Imports
use crate::{
    Application, Builder, DefinitionId, Error, Identified, Runtime, evidence::Vault, runtime::Catalog, runtime::Incompatible,
    runtime::Registration, services::Resident, services::ServiceContext, state::At, state::Entry, state::Envelope, state::Focus,
    state::Origin, state::Position, state::PrincipalId, state::Receipt, state::Recorded, state::Transaction, storage::Cursor,
    storage::JournalConsumer, storage::Payload, storage::Recovery, storage::Source, streams::Stream,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Inverting<A> = fn(&<A as Application>::Transaction, &<A as Application>::State) -> Option<<A as Application>::Transaction>;
type BuildStep<A> = Arc<dyn Fn(Builder<A>) -> Builder<A> + Send + Sync>;
type UndoResult<A> = Result<Receipt, Error<<<A as Application>::Transaction as Transaction<<A as Application>::State>>::Error>>;

/* === Structs === */

pub struct SimulatedRuntime<A>
where
    A: Application,
    A::State: Clone + fmt::Debug + PartialEq,
    A::Transaction: Clone + fmt::Debug,
    A::Record: Clone + fmt::Debug,
{
    origin: Origin<A::State>,
    resources: Arc<A::Resources>,
    executor: executor::Executor,
    assembly: Assembly<A>,
    memory: Memory<A>,
    runtime: Option<Runtime<A>>,
    services: BTreeMap<String, Arc<ServiceControl>>,
    cursors: Vec<Cursor>,
    stopped: Position,
    inverses: Option<Inverting<A>>,
}

struct Divergence<S> {
    at: Position,
    before: S,
    restored: S,
}

pub(crate) struct Configuration<A: Application> {
    assembly: Assembly<A>,
    services: BTreeMap<String, Arc<ServiceControl>>,
    inverses: Option<Inverting<A>>,
    entropy_seed: u64,
}

#[derive(Clone)]
pub struct ServiceProbe {
    control: Arc<ServiceControl>,
}

struct Assembly<A: Application> {
    steps: Vec<BuildStep<A>>,
}

struct ServiceControl {
    faults: broadcast::Sender<String>,
    attempts: AtomicUsize,
    exits: Mutex<Vec<String>>,
}

#[derive(Clone)]
struct ScriptedResident<R> {
    control: Arc<ServiceControl>,
    resident: R,
}

#[derive(Clone)]
struct Stamp {
    id: DefinitionId,
    version: u32,
    elapsed_ms: u64,
}

struct Memory<A: Application> {
    shared: Arc<MemoryState<A>>,
}

struct MemoryState<A: Application> {
    entries: Mutex<Vec<Arc<Entry<Recorded<A>>>>>,
    identities: Mutex<BTreeMap<Position, Stamp>>,
}

struct MemorySource<A: Application> {
    shared: Arc<MemoryState<A>>,
    resume: Position,
    strict: bool,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum SimError {
    #[error("the simulated runtime is not running")]
    NotRunning,

    #[error("service `{0}` is not declared in this simulation")]
    UnknownService(String),

    #[error("service `{0}` has no running attempt to receive the fault")]
    ServiceStopped(String),
}

/* === Traits === */

/* === Implementations === */

impl<S: fmt::Debug> fmt::Display for Divergence<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the inverse of the change at {:?} did not restore the state it changed\n  \
             before:   {:?}\n  restored: {:?}",
            self.at, self.before, self.restored
        )
    }
}

impl ServiceProbe {
    pub fn restarts(&self) -> usize {
        self.control.attempts.load(Ordering::Acquire).saturating_sub(1)
    }

    pub fn exits(&self) -> Vec<String> {
        self.control.exits.lock().expect("simulation service trace lock is healthy").clone()
    }
}

impl<A: Application> Configuration<A> {
    pub(crate) fn new() -> Self {
        Self {
            assembly: Assembly::new(),
            services: BTreeMap::new(),
            inverses: None,
            entropy_seed: 0,
        }
    }

    pub(crate) fn register<C>(&mut self, catalog: C)
    where
        C: Registration<A> + 'static,
    {
        if let Catalog::Transactions { inverses: Some(inverses), .. } = catalog.clone().register() {
            self.inverses = Some(inverses);
        }
        let catalog = std::sync::Mutex::new(catalog);
        self.assembly.push(move |builder| builder.register(catalog.lock().unwrap().clone()));
    }

    pub(crate) fn operation_middleware(&mut self, middleware: Arc<dyn crate::operations::middleware::Middleware>) {
        self.assembly.push(move |builder| builder.operation_middleware(middleware.clone()));
    }

    pub(crate) fn operation_settings(&mut self, settings: crate::operations::Settings) {
        self.assembly.push(move |builder| builder.operation_settings(settings));
    }

    pub(crate) fn operations(&mut self, binding: crate::operations::CatalogBinding) {
        self.assembly.push(move |builder| builder.bind_operations(binding.clone()));
    }

    pub(crate) fn initialize<F, Fut, E>(&mut self, timeout: std::time::Duration, initialize: F)
    where
        F: Fn(crate::runtime::Handle<A>) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = std::result::Result<(), E>> + Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        self.assembly.push(move |builder| builder.initialize(timeout, initialize.clone()));
    }

    pub(crate) fn evidence(&mut self, vault: impl Vault + Clone, kinds: impl IntoIterator<Item = (DefinitionId, u32)>) {
        let kinds: Vec<_> = kinds.into_iter().collect();
        self.assembly.push(move |builder| builder.evidence(vault.clone(), kinds.clone()));
    }

    pub(crate) fn service<R>(&mut self, name: impl Into<String>, resident: R)
    where
        R: Resident<A> + Clone,
    {
        self.service_with(name, crate::services::POLICY, resident);
    }

    pub(crate) fn service_with<R>(&mut self, name: impl Into<String>, policy: crate::ServicePolicy, resident: R)
    where
        R: Resident<A> + Clone,
    {
        let name = name.into();
        let control = Arc::new(ServiceControl::new());
        self.services.insert(name.clone(), Arc::clone(&control));

        let resident = std::sync::Mutex::new(resident);
        self.assembly.push(move |builder| {
            builder.service_with(
                name.clone(),
                policy,
                ScriptedResident {
                    control: Arc::clone(&control),
                    resident: resident.lock().unwrap().clone(),
                },
            )
        });
    }

    pub(crate) fn entropy_seed(&mut self, seed: u64) {
        self.entropy_seed = seed;
    }

    pub(crate) fn simulate(self, origin: Origin<A::State>, resources: Arc<A::Resources>) -> Result<SimulatedRuntime<A>, Incompatible>
    where
        A::State: Clone + fmt::Debug + PartialEq,
        A::Transaction: Clone + fmt::Debug,
        A::Record: Clone + fmt::Debug,
    {
        let mut sim = SimulatedRuntime {
            origin,
            resources,
            executor: executor::Executor::seeded(self.entropy_seed),
            assembly: self.assembly,
            memory: Memory::new(),
            runtime: None,
            services: self.services,
            cursors: Vec::new(),
            stopped: Position::ORIGIN,
            inverses: self.inverses,
        };
        sim.boot()?;
        Ok(sim)
    }
}

impl<A> SimulatedRuntime<A>
where
    A: Application,
    A::State: Clone + fmt::Debug + PartialEq,
    A::Transaction: Clone + fmt::Debug,
    A::Record: Clone + fmt::Debug,
{
    pub fn commit<T>(&mut self, principal: PrincipalId, transaction: T) -> Result<Receipt, Error<T::Error>>
    where
        T: Identified + Transaction<A::State> + Into<A::Transaction>,
        T::Error: From<<A::Transaction as Transaction<A::State>>::Error>,
    {
        let identity = T::definition();
        let journal = self.runtime.as_ref().ok_or(Error::Unavailable)?.state.as_principal(principal);
        let receipt = self.executor.block_on(journal.commit(transaction))?;
        self.memory.identify(receipt.position, identity, self.executor.elapsed_ms());
        self.executor.block_on(journal.wait_until_stored(receipt.frontier));
        self.executor.drive();
        Ok(receipt)
    }

    pub fn record<R>(&mut self, principal: PrincipalId, record: R) -> Result<Receipt, Error>
    where
        R: Identified + Into<A::Record>,
    {
        let identity = R::definition();
        let journal = self.runtime.as_ref().ok_or(Error::Unavailable)?.state.as_principal(principal);
        let receipt = self.executor.block_on(crate::records::RecordManager::new(journal.clone()).publish(record))?;
        self.memory.identify(receipt.position, identity, self.executor.elapsed_ms());
        self.executor.block_on(journal.wait_until_stored(receipt.frontier));
        self.executor.drive();
        Ok(receipt)
    }

    pub fn publish<S: Stream<A>>(&mut self, row: S) -> Result<(), SimError> {
        let runtime = self.runtime.as_ref().ok_or(SimError::NotRunning)?;
        runtime.streams.publish(row);
        let next = Cursor(self.cursors.last().map_or(1, |cursor| cursor.0 + 1));
        self.cursors.push(next);
        self.executor.drive();
        Ok(())
    }

    pub fn submit<O: crate::operations::OperationDeclaration>(
        &mut self, request: O::Request,
    ) -> crate::Result<crate::operations::Running<O>> {
        let runtime = self.runtime.as_ref().ok_or(Error::Closing { domain: "simulated runtime" })?;
        let operations = runtime.operations.clone();
        let handle = self.executor.block_on(async move { operations.submit(request) });
        self.executor.drive();
        handle
    }

    pub fn settle<O: crate::operations::OperationDeclaration>(
        &mut self, invocation: crate::operations::Running<O>,
    ) -> crate::Result<crate::operations::Response<O::Response>> {
        self.executor.block_on(invocation)
    }

    pub fn advance_ms(&mut self, milliseconds: u64) {
        self.executor.advance_ms(milliseconds);
    }

    pub fn fault<E: fmt::Display>(&mut self, name: impl AsRef<str>, fault: E) -> Result<(), SimError> {
        let name = name.as_ref();
        let control = self.services.get(name).ok_or_else(|| SimError::UnknownService(name.to_owned()))?;
        let fault = fault.to_string();
        control.faults.send(fault).map_err(|_| SimError::ServiceStopped(name.to_owned()))?;
        self.executor.drive();
        Ok(())
    }

    pub fn stop(&mut self) -> crate::Result<Position> {
        let Some(runtime) = self.runtime.as_ref() else {
            return Ok(self.stopped);
        };
        self.stopped = self.executor.block_on(runtime.stop())?;
        self.runtime.take();
        Ok(self.stopped)
    }

    pub fn reboot(&mut self) -> Result<(), Incompatible> {
        self.stop().map_err(Incompatible::work)?;
        self.boot()
    }

    pub fn state(&self) -> Result<At<A::State>, SimError> {
        let runtime = self.runtime.as_ref().ok_or(SimError::NotRunning)?;
        Ok(self.executor.block_on(runtime.state.state().read(Clone::clone)).expect("a running simulated journal remains readable"))
    }

    pub fn undo(&mut self, principal: PrincipalId, focus: Focus<A>) -> UndoResult<A> {
        let runtime = self.runtime.as_ref().ok_or(Error::Unavailable)?;
        self.executor.block_on(runtime.state.undo(principal, focus))
    }

    pub fn service(&self, name: impl AsRef<str>) -> ServiceProbe {
        ServiceProbe {
            control: Arc::clone(self.services.get(name.as_ref()).unwrap_or_else(|| panic!("service `{}` is not declared", name.as_ref()))),
        }
    }

    pub fn trace(&self) -> Vec<u8> {
        let mut trace = self.memory.trace();
        trace.push_str(&format!("cursors:{:?}\n", self.cursors));
        for (name, control) in &self.services {
            trace.push_str(&format!(
                "service:{name}:{}:{:?}\n",
                control.attempts.load(Ordering::Acquire),
                control.exits.lock().expect("simulation service trace lock is healthy")
            ));
        }
        trace.push_str(&format!("entropy:{:?}\n", self.executor.entropy_trace()));
        trace.into_bytes()
    }

    pub fn assert_replay_equivalent(&self) {
        let runtime = self.runtime.as_ref().expect("replay equivalence requires a running simulation");
        let through = runtime.state.applied();
        self.executor.block_on(runtime.state.wait_until_stored(through));
        self.executor.drive();
        let live = self.executor.block_on(runtime.state.state().read(Clone::clone)).expect("the live simulated state remains readable");
        let origin = Origin::at(self.origin.value.clone(), self.origin.position);
        let inspector = self
            .executor
            .block_on(Runtime::<A>::inspect(origin, self.memory.source(false)))
            .expect("the simulation's own retained history is recoverable");
        let replayed = self.executor.block_on(inspector.state.read(Clone::clone)).expect("the independently recovered state is readable");

        assert_eq!(live, replayed, "simulated recovery diverged from live state");
    }

    pub fn assert_inverses_restore(&self) {
        if let Err(divergence) = self.inverses_restore() {
            panic!("{divergence}");
        }
    }

    fn inverses_restore(&self) -> Result<(), Divergence<A::State>> {
        let Some(inverses) = self.inverses else {
            // Nothing declared an inverse, so every entry is a barrier and the
            // law has no subject.
            return Ok(());
        };
        let runtime = self.runtime.as_ref().expect("the round-trip oracle requires a running simulation");
        let through = runtime.state.applied();
        self.executor.block_on(runtime.state.wait_until_stored(through));
        self.executor.drive();

        let mut state = self.origin.value.clone();
        for (at, change) in self.memory.applied(self.origin.position) {
            let before = state.clone();
            let inverse = inverses(&change, &before);
            Transaction::apply(&*change, &mut state);

            let Some(inverse) = inverse else {
                continue;
            };
            let mut restored = state.clone();
            Transaction::apply(&inverse, &mut restored);

            if restored != before {
                return Err(Divergence { at, before, restored });
            }
        }

        Ok(())
    }

    pub fn assert_same(&self, other: &Self) {
        assert_eq!(self.trace(), other.trace(), "simulation traces diverged");
    }

    fn boot(&mut self) -> Result<(), Incompatible> {
        let origin = Origin::at(self.origin.value.clone(), self.origin.position);
        // The same resources on every boot, exactly as the same in-memory copy
        // is recovered from: a reboot is a recovery of one runtime, not the
        // assembly of a second one.
        let builder = self.assembly.build(origin, Arc::clone(&self.resources)).tail(usize::MAX).recover(self.memory.source(true));
        let runtime = self.executor.block_on(builder.runtime())?;
        self.runtime = Some(runtime);
        self.executor.drive();
        Ok(())
    }
}

impl<A> Drop for SimulatedRuntime<A>
where
    A: Application,
    A::State: Clone + fmt::Debug + PartialEq,
    A::Transaction: Clone + fmt::Debug,
    A::Record: Clone + fmt::Debug,
{
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            let error = harmos_domain::failure::Report(&error);
            tracing::error!(%error, "simulated runtime shutdown failed during drop");
        }
    }
}

impl<A: Application> Assembly<A> {
    fn new() -> Self {
        Self { steps: Vec::new() }
    }

    fn push(&mut self, step: impl Fn(Builder<A>) -> Builder<A> + Send + Sync + 'static) {
        self.steps.push(Arc::new(step));
    }

    fn build(&self, origin: Origin<A::State>, resources: Arc<A::Resources>) -> Builder<A> {
        self.steps.iter().fold(Runtime::<A>::builder(origin, resources), |builder, step| step(builder))
    }
}

impl ServiceControl {
    fn new() -> Self {
        Self {
            faults: broadcast::channel(32).0,
            attempts: AtomicUsize::new(0),
            exits: Mutex::new(Vec::new()),
        }
    }

    fn exited(&self, exit: String) {
        self.exits.lock().expect("simulation service trace lock is healthy").push(exit);
    }
}

impl<A, R> Resident<A> for ScriptedResident<R>
where
    A: Application,
    R: Resident<A>,
{
    type Error = std::io::Error;

    async fn run(&mut self, context: ServiceContext<A>) -> Result<(), Self::Error> {
        self.control.attempts.fetch_add(1, Ordering::AcqRel);
        let mut faults = self.control.faults.subscribe();
        let answer = tokio::select! {
            answer = self.resident.run(context) => answer.map_err(std::io::Error::other),
            delivered = faults.recv() => Err(std::io::Error::other(delivered.unwrap_or_else(|_| {
                "simulation fault channel closed".to_owned()
            }))),
        };
        self.control.exited(match &answer {
            Ok(()) => "stopped".to_owned(),
            Err(error) => format!("application:{error}"),
        });
        answer
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.resident.stop().await.map_err(std::io::Error::other)
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.resident.cleanup().await.map_err(std::io::Error::other)
    }
}

impl<A: Application> Memory<A> {
    fn new() -> Self {
        Self {
            shared: Arc::new(MemoryState {
                entries: Mutex::new(Vec::new()),
                identities: Mutex::new(BTreeMap::new()),
            }),
        }
    }

    fn source(&self, strict: bool) -> MemorySource<A> {
        MemorySource {
            shared: Arc::clone(&self.shared),
            resume: Position::ORIGIN,
            strict,
        }
    }

    fn applied(&self, anchor: Position) -> Vec<(Position, crate::operations::Response<A::Transaction>)> {
        self.shared
            .entries
            .lock()
            .expect("simulation history lock is healthy")
            .iter()
            .filter(|entry| entry.position > anchor)
            .filter_map(|entry| match &entry.value {
                Recorded::Transaction(change) => Some((entry.position, change.clone())),
                Recorded::Record(_) | Recorded::System(_) => None,
            })
            .collect()
    }

    fn identify(&self, position: Position, (id, version): (DefinitionId, u32), elapsed_ms: u64) {
        self.shared.identities.lock().expect("simulation identity lock is healthy").insert(position, Stamp { id, version, elapsed_ms });
    }
}

impl<A> Memory<A>
where
    A: Application,
    A::Transaction: fmt::Debug,
    A::Record: fmt::Debug,
{
    fn trace(&self) -> String {
        let identities = self.shared.identities.lock().expect("simulation identity lock is healthy");
        self.shared
            .entries
            .lock()
            .expect("simulation history lock is healthy")
            .iter()
            .map(|entry| {
                let stamp = identities.get(&entry.position);
                let value = match &entry.value {
                    Recorded::Transaction(transaction) => format!("transaction:{transaction:?}"),
                    Recorded::Record(record) => format!("record:{record:?}"),
                    Recorded::System(fact) => format!("system:{fact:?}"),
                };
                format!(
                    "entry:{}:{:?}:{:?}:{:?}:{}:{:?}:{}:{:?}:{:?}:{:?}:{value}\n",
                    entry.position.0,
                    entry.value.kind(),
                    stamp.map(|stamp| stamp.id.as_str()),
                    stamp.map(|stamp| stamp.version),
                    stamp.map_or(0, |stamp| stamp.elapsed_ms),
                    entry.metadata.principal.as_ref().map(PrincipalId::as_str),
                    entry.metadata.correlation.0.0,
                    entry.metadata.causation,
                    entry.metadata.request.as_ref().map(crate::state::RequestKey::as_str),
                    entry.metadata.link,
                )
            })
            .collect()
    }
}

impl<A> JournalConsumer<A> for MemorySource<A>
where
    A: Application,
    A::Transaction: Clone,
    A::Record: Clone,
{
    fn resume(&self) -> Position {
        self.resume
    }

    async fn drain(&mut self, entries: Vec<Arc<Entry<Recorded<A>>>>, _applied: Position) -> Position {
        let mut held = self.shared.entries.lock().expect("simulation history lock is healthy");
        for entry in entries {
            self.resume = entry.position;
            held.push(entry);
        }
        self.resume
    }
}

impl<A> Source<A> for MemorySource<A>
where
    A: Application,
    A::Transaction: Clone,
    A::Record: Clone,
{
    async fn load(&mut self) -> Result<Recovery<A>, Incompatible> {
        let identities = self.shared.identities.lock().expect("simulation identity lock is healthy");
        let entries = self
            .shared
            .entries
            .lock()
            .expect("simulation history lock is healthy")
            .iter()
            .map(|entry| stored(entry, &identities, self.strict))
            .collect::<Result<Vec<_>, _>>()?;
        self.resume = entries.last().map_or(Position::ORIGIN, |entry| entry.position);
        Ok(Recovery {
            commits: Vec::new(),
            frontier: self.resume,
            entries,
            encoded: Vec::new(),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn stored<A>(entry: &Entry<Recorded<A>>, identities: &BTreeMap<Position, Stamp>, strict: bool) -> Result<Envelope<Payload<A>>, Incompatible>
where
    A: Application,
    A::Transaction: Clone,
    A::Record: Clone,
{
    let (id, version) = if let Some(prepared) = &entry.prepared {
        (prepared.id.clone(), prepared.version)
    } else {
        match &entry.value {
            Recorded::System(fact) => fact.identify(),
            Recorded::Transaction(_) | Recorded::Record(_) => identities
                .get(&entry.position)
                .map(|stamp| (stamp.id.clone(), stamp.version))
                .or_else(|| (!strict).then(|| (DefinitionId::new("sim.unidentified"), 1)))
                .ok_or_else(|| {
                    Incompatible::unreadable(format!("simulated entry {} bypassed an identified script verb", entry.position))
                })?,
        }
    };
    let value = match &entry.value {
        Recorded::Transaction(transaction) => Recorded::Transaction(transaction.clone()),
        Recorded::Record(record) => Recorded::Record(record.clone()),
        Recorded::System(fact) => Recorded::System(fact.clone()),
    };

    Ok(Envelope {
        position: entry.position,
        kind: entry.value.kind(),
        id: id.clone(),
        version,
        metadata: entry.metadata.clone(),
        payload: Payload {
            prepared: entry.prepared.clone(),
            id,
            version,
            value,
        },
    })
}

/* === Tests === */
