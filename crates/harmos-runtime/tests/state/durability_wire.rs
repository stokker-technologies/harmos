/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod other {
    use std::sync::Arc;

    use harmos_runtime::{Application, DefinitionId, state::Kind, state::Recorded, storage::Payload};
    use serde::{Deserializer, Serializer};

    use super::super::support::journal::Wire;
    use super::super::support::{Deposit, Explode, Ledger, Noted, Pay, Refusal, Withdraw};

    #[harmos_runtime::transactions(state = Ledger, error = Refusal)]
    #[derive(Clone, Debug)]
    pub enum Movement {
        Paid(Pay),
        Blew(Explode),
        Took(Withdraw),
        Put(Deposit),
    }

    #[harmos_runtime::records]
    #[derive(Clone, Debug)]
    pub enum Observed {
        Remark(Noted),
    }

    #[derive(Clone, Copy, Debug)]
    pub struct Branch;

    #[harmos_runtime::lanes]
    pub enum BranchLanes {
        #[default]
        Open,
    }

    impl Application for Branch {
        type Lanes = BranchLanes;
        type State = Ledger;
        type Transaction = Movement;
        type Record = Observed;
        type Streams = ();
        type Resources = ();
    }

    impl Wire for Branch {
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
                Kind::Transaction => Movement::restore(id, version, payload)?.map(|(change, apply)| Payload {
                    prepared: None,
                    id: id.clone(),
                    version,
                    value: Recorded::Transaction(Arc::new(change)),
                    apply,
                }),
                Kind::Record => Observed::restore(id, version, payload)?.map(|fact| Payload {
                    prepared: None,
                    id: id.clone(),
                    version,
                    value: Recorded::Record(fact),
                    apply: None,
                }),
            })
        }
    }
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use support::{Bank, Ledger, Noted, booted, deposit, ledger, withdraw};
// Local Imports
use super::support;
use harmos_runtime::{DefinitionId, Runtime, state::Kind, state::Position, state::Recorded, state::Transaction};
use support::journal::{Journal, Store, Wire};

/* === Constants === */

const VARIANTS: [&str; 9] = ["Deposit", "Withdraw", "Pay", "Explode", "Noted", "Paid", "Took", "Put", "Remark"];

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn payload_own_shape() {
    let copy = Store::default();

    let runtime = booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(&copy))).await;
    let deposited = runtime.state.commit(deposit(7)).await.expect("accepted");
    let noted = runtime.records.publish(Noted("audited".to_owned())).await.expect("a fact cannot be refused");
    runtime.state.wait_until_stored(noted.position).await;
    runtime.stop().await.unwrap();

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
    assert_eq!(lines[0].get("version"), Some(&serde_json::json!(1)));
    assert_eq!(lines[0].get("kind"), Some(&serde_json::json!("Transaction")));
    assert_eq!(lines[0].get("position"), Some(&serde_json::json!(deposited.position.0)));
}

#[tokio::test]
async fn another_build_reads() {
    let copy = Store::default();

    let runtime = booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(&copy))).await;
    runtime.state.commit(deposit(10)).await.expect("accepted");
    runtime.state.commit(withdraw(4)).await.expect("accepted");
    runtime.records.publish(Noted("audited".to_owned())).await.expect("a fact cannot be refused");
    let expected = ledger(&runtime).await;
    let frontier = runtime.state.applied();
    runtime.stop().await.unwrap();

    // The other build's catalogs share not one variant name or position with
    // the ones that wrote this file, and share every definition.
    let recovered = Runtime::<other::Branch>::builder(Ledger::default(), ())
        .register(other::Movement::catalog())
        .register(other::Observed::catalog())
        .recover(Journal::<other::Branch>::open(&copy))
        .runtime()
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
    let runtime = Runtime::<other::Branch>::builder(Ledger::default(), ())
        .register(other::Movement::catalog())
        .register(other::Observed::catalog())
        .runtime()
        .await
        .expect("the generated catalogs freeze");
    let id = DefinitionId::new("ledger.deposit");
    let value = serde_json::json!({ "amount": 7 });
    let bytes = serde_json::to_vec(&value).expect("the envelope payload is JSON");

    let live = runtime.state.decode_transaction(&id, 1, &bytes).expect("the registered restore decodes live bytes");
    let recovered = <other::Branch as Wire>::decode(Kind::Transaction, &id, 1, &value)
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

/* === Tests === */
