/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod source_scenarios;

#[path = "../support/mod.rs"]
mod support;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use harmos::{Application, Runtime, state::Position, state::Recorded, state::Records, state::Transaction};
use schematic_storage::{Jsonl, Wire};
use support::scratch;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Refusal = &'static str;

/* === Structs === */

#[derive(Default)]
struct State {
    balance: i64,
    applications: usize,
}

#[harmos::record(id = "atomic.observed", version = 2, history(ObservedV1))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Observed {
    before: i64,
    amount: i64,
    ordinal: usize,
}

#[harmos::record(id = "atomic.foreign", version = 1)]
#[derive(Serialize, Deserialize)]
struct Foreign;

#[harmos::record(id = "harmos:sidecar/my.sidecar-2/result", version = 1)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Reserved;

#[harmos::record(id = "atomic.broken", version = 1)]
#[derive(Deserialize)]
struct Broken;

#[harmos::transaction(id = "atomic.add", version = 2, inverse = Self, history(AddV1))]
#[derive(Clone, Debug, Deserialize)]
struct Add {
    amount: i64,
    count: usize,
    mode: u8,

    #[serde(skip)]
    encodings: Arc<AtomicUsize>,
}

struct Model;

#[harmos::transaction(version = 1)]
#[derive(Clone, Serialize, Deserialize)]
struct AddV1 {
    amount: i64,
}

#[harmos::record(id = "atomic.large", version = 1)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Large(String);

#[harmos::record(version = 1)]
#[derive(Serialize, Deserialize)]
struct ObservedV1 {
    before: i64,
    amount: i64,
    ordinal: usize,
}

#[harmos::record(id = "atomic.observed", version = 1)]
#[derive(Serialize, Deserialize)]
struct Obsolete {
    before: i64,
    amount: i64,
    ordinal: usize,
}

/* === Enums === */

#[harmos::transactions(state = State, error = Refusal)]
#[derive(Clone, Debug)]
enum Changes {
    Add(Add),
}

#[harmos::records]
#[derive(Clone, Debug, PartialEq, Eq)]
enum Facts {
    Observed(Observed),
    Large(Large),
    Reserved(Reserved),
}

/* === Traits === */

/* === Implementations === */

impl Serialize for Broken {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("record cannot encode"))
    }
}

impl Serialize for Add {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let invocation = self.encodings.fetch_add(1, Ordering::SeqCst);
        if self.mode == 1 && invocation > 0 {
            return Err(serde::ser::Error::custom("second serialization"));
        }
        if self.mode == 2 {
            return Err(serde::ser::Error::custom("first serialization"));
        }
        let mut s = serializer.serialize_struct("Add", 3)?;
        s.serialize_field("amount", &self.amount)?;
        s.serialize_field("count", &self.count)?;
        s.serialize_field("mode", &self.mode)?;
        s.end()
    }
}

impl Transaction<State> for Add {
    type Error = &'static str;
    fn check(&self, _: &State) -> Result<(), Self::Error> {
        if self.mode == 3 { Err("check refused") } else { Ok(()) }
    }
    fn records(&self, state: &State, out: &mut Records) {
        if self.mode == 16 {
            let _ = out.push(Reserved);
        }
        if self.mode >= 10 {
            let owner = match self.mode {
                11 => "Bad",
                12 => "a/b",
                13 => "",
                _ => "my.sidecar-2",
            };
            let _ = out.push_system(harmos::state::SystemFact {
                source: harmos::state::FactSource::Sidecar(owner.to_owned()),
                name: if self.mode == 14 { "" } else { "result/sample" }.to_owned(),
                version: 37,
                payload: if self.mode == 15 { vec![255; Records::MAX_BYTES] } else { vec![0, 255, 17] },
            });
        }
        if self.mode == 6 {
            let _ = out.push(Large("x".repeat(Records::MAX_BYTES)));
        }
        if self.mode == 7 {
            for _ in 0..2 {
                let _ = out.push(Large("x".repeat(Records::MAX_BYTES / 2)));
            }
        }
        if self.mode == 9 {
            let _ = out.push(Obsolete { before: state.balance, amount: self.amount, ordinal: 0 });
        }
        if self.mode == 4 {
            let _ = out.push(Broken);
        }
        if self.mode == 5 {
            let _ = out.push(Foreign);
        }
        for ordinal in 0..self.count {
            let _ = out.push(Observed { before: state.balance, amount: self.amount, ordinal });
        }
    }
    fn apply(&self, state: &mut State) {
        state.balance += self.amount;
        state.applications += 1;
    }
}

impl harmos::state::Invert<State> for Add {
    fn invert(&self, _: &State) -> Self {
        add(-self.amount, self.count, 0)
    }
}

impl Application for Model {
    type State = State;
    type Transaction = Changes;
    type Record = Facts;
    type Streams = ();
    type Resources = ();
}

impl Wire for Model {
    fn identify(v: &Recorded<Self>) -> (harmos::DefinitionId, u32) {
        match v {
            Recorded::Transaction(v) => v.identify(),
            Recorded::Record(v) => v.identify(),
            Recorded::System(v) => v.identify(),
        }
    }
    fn encode<S: serde::Serializer>(v: &Recorded<Self>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Recorded::Transaction(v) => v.encode_with(s),
            Recorded::Record(v) => v.encode_with(s),
            Recorded::System(_) => unreachable!(),
        }
    }
    fn decode<'de, D: serde::Deserializer<'de> + Copy>(
        kind: harmos::state::Kind, id: &harmos::DefinitionId, version: u32, payload: D,
    ) -> Result<Option<harmos::storage::Payload<Self>>, D::Error> {
        Ok(match kind {
            harmos::state::Kind::Transaction => Changes::restore(id, version, payload)?.map(|(value, apply)| harmos::storage::Payload {
                prepared: None,
                id: id.clone(),
                version,
                value: Recorded::Transaction(harmos::operations::Response::new(value)),
                apply,
            }),
            harmos::state::Kind::Record => Facts::restore(id, version, payload)?.map(|value| harmos::storage::Payload {
                prepared: None,
                id: id.clone(),
                version,
                value: Recorded::Record(value),
                apply: None,
            }),
        })
    }
}

impl Transaction<State> for AddV1 {
    type Error = Refusal;
    fn check(&self, _: &State) -> Result<(), Self::Error> {
        panic!("historical check must not run")
    }
    fn records(&self, _: &State, _: &mut Records) {
        panic!("historical records must not run")
    }
    fn apply(&self, state: &mut State) {
        state.balance += 2 * self.amount;
        state.applications += 1;
    }
}

impl From<AddV1> for Add {
    fn from(old: AddV1) -> Self {
        add(old.amount, 1, 0)
    }
}

impl From<ObservedV1> for Observed {
    fn from(old: ObservedV1) -> Self {
        Self { before: old.before, amount: old.amount, ordinal: old.ordinal }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn add(amount: i64, count: usize, mode: u8) -> Add {
    Add { amount, count, mode, encodings: Arc::default() }
}

async fn boot(path: &std::path::Path, tail: usize) -> Runtime<Model> {
    Runtime::<Model>::builder(State::default(), ())
        .register(Changes::catalog())
        .register(Facts::catalog())
        .recover(Jsonl::<Model>::open(path).unwrap())
        .tail(tail)
        .runtime()
        .await
        .unwrap()
}

#[test]
fn contract_scenario() {
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {
                use harmos::storage::Source;
                let directory = scratch("atomic-truncation");
                let path = directory.join("journal.jsonl");
                let runtime = boot(&path, 16).await;
                runtime.state.commit(add(3, 2, 0)).await.unwrap();
                runtime.stop().await.unwrap();
                let bytes = std::fs::read(&path).unwrap();
                let header = bytes.iter().position(|byte| *byte == b'\n').unwrap() + 1;
                for cut in header..=bytes.len() {
                    let truncated = directory.join("truncated.jsonl");
                    std::fs::write(&truncated, &bytes[..cut]).unwrap();
                    let mut source = Jsonl::<Model>::open(&truncated).unwrap_or_else(|e| panic!("cut {cut}: {e}"));
                    let recovered = source.load().await.unwrap();
                    let complete = cut == bytes.len();
                    assert_eq!(recovered.frontier, if complete { Position(3) } else { Position::ORIGIN }, "cut {cut}");
                    assert_eq!(recovered.entries.len(), if complete { 3 } else { 0 }, "cut {cut}");
                    assert_eq!(std::fs::read(&truncated).unwrap(), bytes[..cut], "reading must not repair the file");
                }
                let mut corrupt = bytes.clone();
                corrupt[header + 10] = b'!';
                std::fs::write(&path, corrupt).unwrap();
                assert!(Jsonl::<Model>::open(&path).is_err(), "terminated corruption is loud");
            });
        }
    }
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {

    use harmos::storage::Source;
    let directory = scratch("atomic-v2-migration");
    let path = directory.join("journal.jsonl");
    let legacy = concat!(
        "{\"schema\":2,\"generation\":0,\"origin\":0}\n",
        "{ \"position\":1,\"kind\":\"Transaction\",\"id\":\"atomic.add\",\"version\":1,\"metadata\":{\"principal\":null,\"correlation\":1,\"causation\":null,\"request\":null,\"link\":null,\"at_us\":0},\"payload\":{\"amount\":5}}"
    );
    std::fs::write(&path, legacy).unwrap();
    let mut source = Jsonl::<Model>::open(&path).unwrap();
    assert_eq!(source.load().await.unwrap().frontier, Position(1));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), legacy);
    drop(source);
    let runtime = boot(&path, 1).await;
    let written = runtime.state.commit(add(3, 1, 0)).await.unwrap();
    runtime.state.wait_until_stored(written.frontier).await;
    runtime.stop().await.unwrap();
    let migrated = std::fs::read_to_string(&path).unwrap();
    assert!(migrated.starts_with("{\"schema\":3,"));
    assert!(migrated.contains(legacy.split_once('\n').unwrap().1));
    let restarted = boot(&path, 1).await;
    assert_eq!(
        restarted
            .state
            .state()
            .read(|s| (s.balance, s.applications))
            .await
            .unwrap()
            .value,
        (13, 2)
    );
    restarted.stop().await.unwrap();

});
        }
    }
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {
                let directory = scratch("atomic-repair-append");
                let path = directory.join("journal.jsonl");
                let runtime = boot(&path, 1).await;
                runtime.state.commit(add(4, 1, 0)).await.unwrap();
                runtime.stop().await.unwrap();
                let bytes = std::fs::read(&path).unwrap();
                let mut partial = bytes.clone();
                partial.extend_from_slice(b"{\"group\":1,\"first\":3,\"frontier\":4,\"entries\":[{");
                std::fs::write(&path, &partial).unwrap();
                let runtime = boot(&path, 1).await;
                assert_eq!(runtime.state.applied(), Position(2));
                let receipt = runtime.state.commit(add(2, 1, 0)).await.unwrap();
                assert_eq!(receipt.frontier, Position(4));
                runtime.state.wait_until_stored(receipt.frontier).await;
                runtime.stop().await.unwrap();
                assert!(std::fs::read(&path).unwrap().starts_with(&bytes));
                let runtime = boot(&path, 1).await;
                assert_eq!(runtime.state.state().read(|s| (s.balance, s.applications)).await.unwrap().value, (6, 2));
                runtime.stop().await.unwrap();
            });
        }
    }
}

/* === Tests === */
