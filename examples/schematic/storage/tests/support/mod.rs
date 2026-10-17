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
use std::sync::Arc;
// Third Party Imports
use serde::{Deserialize, Deserializer, Serialize, Serializer};
// Local Imports
use harmos::{
    Application, Builder, DefinitionId, Runtime, runtime::Catalog, runtime::Incompatible,
    runtime::Registration, state::DecodeError, state::Invert, state::Kind, state::PrincipalId,
    state::Recorded, state::Transaction, storage::Payload,
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

#[harmos::transaction(id = "ledger.deposit", version = 1, inverse = Withdraw)]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Deposit {
    pub amount: i64,
}

#[harmos::transaction(id = "ledger.withdraw", version = 1, inverse = Deposit)]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Withdraw {
    pub amount: i64,
}

#[harmos::transaction(id = "ledger.pay", version = 1, irreversible)]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Pay {
    pub effect: u64,
    pub amount: i64,
}

#[harmos::transaction(id = "ledger.bonus", version = 2, irreversible, history(BonusV1))]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Bonus {
    pub amount: i64,
}

#[harmos::transaction(version = 1)]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BonusV1 {
    pub amount: i64,
}

#[harmos::transaction(id = "ledger.explode", version = 1, irreversible)]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Explode;

#[harmos::record(id = "ledger.noted", version = 2, history(NotedV1))]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Noted(pub String);

#[harmos::record(version = 1)]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct NotedV1 {
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unreadable;

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

#[harmos::transactions(state = Ledger, error = Refusal)]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Change {
    Deposit(Deposit),
    Withdraw(Withdraw),
    Pay(Pay),
    Bonus(Bonus),
    Explode(Explode),
}

#[harmos::records]
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

impl Invert<Ledger> for Deposit {
    fn invert(&self, _before: &Ledger) -> Withdraw {
        Withdraw {
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
        (ledger.balance >= self.amount)
            .then_some(())
            .ok_or(Refusal::Overdrawn)
    }

    fn apply(&self, ledger: &mut Ledger) {
        ledger.balance -= self.amount;
        ledger.applications += 1;
    }
}

impl Invert<Ledger> for Withdraw {
    fn invert(&self, _before: &Ledger) -> Deposit {
        Deposit {
            amount: self.amount,
        }
    }
}

impl Transaction<Ledger> for Pay {
    type Error = Refusal;

    fn check(&self, ledger: &Ledger) -> Result<(), Self::Error> {
        (ledger.balance >= self.amount)
            .then_some(())
            .ok_or(Refusal::Overdrawn)
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

impl Transaction<Ledger> for BonusV1 {
    type Error = Refusal;

    fn check(&self, _ledger: &Ledger) -> Result<(), Self::Error> {
        (self.amount > 0).then_some(()).ok_or(Refusal::NotPositive)
    }

    fn apply(&self, ledger: &mut Ledger) {
        ledger.balance += 2 * self.amount;
        ledger.applications += 1;
    }
}

impl From<BonusV1> for Bonus {
    fn from(old: BonusV1) -> Self {
        Self { amount: old.amount }
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

impl TryFrom<NotedV1> for Noted {
    type Error = Unreadable;

    fn try_from(historical: NotedV1) -> Result<Self, Self::Error> {
        if historical.text.is_empty() {
            return Err(Unreadable);
        }

        Ok(Self(historical.text))
    }
}

impl fmt::Display for Unreadable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a note without text is not a note")
    }
}

impl std::error::Error for Unreadable {}

impl Application for Bank {
    type State = Ledger;
    type Transaction = Change;
    type Record = Fact;
    type Streams = ();
    type Resources = ();
}

impl Wire for Bank {
    fn identify(value: &Recorded<Self>) -> (DefinitionId, u32) {
        match value {
            Recorded::Transaction(change) => change.identify(),
            Recorded::Record(fact) => fact.identify(),
            Recorded::System(_) => unreachable!("system facts are encoded by the adapter"),
        }
    }

    fn encode<S: Serializer>(value: &Recorded<Self>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Recorded::Transaction(change) => change.encode_with(serializer),
            Recorded::Record(fact) => fact.encode_with(serializer),
            Recorded::System(_) => unreachable!("system facts are encoded by the adapter"),
        }
    }

    fn decode<'de, D: Deserializer<'de> + Copy>(
        kind: Kind,
        id: &DefinitionId,
        version: u32,
        payload: D,
    ) -> Result<Option<Payload<Self>>, D::Error> {
        Ok(match kind {
            Kind::Transaction => {
                Change::restore(id, version, payload)?.map(|(change, apply)| Payload {
                    prepared: None,
                    id: id.clone(),
                    version,
                    value: Recorded::Transaction(Arc::new(change)),
                    apply,
                })
            }
            Kind::Record => Fact::restore(id, version, payload)?.map(|fact| Payload {
                prepared: None,
                id: id.clone(),
                version,
                value: Recorded::Record(fact),
                apply: None,
            }),
        })
    }
}

impl Registration<Bank> for Definitions {
    fn register(self) -> Catalog<Bank> {
        fn decode(
            _id: &DefinitionId,
            _version: u32,
            _bytes: &[u8],
        ) -> Result<Option<Change>, DecodeError> {
            Ok(None)
        }

        Catalog::Transactions {
            definitions: self.0,
            decode,
            inverses: Some(Change::__harmos_inverses),
        }
    }
}

/* === Public Functions === */
pub fn catalog() -> impl Registration<Bank> {
    Definitions(
        definitions_of(Change::catalog::<Bank>())
            .into_iter()
            .chain(definitions_of(Fact::catalog::<Bank>()))
            .collect(),
    )
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

pub async fn booted(
    origin: Ledger,
    attach: impl FnOnce(Builder<Bank>) -> Builder<Bank>,
) -> Runtime<Bank> {
    opened(origin, attach).await.expect("the copy replays")
}

pub async fn opened(
    origin: Ledger,
    attach: impl FnOnce(Builder<Bank>) -> Builder<Bank>,
) -> Result<Runtime<Bank>, Incompatible> {
    attach(
        Runtime::<Bank>::builder(origin, ())
            .register(Change::catalog())
            .register(Fact::catalog()),
    )
    .runtime()
    .await
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
        Catalog::Streams(_) | Catalog::Lanes(_) => {
            unreachable!("a transaction or fact catalog always registers as `Transactions`")
        }
    }
}

/* === Tests === */
