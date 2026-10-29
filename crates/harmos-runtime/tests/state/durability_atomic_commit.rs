/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

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
use super::support::journal::{Journal, Snapshot, Store, Wire};
use harmos_runtime::{
    Application, Error, Runtime, state::Focus, state::Origin, state::Position, state::PrincipalId, state::Recorded, state::Records,
    state::RequestKey, state::Transaction,
};

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

#[harmos_runtime::record(id = "atomic.observed")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Observed {
    before: i64,
    amount: i64,
    ordinal: usize,
}

#[harmos_runtime::record(id = "atomic.foreign")]
#[derive(Serialize, Deserialize)]
struct Foreign;

#[harmos_runtime::record(id = "harmos:sidecar/my_sidecar_2/result")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Reserved;

#[harmos_runtime::record(id = "atomic.broken")]
#[derive(Deserialize)]
struct Broken;

#[harmos_runtime::transaction(id = "atomic.add",  inverse = Self,)]
#[derive(Clone, Debug, Deserialize)]
struct Add {
    amount: i64,
    count: usize,
    mode: u8,

    #[serde(skip)]
    encodings: Arc<AtomicUsize>,
}

struct Model;

#[harmos_runtime::record(id = "atomic.large")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Large(String);

/* === Enums === */

#[harmos_runtime::transactions(state = State, error = Refusal)]
#[derive(Clone, Debug)]
enum Changes {
    Add(Add),
}

#[harmos_runtime::records]
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
                _ => "my_sidecar_2",
            };
            let _ = out.push_system(harmos_runtime::state::SystemFact {
                source: harmos_runtime::state::FactSource::Sidecar(owner.to_owned()),
                name: if self.mode == 14 { "" } else { "result/sample" }.to_owned(),
                version: 1,
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
        if self.mode == 4 {
            let _ = out.push(Broken);
        }
        if self.mode == 5 {
            let _ = out.push(Foreign);
        }
        for ordinal in 0..self.count {
            let _ = out.push(Observed {
                before: state.balance,
                amount: self.amount,
                ordinal,
            });
        }
    }
    fn apply(&self, state: &mut State) {
        state.balance += self.amount;
        state.applications += 1;
    }
}

impl harmos_runtime::state::Invertible<State> for Add {
    type Inverse = Self;

    fn inverse(&self, _: &State) -> Self::Inverse {
        add(-self.amount, self.count, 0)
    }
}

#[harmos_runtime::lanes]
pub enum ModelLanes {
    #[default]
    Open,
}
impl harmos_runtime::Lifecycle for Model {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl Application for Model {
    type Lanes = ModelLanes;
    type State = State;
    type Transaction = Changes;
    type Record = Facts;
    type Streams = ();
}

impl Wire for Model {
    fn identify(v: &Recorded<Self>) -> (harmos_runtime::DefinitionId, u32) {
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
        kind: harmos_runtime::state::Kind, id: &harmos_runtime::DefinitionId, version: u32, payload: D,
    ) -> Result<Option<harmos_runtime::storage::Payload<Self>>, D::Error> {
        Ok(match kind {
            harmos_runtime::state::Kind::Transaction => {
                Changes::restore(id, version, payload)?.map(|value| harmos_runtime::storage::Payload {
                    prepared: None,
                    id: id.clone(),
                    version,
                    value: Recorded::Transaction(harmos_runtime::operations::Response::new(value)),
                })
            }
            harmos_runtime::state::Kind::Record => Facts::restore(id, version, payload)?.map(|value| harmos_runtime::storage::Payload {
                prepared: None,
                id: id.clone(),
                version,
                value: Recorded::Record(value),
            }),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn add(amount: i64, count: usize, mode: u8) -> Add {
    Add {
        amount,
        count,
        mode,
        encodings: Arc::default(),
    }
}

async fn boot(copy: &Store, tail: usize) -> Runtime<Model> {
    Runtime::<Model>::launch_with(State::default(), (), |setup| {
        setup.settings().register(Changes::catalog());
        setup.settings().register(Facts::catalog());
        setup.settings().recover(Journal::<Model>::open(copy));
        setup.settings().tail(tail);
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn preparation_failure_inert() {
    let copy = Store::default();
    let runtime = boot(&copy, 16).await;
    assert!(matches!(runtime.records.publish(Reserved).await, Err(Error::Preparation(harmos_runtime::state::PreparationError::Identity))));
    for mode in [2, 3, 4, 5, 6, 7, 11, 12, 13, 14, 15, 16] {
        assert!(runtime.state.commit(add(7, 1, mode)).await.is_err());
        assert_eq!(runtime.state.state().read(|s| (s.balance, s.applications)).await.unwrap().value, (0, 0));
        assert_eq!(runtime.state.applied(), Position::ORIGIN);
    }
    assert!(matches!(
        runtime.state.commit(add(1, Records::MAX_RECORDS + 1, 0)).await,
        Err(Error::Preparation(harmos_runtime::state::PreparationError::Limit))
    ));
    runtime.shutdown().await.unwrap();
    assert!(copy.lines().is_empty());
}

#[tokio::test]
async fn restart_preserves_members() {
    let copy = Store::default();
    let runtime = boot(&copy, 1).await;
    let change = add(7, 2, 1);
    let calls = Arc::clone(&change.encodings);
    let key = RequestKey::new("once");
    let receipt = runtime.state.commit_keyed(key.clone(), change.clone()).await.unwrap();
    assert_eq!((receipt.position, receipt.frontier), (Position(1), Position(3)));
    assert_eq!(runtime.state.commit_keyed(key, change).await.unwrap(), receipt);
    runtime.state.wait_until_stored(receipt.frontier).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let facts = runtime.state.query::<Facts>(Position::ORIGIN).await.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(facts.len(), 2);
    assert_eq!(facts[0].position, Position(2));
    assert_eq!(facts[0].value, Facts::Observed(Observed { before: 0, amount: 7, ordinal: 0 }));
    assert!(matches!(runtime.state.state().at(Origin::initial(State::default()), Position(1)).await, Err(Error::IncompleteCommit)));
    runtime.shutdown().await.unwrap();
    let restarted = boot(&copy, 1).await;
    assert_eq!(restarted.state.state().read(|s| (s.balance, s.applications)).await.unwrap().value, (7, 1));
    assert_eq!(restarted.state.query::<Facts>(Position::ORIGIN).await.collect::<Result<Vec<_>, _>>().unwrap().len(), 2);
    restarted.shutdown().await.unwrap();
    assert_eq!(copy.lines().len(), 1);
}

#[tokio::test]
async fn reversals_emit_records() {
    let copy = Store::default();
    let runtime = boot(&copy, 1).await;
    let principal = PrincipalId::new("operator");
    let journal = runtime.state.as_principal(principal.clone());
    let initial = journal.commit(add(9, 1, 0)).await.unwrap();
    let undone = journal.undo(principal.clone(), Focus::all()).await.unwrap();
    let redone = journal.redo(principal, Focus::all()).await.unwrap();
    assert_eq!((initial.frontier, undone.frontier, redone.frontier), (Position(2), Position(4), Position(6)));
    journal.wait_until_stored(redone.frontier).await;
    let changes = journal.query::<Changes>(Position::ORIGIN).await.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(changes[1].metadata.link, Some(harmos_runtime::state::Link::UndoOf(initial.position)));
    assert_eq!(journal.state().read(|s| (s.balance, s.applications)).await.unwrap().value, (9, 3));
    drop(journal);
    runtime.shutdown().await.unwrap();
    let restarted = boot(&copy, 1).await;
    assert_eq!(restarted.state.state().read(|s| (s.balance, s.applications)).await.unwrap().value, (9, 3));
    assert_eq!(restarted.state.query::<Facts>(Position::ORIGIN).await.collect::<Result<Vec<_>, _>>().unwrap().len(), 3);
    restarted.shutdown().await.unwrap();
}

#[tokio::test]
async fn record_keeps_group() {
    let copy = Store::default();
    let mirror = Store::default();
    let live = boot(&copy, 1).await;
    live.state.commit(add(5, 1, 0)).await.unwrap();
    live.shutdown().await.unwrap();
    let original: Vec<String> = copy.lines().iter().map(|line| line.replace("atomic.observed", "atomic.future")).collect();
    copy.replace(original.clone());
    let snapshots = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed = Arc::clone(&snapshots);
    let snapshot = Snapshot::<Model>::folding(State::default(), Position::ORIGIN, move |state, position| {
        observed.lock().unwrap().push((state.balance, position));
        Ok(())
    });
    let runtime = Runtime::<Model>::launch_with(State::default(), (), |setup| {
        setup.settings().register(Changes::catalog());
        setup.settings().register(Facts::catalog());
        setup.settings().recover(Journal::<Model>::open(&copy));
        setup.settings().attach(Journal::<Model>::open(&mirror));
        setup.settings().snapshot(snapshot);
        setup.settings().tail(1);
    })
    .await
    .unwrap();
    assert_eq!(runtime.state.applied(), Position(2));
    assert!(matches!(runtime.state.state().at(Origin::initial(State::default()), Position(1)).await, Err(Error::IncompleteCommit)));
    assert_eq!(runtime.state.query::<Facts>(Position::ORIGIN).await.count(), 0);
    runtime.shutdown().await.unwrap();
    assert_eq!(*snapshots.lock().unwrap(), vec![(5, Position(2))]);
    assert_eq!(mirror.lines(), original);
}

#[tokio::test]
async fn facts_share_group() {
    let copy = Store::default();
    let runtime = boot(&copy, 1).await;
    let receipt = runtime.state.commit(add(5, 1, 10)).await.unwrap();
    assert_eq!((receipt.position, receipt.frontier), (Position(1), Position(3)));
    runtime.state.wait_until_stored(receipt.frontier).await;
    runtime.shutdown().await.unwrap();
    let runtime = boot(&copy, 1).await;
    let facts = runtime.state.query::<harmos_runtime::state::SystemFact>(Position::ORIGIN).await.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].position, Position(2));
    assert_eq!(
        facts[0].value,
        harmos_runtime::state::SystemFact {
            source: harmos_runtime::state::FactSource::Sidecar("my_sidecar_2".to_owned()),
            name: "result/sample".to_owned(),
            version: 1,
            payload: vec![0, 255, 17],
        }
    );
    assert_eq!(facts[0].value.identify().0.as_str(), "harmos:sidecar/my_sidecar_2/result/sample");
    let typed = runtime.state.query::<Facts>(Position::ORIGIN).await.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(typed.len(), 1);
    assert_eq!(typed[0].position, Position(3));
    assert_eq!(runtime.state.state().read(|s| (s.balance, s.applications)).await.unwrap().value, (5, 1));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn mixed_frontier_recovers() {
    let copy = Store::default();
    let runtime = boot(&copy, 1).await;
    let first = runtime.state.commit(add(5, 1, 10)).await.unwrap();
    runtime.state.wait_until_stored(first.frontier).await;
    let later = runtime.state.commit(add(8, 1, 10)).await.unwrap();
    runtime.state.wait_until_stored(later.frontier).await;
    let entries =
        runtime.state.query_through::<Recorded<Model>>(Position::ORIGIN, first.frontier).await.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(entries.iter().map(|entry| entry.position).collect::<Vec<_>>(), [Position(1), Position(2), Position(3)]);
    assert!(matches!(entries[0].value, Recorded::Transaction(_)));
    assert!(matches!(entries[1].value, Recorded::System(_)));
    assert!(matches!(entries[2].value, Recorded::Record(_)));
    runtime.shutdown().await.unwrap();
    let inspector = Runtime::<Model>::inspect(State::default(), Journal::<Model>::open(&copy)).await.unwrap();
    let read =
        inspector.state.query_through::<Recorded<Model>>(Position::ORIGIN, first.frontier).await.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(read.iter().map(|entry| entry.position).collect::<Vec<_>>(), [Position(1), Position(2), Position(3)]);
    assert!(matches!(
        inspector.state.query_through::<Recorded<Model>>(later.frontier, Position(later.frontier.0 + 1)).await.next(),
        Some(Err(Error::FrontierAhead))
    ));
}

#[tokio::test]
async fn contract_refuses_apply() {
    struct Capture(Arc<AtomicUsize>);
    impl harmos_runtime::state::JournalCapture<Model> for Capture {
        async fn capture(
            &mut self, _: Vec<Arc<harmos_runtime::state::Entry<Recorded<Model>>>>, _: Position,
        ) -> Result<(), harmos_runtime::state::CaptureError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    let captures = Arc::new(AtomicUsize::new(0));
    let runtime = Runtime::<Model>::launch_with(State::default(), (), |setup| {
        setup.settings().register(Changes::catalog());
        setup.settings().require_capture(
            harmos_runtime::state::CaptureLimits {
                pending_bytes: 4096,
                max_group_bytes: 4096,
            },
            Capture(Arc::clone(&captures)),
        );
    })
    .await
    .unwrap();
    let refused = runtime.state.commit(add(7, 1, 0)).await;
    assert!(matches!(refused, Err(Error::Preparation(harmos_runtime::state::PreparationError::Membership))));
    assert_eq!(runtime.state.applied(), Position::ORIGIN);
    let state = runtime.state.state().read(|state| (state.balance, state.applications)).await.unwrap();
    assert_eq!(state.value, (0, 0));
    assert_eq!(state.position, Position::ORIGIN);
    runtime.shutdown().await.unwrap();
    assert_eq!(captures.load(Ordering::SeqCst), 0);
}

/* === Tests === */
