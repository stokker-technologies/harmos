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
#[path = "../../../../tests/support/fixtures.rs"]
pub mod fixtures;
mod feeds {
    use serde::{Deserialize, Serialize};

    #[harmos::stream(
        description = "A waveform-shaped sample row: the bank's high-rate feed.\nThe window is deliberately small so eviction is cheap to provoke.",
        id = "pulse",
        window = 8
    )]
    #[harmos::message]
    #[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Pulse {
        pub sample: u64,
    }

    #[harmos::stream(
        description = "A second, slower feed, so per-stream claims have a counterexample.",
        id = "reading",
        window = 4
    )]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Reading {
        pub value: i64,
    }

    #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
    pub struct Run(pub u64);

    impl harmos::streams::Track for Run {
        fn label(&self) -> String {
            format!("run_{}", self.0)
        }
    }

    #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
    pub struct Probe(pub u64);

    impl harmos::streams::Track for Probe {
        fn label(&self) -> String {
            format!("probe_{}", self.0)
        }
    }

    #[harmos::stream(description = "A tracked capture row carrying both declared coordinates.\nThe track is two components deep, which is what a run of many probes is: one probe of one run, or everything one run published, and both are selections of the same declaration.",
        id = "capture",
        window = 4,
        track = (run, probe),
        axis = ordinal
    )]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Capture {
        pub run: Run,
        pub probe: Probe,
        pub ordinal: u64,
        pub value: i64,
    }

    #[harmos::streams]
    pub enum Bank {
        Pulse(Pulse),
        Reading(Reading),
        Capture(Capture),
    }
}

/* === Re-Exports === */
#[allow(
    unused_imports,
    reason = "each integration-test binary compiles only the fixture slice it needs"
)]
pub use self::{
    app::{booted, runtime, runtime_from, streaming},
    feeds::{Capture, Probe, Pulse, Reading, Run},
};

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
// Third Party Imports

// Local Imports
use harmos::{
    Application, DefinitionId, Runtime, runtime::Catalog, runtime::Incompatible,
    runtime::Registration, state::DecodeError, state::Entry, state::Envelope, state::Invert,
    state::Kind, state::Position, state::PrincipalId, state::Recorded, state::Transaction,
    state::Undoable, storage::JournalConsumer, storage::Payload, storage::Recovery,
    storage::Source, streams::Definition,
};

/* === Constants === */
pub const HOUSE: Account = Account(0);

pub const VAULT: Account = Account(1);

pub const DEPOSIT: (Kind, &str, u32) = (Kind::Transaction, "ledger.deposit", 2);

pub const DEPOSIT_V1: (Kind, &str, u32) = (Kind::Transaction, "ledger.deposit", 1);

pub const WITHDRAW: (Kind, &str, u32) = (Kind::Transaction, "ledger.withdraw", 1);

pub const FREEZE: (Kind, &str, u32) = (Kind::Transaction, "ledger.freeze", 1);

pub const EXPLODE: (Kind, &str, u32) = (Kind::Transaction, "ledger.explode", 1);

pub const NOTED: (Kind, &str, u32) = (Kind::Record, "ledger.noted", 1);

/* === Macros === */

/* === Type Aliases === */
pub type Stored = Arc<Mutex<Vec<Arc<Entry<Recorded<Bank>>>>>>;

/* === Structs === */
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Account(pub u8);

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Ledger {
    pub balances: BTreeMap<Account, i64>,
    pub notes: Vec<String>,
    pub applications: u64,
    pub bulk: Vec<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Deposit {
    pub account: Account,
    pub amount: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DepositV1 {
    pub account: Account,
    pub amount: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Withdraw {
    pub account: Account,
    pub amount: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

pub struct Ancient {
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
    Refused(#[from] harmos::Refusal),
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
        Self {
            bulk: vec![0; words],
            ..Self::default()
        }
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

impl Application for Bank {
    type State = Ledger;
    type Transaction = Change;
    type Record = Fact;
    type Streams = feeds::Bank;
    type Resources = ();
}

impl Registration<Bank> for Definitions {
    fn register(self) -> Catalog<Bank> {
        fn decode(
            id: &DefinitionId,
            version: u32,
            bytes: &[u8],
        ) -> Result<Option<Change>, DecodeError> {
            let malformed = |_| DecodeError::Malformed {
                id: id.clone(),
                version,
            };
            match (id.as_str(), version) {
                ("ledger.deposit", 1 | 2) => serde_json::from_slice(bytes)
                    .map(Change::Deposit)
                    .map(Some)
                    .map_err(malformed),
                ("ledger.withdraw", 1) => serde_json::from_slice(bytes)
                    .map(Change::Withdraw)
                    .map(Some)
                    .map_err(malformed),
                ("ledger.freeze", 1) => serde_json::from_slice::<()>(bytes)
                    .map(|()| Some(Change::Freeze))
                    .map_err(malformed),
                ("ledger.explode", 1) => serde_json::from_slice::<()>(bytes)
                    .map(|()| Some(Change::Explode))
                    .map_err(malformed),
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

impl Undoable for Deposit {
    type Inverse = Withdraw;
}

impl Invert<Ledger> for Deposit {
    fn invert(&self, _before: &Ledger) -> Withdraw {
        Withdraw {
            account: self.account,
            amount: self.amount,
        }
    }
}

impl Transaction<Ledger> for DepositV1 {
    type Error = Refusal;

    fn check(&self, _ledger: &Ledger) -> Result<(), Self::Error> {
        (self.amount > 0).then_some(()).ok_or(Refusal::NotPositive)
    }

    fn apply(&self, ledger: &mut Ledger) {
        *ledger.balances.entry(self.account).or_default() += 2 * self.amount;
        ledger.applications += 1;
    }
}

impl From<DepositV1> for Deposit {
    fn from(old: DepositV1) -> Self {
        Self {
            account: old.account,
            amount: old.amount,
        }
    }
}

impl Transaction<Ledger> for Withdraw {
    type Error = Refusal;

    fn check(&self, ledger: &Ledger) -> Result<(), Self::Error> {
        if self.amount <= 0 {
            return Err(Refusal::NotPositive);
        }
        (ledger.balance(self.account) >= self.amount)
            .then_some(())
            .ok_or(Refusal::Overdrawn)
    }

    fn apply(&self, ledger: &mut Ledger) {
        *ledger.balances.entry(self.account).or_default() -= self.amount;
        ledger.applications += 1;
    }
}

impl Undoable for Withdraw {
    type Inverse = Deposit;
}

impl Invert<Ledger> for Withdraw {
    fn invert(&self, _before: &Ledger) -> Deposit {
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

        (
            Self {
                checkpoint: Arc::clone(&checkpoint),
            },
            checkpoint,
        )
    }
}

impl JournalConsumer<Bank> for Mirror {
    fn resume(&self) -> Position {
        Position(self.checkpoint.load(Ordering::Acquire))
    }

    async fn drain(
        &mut self,
        _entries: Vec<Arc<Entry<Recorded<Bank>>>>,
        applied: Position,
    ) -> Position {
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

    async fn drain(
        &mut self,
        entries: Vec<Arc<Entry<Recorded<Bank>>>>,
        _applied: Position,
    ) -> Position {
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
        let entries: Vec<Envelope<Payload<Bank>>> =
            self.held().iter().map(|entry| stored(entry)).collect();

        self.resume = entries
            .last()
            .map_or(Position::ORIGIN, |envelope| envelope.position);

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

impl Ancient {
    pub fn resuming(entries: &Stored) -> Self {
        Self {
            entries: Arc::clone(entries),
            resume: Position::ORIGIN,
        }
    }
}

impl JournalConsumer<Bank> for Ancient {
    fn resume(&self) -> Position {
        self.resume
    }

    async fn drain(
        &mut self,
        _entries: Vec<Arc<Entry<Recorded<Bank>>>>,
        applied: Position,
    ) -> Position {
        self.resume = applied;

        applied
    }
}

impl Source<Bank> for Ancient {
    async fn load(&mut self) -> Result<Recovery<Bank>, Incompatible> {
        let entries: Vec<Envelope<Payload<Bank>>> = self
            .entries
            .lock()
            .expect("the copy is never poisoned")
            .iter()
            .map(|entry| match &entry.value {
                Recorded::Transaction(change) => match &**change {
                    Change::Deposit(deposit) => ancient(entry, *deposit),
                    _ => stored(entry),
                },
                Recorded::Record(_) => stored(entry),
                Recorded::System(_) => stored(entry),
            })
            .collect();

        self.resume = entries
            .last()
            .map_or(Position::ORIGIN, |envelope| envelope.position);

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

impl harmos::state::CatalogCodec for Change {
    fn identify(&self) -> (harmos::DefinitionId, u32) {
        (
            harmos::DefinitionId::new(match self {
                Self::Deposit(_) => "ledger.deposit",
                Self::Withdraw(_) => "ledger.withdraw",
                Self::Freeze => "ledger.freeze",
                Self::Explode => "ledger.explode",
            }),
            if matches!(self, Self::Deposit(_)) {
                2
            } else {
                1
            },
        )
    }
    fn encode<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Deposit(v) => serde::Serialize::serialize(v, serializer),
            Self::Withdraw(v) => serde::Serialize::serialize(v, serializer),
            Self::Freeze | Self::Explode => serializer.serialize_unit(),
        }
    }
}

impl harmos::state::CatalogCodec for Fact {
    fn identify(&self) -> (harmos::DefinitionId, u32) {
        (harmos::DefinitionId::new("ledger.noted"), 1)
    }
    fn encode<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Noted(v) => serde::Serialize::serialize(v, serializer),
        }
    }
}

impl harmos::state::RecordCatalog for Fact {
    fn restore(
        id: &harmos::DefinitionId,
        version: u32,
        payload: &serde_json::Value,
    ) -> Result<Option<Self>, serde_json::Error> {
        if id.as_str() != "ledger.noted" || version != 1 {
            return Ok(None);
        }
        serde::Deserialize::deserialize(payload).map(|v| Some(Self::Noted(v)))
    }
}

impl harmos::state::TransactionCatalog<Ledger> for Change {}

/* === Public Functions === */
pub fn catalog() -> impl Registration<Bank> {
    Definitions(
        [DEPOSIT, DEPOSIT_V1, WITHDRAW, FREEZE, EXPLODE, NOTED]
            .map(|(kind, id, version)| (kind, DefinitionId::new(id), version))
            .into(),
    )
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
    feeds::Bank::catalog()
}

pub fn pulse(sample: u64) -> Pulse {
    Pulse { sample }
}

pub fn capture(run: u64, probe: u64, ordinal: u64, value: i64) -> Capture {
    Capture {
        run: Run(run),
        probe: Probe(probe),
        ordinal,
        value,
    }
}

pub async fn balance(runtime: &Runtime<Bank>, account: Account) -> i64 {
    runtime
        .state
        .state()
        .read(|ledger| ledger.balance(account))
        .await
        .expect("reads are open")
        .value
}

pub async fn ledger(runtime: &Runtime<Bank>) -> Ledger {
    runtime
        .state
        .state()
        .read(Clone::clone)
        .await
        .expect("reads are open")
        .value
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
        Change::Deposit(change) => Some(change.invert(ledger).into()),
        Change::Withdraw(change) => Some(change.invert(ledger).into()),
        Change::Freeze | Change::Explode => None,
    }
}

fn stored(entry: &Entry<Recorded<Bank>>) -> Envelope<Payload<Bank>> {
    let (kind, id, version) = match &entry.value {
        Recorded::Transaction(change) => match &**change {
            Change::Deposit(_) => DEPOSIT,
            Change::Withdraw(_) => WITHDRAW,
            Change::Freeze => FREEZE,
            Change::Explode => EXPLODE,
        },
        Recorded::Record(_) => NOTED,
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
                    apply: None,
                },
            };
        }
    };
    let value = match &entry.value {
        Recorded::Transaction(change) => Recorded::Transaction(Arc::clone(change)),
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
            apply: None,
        },
    }
}

fn ancient(entry: &Entry<Recorded<Bank>>, deposit: Deposit) -> Envelope<Payload<Bank>> {
    let (kind, id, version) = DEPOSIT_V1;
    let old = DepositV1 {
        account: deposit.account,
        amount: deposit.amount,
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
            value: Recorded::Transaction(Arc::new(Change::from(Deposit::from(old)))),
            apply: Some(Box::new(move |ledger: &mut Ledger| old.apply(ledger))),
        },
    }
}

/* === Tests === */
