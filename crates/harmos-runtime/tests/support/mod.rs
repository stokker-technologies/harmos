/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![allow(dead_code)]

/* === Modules === */

mod app;
pub mod fixtures;

/* === Re-Exports === */

#[allow(unused_imports, reason = "each integration-test binary compiles only the fixture slice it needs")]
pub use self::{
    app::{booted, runtime, runtime_from, streaming},
    feeds::{Pulse, Reading},
};

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

// Third Party Imports

// Local Imports
use harmos_runtime::{
    Application, DefinitionId, Runtime, runtime::Catalog, runtime::Incompatible, runtime::Registration, state::DecodeError, state::Entry,
    state::Envelope, state::Invertible, state::Kind, state::Position, state::PrincipalId, state::Recorded, state::Transaction,
    storage::JournalConsumer, storage::Payload, storage::Recovery, storage::Source, streams::Definition,
};

/* === Constants === */

pub const SERIAL: harmos_runtime::operations::Lane =
    harmos_runtime::operations::Lane::new("Serial", harmos_runtime::operations::Capacity::serial());
pub const CONCURRENT: harmos_runtime::operations::Lane =
    harmos_runtime::operations::Lane::new("Concurrent", harmos_runtime::operations::Capacity::concurrent(2));
pub const UNBOUNDED: harmos_runtime::operations::Lane =
    harmos_runtime::operations::Lane::new("Unbounded", harmos_runtime::operations::Capacity::unbounded());

pub const HOUSE: Account = Account(0);
pub const VAULT: Account = Account(1);
pub static DEPOSIT: LazyLock<(Kind, &str, u32)> = LazyLock::new(|| (Kind::Transaction, "ledger.deposit", revision::<Deposit>()));
pub static WITHDRAW: LazyLock<(Kind, &str, u32)> = LazyLock::new(|| (Kind::Transaction, "ledger.withdraw", revision::<Withdraw>()));
pub static FREEZE: LazyLock<(Kind, &str, u32)> = LazyLock::new(|| (Kind::Transaction, "ledger.freeze", revision::<()>()));
pub static EXPLODE: LazyLock<(Kind, &str, u32)> = LazyLock::new(|| (Kind::Transaction, "ledger.explode", revision::<()>()));
pub static NOTED: LazyLock<(Kind, &str, u32)> = LazyLock::new(|| (Kind::Record, "ledger.noted", revision::<Noted>()));

/* === Macros === */

/* === Type Aliases === */

pub type Stored = Arc<Mutex<Vec<Arc<Entry<Recorded<Bank>>>>>>;

/* === Structs === */

#[derive(harmos_runtime::Described, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Account(pub u8);

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Ledger {
    pub balances: BTreeMap<Account, i64>,
    pub notes: Vec<String>,
    pub applications: u64,
    pub bulk: Vec<u64>,
}

#[derive(harmos_runtime::Described, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Deposit {
    pub account: Account,
    pub amount: i64,
}

#[derive(harmos_runtime::Described, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Withdraw {
    pub account: Account,
    pub amount: i64,
}

#[derive(harmos_runtime::Described, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Noted(pub String);

#[derive(Clone, Copy, Debug)]
pub struct Bank;

#[derive(Clone)]
struct Definitions(Vec<(Kind, DefinitionId, u32)>);

pub struct Mirror {
    checkpoint: Arc<AtomicU64>,
}

pub struct Copied {
    entries: Stored,
    resume: Position,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NotPositive,
    Overdrawn,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Failed {
    #[error("{0}")]
    Attempt(String),

    #[error(transparent)]
    Refused(#[from] harmos_runtime::Refusal),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Deposit(Deposit),
    Withdraw(Withdraw),
    Freeze,
    Explode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fact {
    Noted(Noted),
}

/* === Traits === */

/* === Implementations === */

impl Ledger {
    pub fn balance(&self, account: Account) -> i64 {
        self.balances.get(&account).copied().unwrap_or_default()
    }

    pub fn bulky(words: usize) -> Self {
        Self { bulk: vec![0; words], ..Self::default() }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotPositive => "the amount must be positive",
            Self::Overdrawn => "the balance would go negative",
        })
    }
}

impl std::error::Error for Refusal {}

impl From<&str> for Failed {
    fn from(message: &str) -> Self {
        Self::Attempt(message.to_owned())
    }
}

impl From<String> for Failed {
    fn from(message: String) -> Self {
        Self::Attempt(message)
    }
}

impl harmos_runtime::Lifecycle for Bank {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), setup: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        setup.settings().lanes(
            harmos_runtime::operations::LaneSet::new([crate::support::SERIAL, crate::support::CONCURRENT, crate::support::UNBOUNDED])
                .unwrap(),
        );
        Ok(Self)
    }
}

impl Application for Bank {
    type State = Ledger;
    type Transaction = Change;
    type Record = Fact;
}

impl Registration<Bank> for Definitions {
    fn register(self) -> Catalog<Bank> {
        fn decode(id: &DefinitionId, version: u32, bytes: &[u8]) -> Result<Option<Change>, DecodeError> {
            let malformed = |_| DecodeError::Malformed { id: id.clone(), version };
            match (id.as_str(), version) {
                (id, version) if (id, version) == (DEPOSIT.1, DEPOSIT.2) => {
                    serde_json::from_slice(bytes).map(Change::Deposit).map(Some).map_err(malformed)
                }
                (id, version) if (id, version) == (WITHDRAW.1, WITHDRAW.2) => {
                    serde_json::from_slice(bytes).map(Change::Withdraw).map(Some).map_err(malformed)
                }
                (id, version) if (id, version) == (FREEZE.1, FREEZE.2) => {
                    serde_json::from_slice::<()>(bytes).map(|()| Some(Change::Freeze)).map_err(malformed)
                }
                (id, version) if (id, version) == (EXPLODE.1, EXPLODE.2) => {
                    serde_json::from_slice::<()>(bytes).map(|()| Some(Change::Explode)).map_err(malformed)
                }
                _ => Ok(None),
            }
        }

        Catalog::Transactions {
            definitions: self.0,
            decode,
            inverses: Some(inverses),
        }
    }
}

impl Transaction<Ledger> for Deposit {
    type Error = Refusal;

    fn check(&self, _ledger: &Ledger) -> Result<(), Self::Error> {
        (self.amount > 0).then_some(()).ok_or(Refusal::NotPositive)
    }

    fn apply(&self, ledger: &mut Ledger) {
        *ledger.balances.entry(self.account).or_default() += self.amount;
        ledger.applications += 1;
    }
}

impl Invertible<Ledger> for Deposit {
    type Inverse = Withdraw;

    fn inverse(&self, _before: &Ledger) -> Self::Inverse {
        Withdraw {
            account: self.account,
            amount: self.amount,
        }
    }
}

impl Transaction<Ledger> for Withdraw {
    type Error = Refusal;

    fn check(&self, ledger: &Ledger) -> Result<(), Self::Error> {
        if self.amount <= 0 {
            return Err(Refusal::NotPositive);
        }
        (ledger.balance(self.account) >= self.amount).then_some(()).ok_or(Refusal::Overdrawn)
    }

    fn apply(&self, ledger: &mut Ledger) {
        *ledger.balances.entry(self.account).or_default() -= self.amount;
        ledger.applications += 1;
    }
}

impl Invertible<Ledger> for Withdraw {
    type Inverse = Deposit;

    fn inverse(&self, _before: &Ledger) -> Self::Inverse {
        Deposit {
            account: self.account,
            amount: self.amount,
        }
    }
}

impl Transaction<Ledger> for Change {
    type Error = Refusal;

    fn check(&self, ledger: &Ledger) -> Result<(), Self::Error> {
        match self {
            Self::Deposit(change) => change.check(ledger),
            Self::Withdraw(change) => change.check(ledger),
            Self::Freeze | Self::Explode => Ok(()),
        }
    }

    fn apply(&self, ledger: &mut Ledger) {
        match self {
            Self::Deposit(change) => change.apply(ledger),
            Self::Withdraw(change) => change.apply(ledger),
            Self::Freeze => ledger.applications += 1,
            Self::Explode => panic!("apply exploded"),
        }
    }
}

impl From<Deposit> for Change {
    fn from(change: Deposit) -> Self {
        Self::Deposit(change)
    }
}

impl From<Withdraw> for Change {
    fn from(change: Withdraw) -> Self {
        Self::Withdraw(change)
    }
}

impl From<Noted> for Fact {
    fn from(fact: Noted) -> Self {
        Self::Noted(fact)
    }
}

impl Mirror {
    pub fn new() -> (Self, Arc<AtomicU64>) {
        let checkpoint = Arc::new(AtomicU64::new(0));

        (Self { checkpoint: Arc::clone(&checkpoint) }, checkpoint)
    }
}

impl JournalConsumer<Bank> for Mirror {
    fn resume(&self) -> Position {
        Position(self.checkpoint.load(Ordering::Acquire))
    }

    async fn drain(&mut self, _entries: Vec<Arc<Entry<Recorded<Bank>>>>, applied: Position) -> Position {
        self.checkpoint.store(applied.0, Ordering::Release);

        applied
    }
}

impl Copied {
    pub fn new() -> (Self, Stored) {
        let entries = Arc::new(Mutex::new(Vec::new()));

        (Self::resuming(&entries), entries)
    }

    pub fn resuming(entries: &Stored) -> Self {
        Self {
            entries: Arc::clone(entries),
            resume: Position::ORIGIN,
        }
    }

    fn held(&self) -> std::sync::MutexGuard<'_, Vec<Arc<Entry<Recorded<Bank>>>>> {
        self.entries.lock().expect("the copy is never poisoned")
    }
}

impl JournalConsumer<Bank> for Copied {
    fn resume(&self) -> Position {
        self.resume
    }

    async fn drain(&mut self, entries: Vec<Arc<Entry<Recorded<Bank>>>>, _applied: Position) -> Position {
        let mut reached = self.resume;
        let mut held = self.held();

        for entry in entries {
            reached = entry.position;
            held.push(entry);
        }
        drop(held);

        // A copy is durable exactly as far as what it was handed, never as far
        // as what the writer has applied.
        self.resume = reached;

        reached
    }
}

impl Source<Bank> for Copied {
    async fn load(&mut self) -> Result<Recovery<Bank>, Incompatible> {
        let entries: Vec<Envelope<Payload<Bank>>> = self.held().iter().map(|entry| stored(entry)).collect();

        self.resume = entries.last().map_or(Position::ORIGIN, |envelope| envelope.position);

        Ok(Recovery {
            commits: Vec::new(),
            frontier: self.resume,
            entries,
            // A copy that keeps values rather than bytes has no stored
            // encoding to forward.
            encoded: Vec::new(),
        })
    }
}

impl harmos_runtime::state::CatalogCodec for Change {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        let (_, id, version) = match self {
            Self::Deposit(_) => *DEPOSIT,
            Self::Withdraw(_) => *WITHDRAW,
            Self::Freeze => *FREEZE,
            Self::Explode => *EXPLODE,
        };
        (DefinitionId::new(id), version)
    }
    fn encode<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Deposit(v) => serde::Serialize::serialize(v, serializer),
            Self::Withdraw(v) => serde::Serialize::serialize(v, serializer),
            Self::Freeze | Self::Explode => serializer.serialize_unit(),
        }
    }
}

impl harmos_runtime::state::CatalogCodec for Fact {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        (DefinitionId::new(NOTED.1), NOTED.2)
    }
    fn encode<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Noted(v) => serde::Serialize::serialize(v, serializer),
        }
    }
}

impl harmos_runtime::state::RecordCatalog for Fact {
    fn restore(id: &harmos_runtime::DefinitionId, version: u32, payload: &serde_json::Value) -> Result<Option<Self>, serde_json::Error> {
        if id.as_str() != "ledger.noted" || version != NOTED.2 {
            return Ok(None);
        }
        serde::Deserialize::deserialize(payload).map(|v| Some(Self::Noted(v)))
    }
}

impl harmos_runtime::state::TransactionCatalog<Ledger> for Change {}

/* === Public Functions === */

fn revision<T: harmos_runtime::schema::Described>() -> u32 {
    let graph =
        harmos_runtime::schema::graph::Schema::from_declaration(T::DEFINITION, &harmos_runtime::config::SchemaSettings::default()).unwrap();
    harmos_runtime::schema::identity::revision(&graph).unwrap()
}

pub fn catalog() -> impl Registration<Bank> {
    Definitions([*DEPOSIT, *WITHDRAW, *FREEZE, *EXPLODE, *NOTED].map(|(kind, id, version)| (kind, DefinitionId::new(id), version)).into())
}

pub fn deposit(amount: i64) -> Change {
    deposit_to(HOUSE, amount)
}

pub fn deposit_to(account: Account, amount: i64) -> Change {
    Change::Deposit(Deposit { account, amount })
}

pub fn withdraw(amount: i64) -> Change {
    withdraw_from(HOUSE, amount)
}

pub fn withdraw_from(account: Account, amount: i64) -> Change {
    Change::Withdraw(Withdraw { account, amount })
}

pub fn account(change: &Change) -> Option<Account> {
    match change {
        Change::Deposit(change) => Some(change.account),
        Change::Withdraw(change) => Some(change.account),
        Change::Freeze | Change::Explode => None,
    }
}

pub fn streams() -> Vec<Definition> {
    feeds::stream_definitions()
}

pub fn pulse(sample: u64) -> Pulse {
    Pulse { sample }
}

pub async fn balance(runtime: &Runtime<Bank>, account: Account) -> i64 {
    runtime.state.state().read(|ledger| ledger.balance(account)).await.expect("reads are open").value
}

pub async fn ledger(runtime: &Runtime<Bank>) -> Ledger {
    runtime.state.state().read(Clone::clone).await.expect("reads are open").value
}

pub fn alice() -> PrincipalId {
    PrincipalId::new("alice")
}

pub fn bob() -> PrincipalId {
    PrincipalId::new("bob")
}

pub async fn quietly<T>(act: impl Future<Output = T>) -> T {
    let reporting = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = act.await;
    std::panic::set_hook(reporting);

    outcome
}

/* === Private Functions === */

fn inverses(change: &Change, ledger: &Ledger) -> Option<Change> {
    match change {
        Change::Deposit(change) => Some(change.inverse(ledger).into()),
        Change::Withdraw(change) => Some(change.inverse(ledger).into()),
        Change::Freeze | Change::Explode => None,
    }
}

fn stored(entry: &Entry<Recorded<Bank>>) -> Envelope<Payload<Bank>> {
    let (kind, id, version) = match &entry.value {
        Recorded::Transaction(change) => match &**change {
            Change::Deposit(_) => *DEPOSIT,
            Change::Withdraw(_) => *WITHDRAW,
            Change::Freeze => *FREEZE,
            Change::Explode => *EXPLODE,
        },
        Recorded::Record(_) => *NOTED,
        Recorded::System(fact) => {
            let (id, version) = fact.identify();
            return Envelope {
                position: entry.position,
                kind: Kind::Record,
                id: id.clone(),
                version,
                metadata: entry.metadata.clone(),
                payload: Payload {
                    prepared: None,
                    id,
                    version,
                    value: Recorded::System(fact.clone()),
                },
            };
        }
    };
    let value = match &entry.value {
        Recorded::Transaction(change) => Recorded::Transaction(change.clone()),
        Recorded::Record(fact) => Recorded::Record(fact.clone()),
        Recorded::System(fact) => Recorded::System(fact.clone()),
    };

    Envelope {
        position: entry.position,
        kind,
        id: DefinitionId::new(id),
        version,
        metadata: entry.metadata.clone(),
        payload: Payload {
            prepared: None,
            id: DefinitionId::new(id),
            version,
            value,
            // Everything this copy holds was written by the version this build
            // writes, so the catalog's own apply is the one that wrote it.
        },
    }
}

mod feeds {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports
    use serde::{Deserialize, Serialize};

    // Local Imports

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    #[harmos_runtime::stream(
        description = "A waveform-shaped sample row: the bank's high-rate feed.\nThe window is deliberately small so eviction is cheap to provoke.",
        id = "pulse",
        window = 8
    )]
    #[harmos_runtime::message]
    #[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Pulse {
        pub sample: u64,
    }

    #[harmos_runtime::stream(
        description = "A second, slower feed, so per-stream claims have a counterexample.",
        id = "reading",
        window = 4
    )]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Reading {
        pub value: i64,
    }

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    /* === Public Functions === */

    pub fn stream_definitions() -> Vec<harmos_runtime::streams::Definition> {
        vec![<Pulse>::definition(), <Reading>::definition()]
    }

    /* === Private Functions === */

    /* === Tests === */
}

/* === Tests === */
