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

#[harmos_runtime::transaction(id = "ledger.deposit",  inverse = Withdraw)]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Deposit {
    pub amount: i64,
}

#[harmos_runtime::transaction(id = "ledger.withdraw",  inverse = Deposit)]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Withdraw {
    pub amount: i64,
}

#[harmos_runtime::transaction(id = "ledger.pay")]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Pay {
    pub effect: u64,
    pub amount: i64,
}

#[harmos_runtime::transaction(id = "ledger.bonus")]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Bonus {
    pub amount: i64,
}

#[harmos_runtime::transaction(id = "ledger.explode")]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Explode;

#[harmos_runtime::record(id = "ledger.noted")]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
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

#[harmos_runtime::lanes]
pub enum Lanes {
    #[default]
    Open,

    #[lane(serial)]
    Serial,

    #[lane(concurrent(2))]
    Concurrent,

    #[lane(unbounded)]
    Unbounded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NotPositive,
    Overdrawn,
}

#[harmos_runtime::transactions(state = Ledger, error = Refusal)]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Change {
    Deposit(Deposit),
    Withdraw(Withdraw),
    Pay(Pay),
    Bonus(Bonus),
    Explode(Explode),
}

#[harmos_runtime::records]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Fact {
    Noted(Noted),
}

/* === Traits === */

/* === Implementations === */

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

impl Application for Bank {
    type Lanes = Lanes;
    type State = Ledger;
    type Transaction = Change;
    type Record = Fact;
    type Streams = ();
}

impl Wire for Bank {
    fn identify(value: &Recorded<Self>) -> (DefinitionId, u32) {
        match value {
            Recorded::Transaction(change) => change.identify(),
            Recorded::Record(fact) => fact.identify(),
            Recorded::System(_) => unreachable!("system facts are encoded by the journal"),
        }
    }

    fn encode<S: Serializer>(value: &Recorded<Self>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Recorded::Transaction(change) => change.encode_with(serializer),
            Recorded::Record(fact) => fact.encode_with(serializer),
            Recorded::System(_) => unreachable!("system facts are encoded by the journal"),
        }
    }

    fn decode<'de, D: Deserializer<'de> + Copy>(
        kind: Kind, id: &DefinitionId, version: u32, payload: D,
    ) -> Result<Option<Payload<Self>>, D::Error> {
        Ok(match kind {
            Kind::Transaction => Change::restore(id, version, payload)?.map(|change| Payload {
                prepared: None,
                id: id.clone(),
                version,
                value: Recorded::Transaction(harmos_runtime::operations::Response::new(change)),
            }),
            Kind::Record => Fact::restore(id, version, payload)?.map(|fact| Payload {
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
            definitions: self.0,
            decode,
            inverses: Some(Change::__harmos_inverses),
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
    Definitions(definitions_of::<Bank>(Change::catalog()).into_iter().chain(definitions_of::<Bank>(Fact::catalog())).collect())
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
        setup.settings().register(Change::catalog()).register(Fact::catalog());
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
