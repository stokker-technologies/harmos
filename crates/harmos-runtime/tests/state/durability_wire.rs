/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use support::journal::{Journal, Store, Wire};
use support::{Bank, Ledger, Noted, booted, deposit, ledger, withdraw};

// Local Imports
use super::support;
use harmos_runtime::{DefinitionId, state::Kind, state::Position, state::Recorded, state::Transaction};

/* === Constants === */

const VARIANTS: [&str; 9] = ["Deposit", "Withdraw", "Pay", "Explode", "Noted", "Paid", "Took", "Put", "Remark"];

/* === Private Functions === */

#[tokio::test]
async fn payload_own_shape() {
    let copy = Store::default();

    let runtime = booted(Ledger::default(), |setup| {
        setup.settings().recover(Journal::<Bank>::open(&copy));
    })
    .await;
    let deposited = runtime.state.commit(deposit(7)).await.expect("accepted");
    let noted = runtime.records.publish(Noted("audited".to_owned())).await.expect("a fact cannot be refused");
    runtime.state.wait_until_stored(noted.position).await;
    crate::support::app::shutdown(&runtime).await.unwrap();

    let lines = copy.members();

    assert_eq!(
        lines[0].get("payload"),
        Some(&serde_json::json!({ "amount": 7 })),
        "the payload is what the definition's own serde derive writes"
    );
    assert_eq!(lines[1].get("payload"), Some(&serde_json::json!("audited")), "a fact is its own shape too, whatever shape that is");
    for line in &lines {
        let keys = line["payload"].as_object().into_iter().flat_map(|payload| payload.keys().map(String::as_str).collect::<Vec<_>>());

        for key in keys {
            assert!(!VARIANTS.contains(&key), "no catalog variant name reaches the wire: {line}");
        }
    }

    // What names the payload is the pair beside it, and only that pair.
    assert_eq!(lines[0].get("id").and_then(serde_json::Value::as_str), Some("ledger.deposit"));
    assert_eq!(lines[0].get("version"), Some(&serde_json::json!(super::support::Deposit::ENTRY.definition().1)));
    assert_eq!(lines[0].get("kind"), Some(&serde_json::json!("Transaction")));
    assert_eq!(lines[0].get("position"), Some(&serde_json::json!(deposited.position.0)));
}

#[tokio::test]
async fn another_build_reads() {
    let copy = Store::default();

    let runtime = booted(Ledger::default(), |setup| {
        setup.settings().recover(Journal::<Bank>::open(&copy));
    })
    .await;
    runtime.state.commit(deposit(10)).await.expect("accepted");
    runtime.state.commit(withdraw(4)).await.expect("accepted");
    runtime.records.publish(Noted("audited".to_owned())).await.expect("a fact cannot be refused");
    let expected = ledger(&runtime).await;
    let frontier = runtime.state.applied();
    crate::support::app::shutdown(&runtime).await.unwrap();

    // The other build's catalogs share not one variant name or position with
    // the ones that wrote this file, and share every definition.
    let recovered = crate::support::app::launch::<other::Branch>(Ledger::default(), (), |setup| {
        setup.settings().register(other::movement_state_catalog());
        setup.settings().register(other::observed_registry());
        setup.settings().recover(Journal::<other::Branch>::open(&copy));
    })
    .await
    .expect("every definition is declared, under the names the file carries");

    assert_eq!(
        recovered.state.state().read(Clone::clone).await.expect("reads are open").value,
        expected,
        "the stored pair `(id, version)` is the whole of what a payload is read by"
    );
    assert_eq!(recovered.state.applied(), frontier);
    assert_ne!(frontier, Position::ORIGIN);
}

#[tokio::test]
async fn decode_matches_recovery() {
    let runtime = crate::support::app::launch::<other::Branch>(Ledger::default(), (), |setup| {
        setup.settings().register(other::movement_state_catalog());
        setup.settings().register(other::observed_registry());
    })
    .await
    .expect("the generated catalogs freeze");
    let id = DefinitionId::new("ledger.deposit");
    let value = serde_json::json!({ "amount": 7 });
    let bytes = serde_json::to_vec(&value).expect("the envelope payload is JSON");

    let live = runtime
        .state
        .decode_transaction(&id, super::support::Deposit::ENTRY.definition().1, &bytes)
        .expect("the registered restore decodes live bytes");
    let recovered = <other::Branch as Wire>::decode(Kind::Transaction, &id, super::support::Deposit::ENTRY.definition().1, &value)
        .expect("the recovery decoder accepts the same payload")
        .expect("the recovery catalog declares the pair");
    let Recorded::Transaction(recovered) = recovered.value else {
        panic!("a transaction envelope decodes as a transaction");
    };
    let mut live_state = Ledger::default();
    let mut recovery_state = Ledger::default();

    live.apply(&mut live_state);
    recovered.apply(&mut recovery_state);

    assert_eq!(live_state, recovery_state, "the same envelope payload reaches the same generated restore path");
}

mod other {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports
    use serde::{Deserializer, Serializer};

    // Local Imports
    use super::super::support::journal::Wire;
    use super::super::support::{Deposit, Explode, Ledger, Noted, Pay, Refusal, Withdraw};
    use harmos_runtime::{Application, DefinitionId, state::Kind, state::Recorded, storage::Payload};

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    #[derive(Clone, Copy, Debug)]
    pub struct Branch;

    /* === Enums === */

    #[derive(Clone, Debug)]
    pub enum Movement {
        Paid(Pay),
        Blew(Explode),
        Took(Withdraw),
        Put(Deposit),
    }
    impl From<Pay> for Movement {
        fn from(value: Pay) -> Self {
            Self::Paid(value)
        }
    }
    impl From<Explode> for Movement {
        fn from(value: Explode) -> Self {
            Self::Blew(value)
        }
    }
    impl From<Withdraw> for Movement {
        fn from(value: Withdraw) -> Self {
            Self::Took(value)
        }
    }
    impl From<Deposit> for Movement {
        fn from(value: Deposit) -> Self {
            Self::Put(value)
        }
    }
    impl harmos_runtime::state::Transaction<Ledger> for Movement {
        type Error = Refusal;
        fn check(&self, state: &Ledger) -> std::result::Result<(), Self::Error> {
            match self {
                Self::Paid(value) => harmos_runtime::state::Transaction::check(value, state),
                Self::Blew(value) => harmos_runtime::state::Transaction::check(value, state),
                Self::Took(value) => harmos_runtime::state::Transaction::check(value, state),
                Self::Put(value) => harmos_runtime::state::Transaction::check(value, state),
            }
        }
        fn apply(&self, state: &mut Ledger) {
            match self {
                Self::Paid(value) => harmos_runtime::state::Transaction::apply(value, state),
                Self::Blew(value) => harmos_runtime::state::Transaction::apply(value, state),
                Self::Took(value) => harmos_runtime::state::Transaction::apply(value, state),
                Self::Put(value) => harmos_runtime::state::Transaction::apply(value, state),
            }
        }
        fn records(&self, state: &Ledger, records: &mut harmos_runtime::state::Records) {
            match self {
                Self::Paid(value) => harmos_runtime::state::Transaction::records(value, state, records),
                Self::Blew(value) => harmos_runtime::state::Transaction::records(value, state, records),
                Self::Took(value) => harmos_runtime::state::Transaction::records(value, state, records),
                Self::Put(value) => harmos_runtime::state::Transaction::records(value, state, records),
            }
        }
    }
    impl harmos_runtime::state::CatalogCodec for Movement {
        fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
            match self {
                Self::Paid(_) => Pay::ENTRY.definition(),
                Self::Blew(_) => Explode::ENTRY.definition(),
                Self::Took(_) => Withdraw::ENTRY.definition(),
                Self::Put(_) => Deposit::ENTRY.definition(),
            }
        }
        fn encode<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
            match self {
                Self::Paid(value) => serde::Serialize::serialize(value, serializer),
                Self::Blew(value) => serde::Serialize::serialize(value, serializer),
                Self::Took(value) => serde::Serialize::serialize(value, serializer),
                Self::Put(value) => serde::Serialize::serialize(value, serializer),
            }
        }
    }
    pub fn movement_state_catalog() -> harmos_runtime::state::StateCatalog<Ledger, Movement> {
        harmos_runtime::state::StateCatalog::new()
            .transaction(Pay::ENTRY, Movement::Paid, |value| match value {
                Movement::Paid(value) => Some(value),
                _ => None,
            })
            .transaction(Explode::ENTRY, Movement::Blew, |value| match value {
                Movement::Blew(value) => Some(value),
                _ => None,
            })
            .transaction(Withdraw::ENTRY, Movement::Took, |value| match value {
                Movement::Took(value) => Some(value),
                _ => None,
            })
            .transaction(Deposit::ENTRY, Movement::Put, |value| match value {
                Movement::Put(value) => Some(value),
                _ => None,
            })
    }

    #[derive(Clone, Debug)]
    pub enum Observed {
        Remark(Noted),
    }
    impl From<Noted> for Observed {
        fn from(value: Noted) -> Self {
            Self::Remark(value)
        }
    }
    impl harmos_runtime::state::CatalogCodec for Observed {
        fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
            match self {
                Self::Remark(_) => <Noted as harmos_runtime::state::Record>::ENTRY.definition(),
            }
        }
        fn encode<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
            match self {
                Self::Remark(value) => serde::Serialize::serialize(value, serializer),
            }
        }
    }
    pub fn observed_registry() -> harmos_runtime::state::RecordRegistry<Observed> {
        harmos_runtime::state::RecordRegistry::new().entry(<Noted as harmos_runtime::state::Record>::ENTRY, Observed::Remark, |value| {
            let Observed::Remark(value) = value;
            Some(value)
        })
    }

    /* === Traits === */

    /* === Implementations === */

    impl harmos_runtime::Lifecycle for Branch {
        type Config = ();
        type Error = std::convert::Infallible;
        type Context = harmos_runtime::Startup<Self>;

        async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
            Ok(Self)
        }
    }

    impl Application for Branch {
        type State = Ledger;
        type Transaction = Movement;
        type Record = Observed;
    }

    impl Wire for Branch {
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
                Kind::Transaction => movement_state_catalog().restore(id, version, payload)?.map(|change| Payload {
                    prepared: None,
                    id: id.clone(),
                    version,
                    value: Recorded::Transaction(harmos_runtime::operations::Response::new(change)),
                }),
                Kind::Record => observed_registry().restore(id, version, payload)?.map(|fact| Payload {
                    prepared: None,
                    id: id.clone(),
                    version,
                    value: Recorded::Record(fact),
                }),
            })
        }
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}
