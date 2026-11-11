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

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fmt;
use std::path::PathBuf;
// Third Party Imports
use serde::{Deserialize, Deserializer, Serialize, Serializer};
// Local Imports
use harmos_runtime::{
    Application, DefinitionId, Runtime, Setup, StartupError, runtime::Catalog, runtime::Registration, state::DecodeError,
    state::Invertible, state::Kind, state::PrincipalId, state::Recorded, state::Transaction, storage::Payload,
};
use schematic_storage::Wire;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct Ledger {
    pub balance: i64,
    pub applications: u64,
    pub payouts: Vec<u64>,
}

#[harmos_runtime::transaction(id = "ledger.deposit",  inverse = Withdraw)]
#[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Deposit {
    pub amount: i64,
}

#[harmos_runtime::transaction(id = "ledger.withdraw",  inverse = Deposit)]
#[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Withdraw {
    pub amount: i64,
}

#[harmos_runtime::transaction(id = "ledger.pay")]
#[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Pay {
    pub effect: u64,
    pub amount: i64,
}

#[harmos_runtime::transaction(id = "ledger.bonus")]
#[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Bonus {
    pub amount: i64,
}

#[harmos_runtime::transaction(id = "ledger.explode")]
#[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Explode;

#[derive(harmos_runtime::Described, Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Noted(pub String);

#[derive(Clone, Copy, Debug)]
pub struct Bank;

#[derive(Clone)]
struct Definitions(Vec<(Kind, DefinitionId, u32)>);

/* === Enums === */

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

    async fn prepare(_: (), _setup: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
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
            Recorded::Transaction(change) => change.identify(),
            Recorded::Record(fact) => harmos_runtime::state::CatalogCodec::identify(fact),
            Recorded::System(_) => unreachable!("system facts are encoded by the adapter"),
        }
    }

    fn encode<S: Serializer>(value: &Recorded<Self>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Recorded::Transaction(change) => change.encode_with(serializer),
            Recorded::Record(fact) => harmos_runtime::state::CatalogCodec::encode(fact, serializer),
            Recorded::System(_) => unreachable!("system facts are encoded by the adapter"),
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
            states: Some(Change::catalog()),
            records: Some(fact_registry()),
            definitions: self.0,
            decode: std::sync::Arc::new(decode),
            inverses: Some(std::sync::Arc::new(Change::__harmos_inverses)),
        }
    }
}

/* === Public Functions === */

pub fn catalog() -> impl Registration<Bank> {
    Definitions(definitions_of::<Bank>(Change::catalog()).into_iter().chain(definitions_of::<Bank>(fact_registry())).collect())
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
        setup.settings().register(Change::catalog()).register(fact_registry());
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

pub fn scratch(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);

    std::fs::remove_dir_all(&directory).ok();
    std::fs::create_dir_all(&directory).expect("the target directory is writable");

    directory
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
