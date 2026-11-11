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

#[path = "journal.rs"]
pub mod journal;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
// Third Party Imports
use serde::{Deserialize, Deserializer, Serialize, Serializer};
// Local Imports
use harmos_runtime::{
    Application, DefinitionId, Runtime, Setup, StartupError, runtime::Catalog, runtime::Registration, state::DecodeError, state::Entry,
    state::Invertible, state::Kind, state::Position, state::PrincipalId, state::Recorded, state::Transaction, storage::JournalConsumer,
    storage::Payload,
};
use journal::Wire;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Dispatched = (Arc<AtomicU64>, Arc<std::sync::Mutex<Vec<u64>>>);

/* === Structs === */

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct Ledger {
    pub balance: i64,
    pub applications: u64,
    pub payouts: Vec<u64>,
}

#[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Deposit {
    pub amount: i64,
}
impl Deposit {
    pub const ENTRY: harmos_runtime::state::TransactionEntry<Ledger, Self> =
        harmos_runtime::state::TransactionEntry::new("ledger.deposit").reversible::<Withdraw>();
}

#[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Withdraw {
    pub amount: i64,
}
impl Withdraw {
    pub const ENTRY: harmos_runtime::state::TransactionEntry<Ledger, Self> =
        harmos_runtime::state::TransactionEntry::new("ledger.withdraw").reversible::<Deposit>();
}

#[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Pay {
    pub effect: u64,
    pub amount: i64,
}
impl Pay {
    pub const ENTRY: harmos_runtime::state::TransactionEntry<Ledger, Self> =
        harmos_runtime::state::TransactionEntry::new("ledger.pay").barrier();
}

#[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Bonus {
    pub amount: i64,
}
impl Bonus {
    pub const ENTRY: harmos_runtime::state::TransactionEntry<Ledger, Self> =
        harmos_runtime::state::TransactionEntry::new("ledger.bonus").barrier();
}

#[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Explode;
impl Explode {
    pub const ENTRY: harmos_runtime::state::TransactionEntry<Ledger, Self> =
        harmos_runtime::state::TransactionEntry::new("ledger.explode").barrier();
}

#[derive(harmos_runtime::Described, Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Noted(pub String);

#[derive(Clone, Copy, Debug)]
pub struct Bank;

#[derive(Clone)]
struct Definitions(Vec<(Kind, DefinitionId, u32)>);

pub struct Payouts {
    checkpoint: Arc<AtomicU64>,
    dispatched: Arc<std::sync::Mutex<Vec<u64>>>,
    holding: Option<u64>,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NotPositive,
    Overdrawn,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Change {
    Deposit(Deposit),
    Withdraw(Withdraw),
    Pay(Pay),
    Bonus(Bonus),
    Explode(Explode),
}
impl From<Deposit> for Change {
    fn from(value: Deposit) -> Self {
        Self::Deposit(value)
    }
}
impl From<Withdraw> for Change {
    fn from(value: Withdraw) -> Self {
        Self::Withdraw(value)
    }
}
impl From<Pay> for Change {
    fn from(value: Pay) -> Self {
        Self::Pay(value)
    }
}
impl From<Bonus> for Change {
    fn from(value: Bonus) -> Self {
        Self::Bonus(value)
    }
}
impl From<Explode> for Change {
    fn from(value: Explode) -> Self {
        Self::Explode(value)
    }
}
impl harmos_runtime::state::Transaction<Ledger> for Change {
    type Error = Refusal;
    fn check(&self, state: &Ledger) -> std::result::Result<(), Self::Error> {
        match self {
            Self::Deposit(value) => harmos_runtime::state::Transaction::check(value, state).map_err(Into::into),
            Self::Withdraw(value) => harmos_runtime::state::Transaction::check(value, state).map_err(Into::into),
            Self::Pay(value) => harmos_runtime::state::Transaction::check(value, state).map_err(Into::into),
            Self::Bonus(value) => harmos_runtime::state::Transaction::check(value, state).map_err(Into::into),
            Self::Explode(value) => harmos_runtime::state::Transaction::check(value, state).map_err(Into::into),
        }
    }
    fn apply(&self, state: &mut Ledger) {
        match self {
            Self::Deposit(value) => harmos_runtime::state::Transaction::apply(value, state),
            Self::Withdraw(value) => harmos_runtime::state::Transaction::apply(value, state),
            Self::Pay(value) => harmos_runtime::state::Transaction::apply(value, state),
            Self::Bonus(value) => harmos_runtime::state::Transaction::apply(value, state),
            Self::Explode(value) => harmos_runtime::state::Transaction::apply(value, state),
        }
    }
    fn records(&self, state: &Ledger, records: &mut harmos_runtime::state::Records) {
        match self {
            Self::Deposit(value) => harmos_runtime::state::Transaction::records(value, state, records),
            Self::Withdraw(value) => harmos_runtime::state::Transaction::records(value, state, records),
            Self::Pay(value) => harmos_runtime::state::Transaction::records(value, state, records),
            Self::Bonus(value) => harmos_runtime::state::Transaction::records(value, state, records),
            Self::Explode(value) => harmos_runtime::state::Transaction::records(value, state, records),
        }
    }
}
impl harmos_runtime::state::CatalogCodec for Change {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        match self {
            Self::Deposit(_) => Deposit::ENTRY.definition(),
            Self::Withdraw(_) => Withdraw::ENTRY.definition(),
            Self::Pay(_) => Pay::ENTRY.definition(),
            Self::Bonus(_) => Bonus::ENTRY.definition(),
            Self::Explode(_) => Explode::ENTRY.definition(),
        }
    }
    fn encode<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Deposit(value) => serde::Serialize::serialize(value, serializer),
            Self::Withdraw(value) => serde::Serialize::serialize(value, serializer),
            Self::Pay(value) => serde::Serialize::serialize(value, serializer),
            Self::Bonus(value) => serde::Serialize::serialize(value, serializer),
            Self::Explode(value) => serde::Serialize::serialize(value, serializer),
        }
    }
}
pub fn change_state_catalog() -> harmos_runtime::state::StateCatalog<Ledger, Change> {
    harmos_runtime::state::StateCatalog::new()
        .transaction(Deposit::ENTRY, Change::Deposit, |value| match value {
            Change::Deposit(value) => Some(value),
            _ => None,
        })
        .transaction(Withdraw::ENTRY, Change::Withdraw, |value| match value {
            Change::Withdraw(value) => Some(value),
            _ => None,
        })
        .transaction(Pay::ENTRY, Change::Pay, |value| match value {
            Change::Pay(value) => Some(value),
            _ => None,
        })
        .transaction(Bonus::ENTRY, Change::Bonus, |value| match value {
            Change::Bonus(value) => Some(value),
            _ => None,
        })
        .transaction(Explode::ENTRY, Change::Explode, |value| match value {
            Change::Explode(value) => Some(value),
            _ => None,
        })
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Fact {
    Noted(Noted),
}
impl From<Noted> for Fact {
    fn from(value: Noted) -> Self {
        Self::Noted(value)
    }
}
impl harmos_runtime::state::CatalogCodec for Fact {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        match self {
            Self::Noted(_) => <Noted as harmos_runtime::state::Record>::ENTRY.definition(),
        }
    }
    fn encode<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Noted(value) => serde::Serialize::serialize(value, serializer),
        }
    }
}
pub fn fact_registry() -> harmos_runtime::state::RecordRegistry<Fact> {
    harmos_runtime::state::RecordRegistry::new().entry(<Noted as harmos_runtime::state::Record>::ENTRY, Fact::Noted, |value| {
        let Fact::Noted(value) = value;
        Some(value)
    })
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::state::Record for Noted {
    const ENTRY: harmos_runtime::state::RecordEntry<Self> = harmos_runtime::state::RecordEntry::new("ledger.noted");
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

impl Transaction<Ledger> for Deposit {
    type Error = Refusal;

    fn check(&self, _ledger: &Ledger) -> Result<(), Self::Error> {
        (self.amount > 0).then_some(()).ok_or(Refusal::NotPositive)
    }

    fn apply(&self, ledger: &mut Ledger) {
        ledger.balance += self.amount;
        ledger.applications += 1;
    }
}

impl Invertible<Ledger> for Deposit {
    type Inverse = Withdraw;

    fn inverse(&self, _before: &Ledger) -> Withdraw {
        Withdraw { amount: self.amount }
    }
}

impl Transaction<Ledger> for Withdraw {
    type Error = Refusal;

    fn check(&self, ledger: &Ledger) -> Result<(), Self::Error> {
        if self.amount <= 0 {
            return Err(Refusal::NotPositive);
        }
        (ledger.balance >= self.amount).then_some(()).ok_or(Refusal::Overdrawn)
    }

    fn apply(&self, ledger: &mut Ledger) {
        ledger.balance -= self.amount;
        ledger.applications += 1;
    }
}

impl Invertible<Ledger> for Withdraw {
    type Inverse = Deposit;

    fn inverse(&self, _before: &Ledger) -> Deposit {
        Deposit { amount: self.amount }
    }
}

impl Transaction<Ledger> for Pay {
    type Error = Refusal;

    fn check(&self, ledger: &Ledger) -> Result<(), Self::Error> {
        (ledger.balance >= self.amount).then_some(()).ok_or(Refusal::Overdrawn)
    }

    fn apply(&self, ledger: &mut Ledger) {
        ledger.balance -= self.amount;
        ledger.payouts.push(self.effect);
        ledger.applications += 1;
    }
}

impl Transaction<Ledger> for Bonus {
    type Error = Refusal;

    fn check(&self, _ledger: &Ledger) -> Result<(), Self::Error> {
        (self.amount > 0).then_some(()).ok_or(Refusal::NotPositive)
    }

    fn apply(&self, ledger: &mut Ledger) {
        ledger.balance += self.amount;
        ledger.applications += 1;
    }
}

impl Transaction<Ledger> for Explode {
    type Error = Refusal;

    fn check(&self, _ledger: &Ledger) -> Result<(), Self::Error> {
        Ok(())
    }

    fn apply(&self, _ledger: &mut Ledger) {
        panic!("apply exploded");
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

impl Wire for Bank {
    fn identify(value: &Recorded<Self>) -> (DefinitionId, u32) {
        match value {
            Recorded::Transaction(change) => harmos_runtime::state::CatalogCodec::identify(&**change),
            Recorded::Record(fact) => harmos_runtime::state::CatalogCodec::identify(fact),
            Recorded::System(_) => unreachable!("system facts are encoded by the journal"),
        }
    }

    fn encode<S: Serializer>(value: &Recorded<Self>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Recorded::Transaction(change) => harmos_runtime::state::CatalogCodec::encode(&**change, serializer),
            Recorded::Record(fact) => harmos_runtime::state::CatalogCodec::encode(fact, serializer),
            Recorded::System(_) => unreachable!("system facts are encoded by the journal"),
        }
    }

    fn decode<'de, D: Deserializer<'de> + Copy>(
        kind: Kind, id: &DefinitionId, version: u32, payload: D,
    ) -> Result<Option<Payload<Self>>, D::Error> {
        Ok(match kind {
            Kind::Transaction => change_state_catalog().restore(id, version, payload)?.map(|change| Payload {
                prepared: None,
                id: id.clone(),
                version,
                value: Recorded::Transaction(harmos_runtime::operations::Response::new(change)),
            }),
            Kind::Record => fact_registry().restore(id, version, payload)?.map(|fact| Payload {
                prepared: None,
                id: id.clone(),
                version,
                value: Recorded::Record(fact),
            }),
        })
    }
}

impl Registration<Bank> for Definitions {
    fn register(self) -> Catalog<Bank> {
        fn decode(_id: &DefinitionId, _version: u32, _bytes: &[u8]) -> Result<Option<Change>, DecodeError> {
            Ok(None)
        }

        Catalog::Transactions {
            states: Some(change_state_catalog()),
            records: Some(fact_registry()),
            definitions: self.0,
            decode: std::sync::Arc::new(decode),
            inverses: Some({
                let catalog = change_state_catalog();
                std::sync::Arc::new(move |change, before| catalog.inverse(change, before))
            }),
        }
    }
}

impl Payouts {
    pub fn resuming(watched: &Dispatched) -> Self {
        Self {
            checkpoint: Arc::clone(&watched.0),
            dispatched: Arc::clone(&watched.1),
            holding: None,
        }
    }

    pub fn new() -> (Self, Dispatched) {
        let watched = (Arc::new(AtomicU64::new(0)), Arc::new(std::sync::Mutex::new(Vec::new())));

        (Self::resuming(&watched), watched)
    }

    pub fn holding(mut self, effect: u64) -> Self {
        self.holding = Some(effect);
        self
    }
}

impl JournalConsumer<Bank> for Payouts {
    fn resume(&self) -> Position {
        Position(self.checkpoint.load(Ordering::Acquire))
    }

    async fn drain(&mut self, entries: Vec<Arc<Entry<Recorded<Bank>>>>, _applied: Position) -> Position {
        let mut reached = self.resume();

        for entry in &entries {
            if let Recorded::Transaction(change) = &entry.value
                && let Change::Pay(payout) = &**change
            {
                if self.holding == Some(payout.effect) {
                    // A checkpoint never advances past an unacknowledged item.
                    self.checkpoint.store(reached.0, Ordering::Release);

                    return reached;
                }
                self.dispatched.lock().expect("the listener is never poisoned").push(payout.effect);
            }
            reached = entry.position;
        }
        // An empty drain is the frontier question alone, and the checkpoint
        // answers for itself: it never adopts `applied`, because a held
        // payout may still be owing.
        self.checkpoint.store(reached.0, Ordering::Release);

        reached
    }
}

/* === Public Functions === */

pub fn catalog() -> impl Registration<Bank> {
    Definitions(definitions_of::<Bank>(change_state_catalog()).into_iter().chain(definitions_of::<Bank>(fact_registry())).collect())
}

pub fn deposit(amount: i64) -> Deposit {
    Deposit { amount }
}

pub fn withdraw(amount: i64) -> Withdraw {
    Withdraw { amount }
}

pub fn pay(effect: u64, amount: i64) -> Pay {
    Pay { effect, amount }
}

pub async fn booted(origin: Ledger, attach: impl FnOnce(&mut Setup<Bank>)) -> Runtime<Bank> {
    opened(origin, attach).await.expect("the copy replays")
}

pub async fn opened(origin: Ledger, attach: impl FnOnce(&mut Setup<Bank>)) -> Result<Runtime<Bank>, StartupError> {
    Runtime::<Bank>::launch_with(origin, (), |setup| {
        setup.settings().register(change_state_catalog()).register(fact_registry());
        attach(setup);
    })
    .await
}

pub async fn ledger(runtime: &Runtime<Bank>) -> Ledger {
    runtime.state.state().read(Clone::clone).await.expect("reads are open").value
}

pub fn alice() -> PrincipalId {
    PrincipalId::new("alice")
}

/* === Private Functions === */

fn definitions_of<A: Application>(catalog: impl Registration<A>) -> Vec<(Kind, DefinitionId, u32)> {
    match catalog.register() {
        Catalog::Transactions { definitions, .. } => definitions,
        Catalog::Streams(_) => {
            unreachable!("a transaction or fact catalog always registers as `Transactions`")
        }
    }
}

/* === Tests === */
