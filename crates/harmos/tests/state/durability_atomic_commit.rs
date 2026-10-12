/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

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
use super::support::scratch;
use harmos::{
    Application, Error, Runtime, state::Focus, state::Origin, state::Position, state::PrincipalId,
    state::Recorded, state::Records, state::RequestKey, state::Transaction,
};
use jsonl_adapter::{Jsonl, Wire};

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
        if self.mode == 3 {
            Err("check refused")
        } else {
            Ok(())
        }
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
                payload: if self.mode == 15 {
                    vec![255; Records::MAX_BYTES]
                } else {
                    vec![0, 255, 17]
                },
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
            let _ = out.push(Obsolete {
                before: state.balance,
                amount: self.amount,
                ordinal: 0,
            });
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
        kind: harmos::state::Kind,
        id: &harmos::DefinitionId,
        version: u32,
        payload: D,
    ) -> Result<Option<harmos::storage::Payload<Self>>, D::Error> {
        Ok(match kind {
            harmos::state::Kind::Transaction => {
                Changes::restore(id, version, payload)?.map(|(value, apply)| {
                    harmos::storage::Payload {
                        prepared: None,
                        id: id.clone(),
                        version,
                        value: Recorded::Transaction(Arc::new(value)),
                        apply,
                    }
                })
            }
            harmos::state::Kind::Record => {
                Facts::restore(id, version, payload)?.map(|value| harmos::storage::Payload {
                    prepared: None,
                    id: id.clone(),
                    version,
                    value: Recorded::Record(value),
                    apply: None,
                })
            }
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
        Self {
            before: old.before,
            amount: old.amount,
            ordinal: old.ordinal,
        }
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

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("preparation_ignored_nothing", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let path = scratch("atomic-refusal").join("journal.jsonl");
                    let runtime = boot(&path, 16).await;
                    assert!(matches!(
                        runtime.records.publish(Reserved).await,
                        Err(Error::Preparation(
                            harmos::state::PreparationError::Identity
                        ))
                    ));
                    for mode in [2, 3, 4, 5, 6, 7, 9, 11, 12, 13, 14, 15, 16] {
                        assert!(runtime.state.commit(add(7, 1, mode)).await.is_err());
                        assert_eq!(
                            runtime
                                .state
                                .state()
                                .read(|s| (s.balance, s.applications))
                                .await
                                .unwrap()
                                .value,
                            (0, 0)
                        );
                        assert_eq!(runtime.state.applied(), Position::ORIGIN);
                    }
                    assert!(matches!(
                        runtime
                            .state
                            .commit(add(1, Records::MAX_RECORDS + 1, 0))
                            .await,
                        Err(Error::Preparation(harmos::state::PreparationError::Limit))
                    ));
                    runtime.stop().await.unwrap();
                    assert_eq!(std::fs::read_to_string(path).unwrap().lines().count(), 1);
                });
        });
    }
    {
        scenario_runtime::check("restart_preserves_members", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let path = scratch("atomic-restart").join("journal.jsonl");
                    let runtime = boot(&path, 1).await;
                    let change = add(7, 2, 1);
                    let calls = Arc::clone(&change.encodings);
                    let key = RequestKey::new("once");
                    let receipt = runtime
                        .state
                        .commit_keyed(key.clone(), change.clone())
                        .await
                        .unwrap();
                    assert_eq!(
                        (receipt.position, receipt.frontier),
                        (Position(1), Position(3))
                    );
                    assert_eq!(
                        runtime.state.commit_keyed(key, change).await.unwrap(),
                        receipt
                    );
                    runtime.state.wait_until_stored(receipt.frontier).await;
                    assert_eq!(calls.load(Ordering::SeqCst), 1);
                    let facts = runtime
                        .state
                        .query::<Facts>(Position::ORIGIN)
                        .await
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap();
                    assert_eq!(facts.len(), 2);
                    assert_eq!(facts[0].position, Position(2));
                    assert_eq!(
                        facts[0].value,
                        Facts::Observed(Observed {
                            before: 0,
                            amount: 7,
                            ordinal: 0
                        })
                    );
                    assert!(matches!(
                        runtime
                            .state
                            .state()
                            .at(Origin::initial(State::default()), Position(1))
                            .await,
                        Err(Error::IncompleteCommit)
                    ));
                    runtime.stop().await.unwrap();
                    let restarted = boot(&path, 1).await;
                    assert_eq!(
                        restarted
                            .state
                            .state()
                            .read(|s| (s.balance, s.applications))
                            .await
                            .unwrap()
                            .value,
                        (7, 1)
                    );
                    assert_eq!(
                        restarted
                            .state
                            .query::<Facts>(Position::ORIGIN)
                            .await
                            .collect::<Result<Vec<_>, _>>()
                            .unwrap()
                            .len(),
                        2
                    );
                    restarted.stop().await.unwrap();
                    let text = std::fs::read_to_string(path).unwrap();
                    assert!(text.starts_with("{\"schema\":3,"));
                    assert_eq!(text.lines().count(), 2);
                });
        });
    }
    {
        scenario_runtime::check("undo_redo_emit", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let path = scratch("atomic-revert").join("journal.jsonl");
                    let runtime = boot(&path, 1).await;
                    let principal = PrincipalId::new("operator");
                    let journal = runtime.state.as_principal(principal.clone());
                    let initial = journal.commit(add(9, 1, 0)).await.unwrap();
                    let undone = journal.undo(principal.clone(), Focus::all()).await.unwrap();
                    let redone = journal.redo(principal, Focus::all()).await.unwrap();
                    assert_eq!(
                        (initial.frontier, undone.frontier, redone.frontier),
                        (Position(2), Position(4), Position(6))
                    );
                    journal.wait_until_stored(redone.frontier).await;
                    let changes = journal
                        .query::<Changes>(Position::ORIGIN)
                        .await
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap();
                    assert_eq!(
                        changes[1].metadata.link,
                        Some(harmos::state::Link::UndoOf(initial.position))
                    );
                    assert_eq!(
                        journal
                            .state()
                            .read(|s| (s.balance, s.applications))
                            .await
                            .unwrap()
                            .value,
                        (9, 3)
                    );
                    drop(journal);
                    runtime.stop().await.unwrap();
                    let restarted = boot(&path, 1).await;
                    assert_eq!(
                        restarted
                            .state
                            .state()
                            .read(|s| (s.balance, s.applications))
                            .await
                            .unwrap()
                            .value,
                        (9, 3)
                    );
                    assert_eq!(
                        restarted
                            .state
                            .query::<Facts>(Position::ORIGIN)
                            .await
                            .collect::<Result<Vec<_>, _>>()
                            .unwrap()
                            .len(),
                        3
                    );
                    restarted.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("historical_snapshot_boundary", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::{storage::JournalConsumer, storage::Source};
                    let directory = scratch("atomic-historical");
                    let path = directory.join("journal.jsonl");
                    let live = boot(&path, 1).await;
                    live.state.commit(add(5, 1, 0)).await.unwrap();
                    live.stop().await.unwrap();
                    let text = std::fs::read_to_string(&path).unwrap();
                    let (header, line) = text.split_once('\n').unwrap();
                    let mut group: serde_json::Value = serde_json::from_str(line).unwrap();
                    group["entries"][0]["version"] = 1.into();
                    group["entries"][0]["payload"] = serde_json::json!({"amount":5});
                    std::fs::write(&path, format!("{header}\n{group}\n")).unwrap();
                    let snapshots = Arc::new(std::sync::Mutex::new(Vec::new()));
                    let observed = Arc::clone(&snapshots);
                    let snapshot = jsonl_adapter::Snapshot::<Model>::folding(
                        State::default(),
                        Position::ORIGIN,
                        move |state, position| {
                            observed.lock().unwrap().push((
                                state.balance,
                                state.applications,
                                position,
                            ));
                            Ok(())
                        },
                    );
                    let runtime = Runtime::<Model>::builder(State::default(), ())
                        .register(Changes::catalog())
                        .register(Facts::catalog())
                        .recover(Jsonl::<Model>::open(&path).unwrap())
                        .snapshot(snapshot)
                        .tail(1)
                        .runtime()
                        .await
                        .unwrap();
                    assert_eq!(
                        runtime
                            .state
                            .state()
                            .read(|s| (s.balance, s.applications))
                            .await
                            .unwrap()
                            .value,
                        (10, 1)
                    );
                    runtime.stop().await.unwrap();
                    assert_eq!(*snapshots.lock().unwrap(), vec![(10, 1, Position(2))]);
                    let mut source = Jsonl::<Model>::open(&path).unwrap();
                    assert_eq!(
                        source
                            .read(Position(1), Position(2))
                            .await
                            .unwrap()
                            .entries
                            .len(),
                        1
                    );
                    assert!(
                        Jsonl::<Model>::open(&path)
                            .unwrap()
                            .after(Position(1))
                            .load()
                            .await
                            .is_err()
                    );
                    source.prune(Position(1)).await;
                    assert_eq!(
                        source
                            .read(Position::ORIGIN, Position(2))
                            .await
                            .unwrap()
                            .entries
                            .len(),
                        2
                    );
                    source.prune(Position(2)).await;
                    assert!(matches!(
                        source.read(Position::ORIGIN, Position(2)).await,
                        Err(Error::BeneathHorizon)
                    ));
                    assert_eq!(source.resume(), Position(2));
                });
        });
    }
    {
        scenario_runtime::check("record_keeps_group", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let directory = scratch("atomic-unknown");
                    let path = directory.join("journal.jsonl");
                    let mirror = directory.join("mirror.jsonl");
                    let live = boot(&path, 1).await;
                    live.state.commit(add(5, 1, 0)).await.unwrap();
                    live.stop().await.unwrap();
                    let original = std::fs::read_to_string(&path)
                        .unwrap()
                        .replace("atomic.observed", "atomic.future");
                    std::fs::write(&path, &original).unwrap();
                    let snapshots = Arc::new(std::sync::Mutex::new(Vec::new()));
                    let observed = Arc::clone(&snapshots);
                    let snapshot = jsonl_adapter::Snapshot::<Model>::folding(
                        State::default(),
                        Position::ORIGIN,
                        move |state, position| {
                            observed.lock().unwrap().push((state.balance, position));
                            Ok(())
                        },
                    );
                    let runtime = Runtime::<Model>::builder(State::default(), ())
                        .register(Changes::catalog())
                        .register(Facts::catalog())
                        .recover(Jsonl::<Model>::open(&path).unwrap())
                        .attach(Jsonl::<Model>::open(&mirror).unwrap())
                        .snapshot(snapshot)
                        .tail(1)
                        .runtime()
                        .await
                        .unwrap();
                    assert_eq!(runtime.state.applied(), Position(2));
                    assert!(matches!(
                        runtime
                            .state
                            .state()
                            .at(Origin::initial(State::default()), Position(1))
                            .await,
                        Err(Error::IncompleteCommit)
                    ));
                    assert_eq!(
                        runtime.state.query::<Facts>(Position::ORIGIN).await.count(),
                        0
                    );
                    runtime.stop().await.unwrap();
                    assert_eq!(*snapshots.lock().unwrap(), vec![(5, Position(2))]);
                    assert_eq!(std::fs::read_to_string(mirror).unwrap(), original);
                });
        });
    }
    {
        scenario_runtime::check("facts_share_group", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let path = scratch("atomic-sidecar").join("journal.jsonl");
                    let runtime = boot(&path, 1).await;
                    let receipt = runtime.state.commit(add(5, 1, 10)).await.unwrap();
                    assert_eq!(
                        (receipt.position, receipt.frontier),
                        (Position(1), Position(3))
                    );
                    runtime.state.wait_until_stored(receipt.frontier).await;
                    runtime.stop().await.unwrap();
                    let runtime = boot(&path, 1).await;
                    let facts = runtime
                        .state
                        .query::<harmos::state::SystemFact>(Position::ORIGIN)
                        .await
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap();
                    assert_eq!(facts.len(), 1);
                    assert_eq!(facts[0].position, Position(2));
                    assert_eq!(
                        facts[0].value,
                        harmos::state::SystemFact {
                            source: harmos::state::FactSource::Sidecar("my.sidecar-2".to_owned()),
                            name: "result/sample".to_owned(),
                            version: 37,
                            payload: vec![0, 255, 17],
                        }
                    );
                    assert_eq!(
                        facts[0].value.identify().0.as_str(),
                        "harmos:sidecar/my.sidecar-2/result/sample"
                    );
                    let typed = runtime
                        .state
                        .query::<Facts>(Position::ORIGIN)
                        .await
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap();
                    assert_eq!(typed.len(), 1);
                    assert_eq!(typed[0].position, Position(3));
                    assert_eq!(
                        runtime
                            .state
                            .state()
                            .read(|s| (s.balance, s.applications))
                            .await
                            .unwrap()
                            .value,
                        (5, 1)
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("mixed_frontier_recovers", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::storage::Source;
                    let path = scratch("atomic-query-frontier").join("journal.jsonl");
                    let runtime = boot(&path, 1).await;
                    let first = runtime.state.commit(add(5, 1, 10)).await.unwrap();
                    runtime.state.wait_until_stored(first.frontier).await;
                    let mut lagging = Jsonl::<Model>::open(&path).unwrap();
                    let later = runtime.state.commit(add(8, 1, 10)).await.unwrap();
                    runtime.state.wait_until_stored(later.frontier).await;
                    assert!(matches!(
                        lagging.read(Position::ORIGIN, later.frontier).await,
                        Err(Error::BeyondTail)
                    ));
                    let entries = runtime
                        .state
                        .query_through::<Recorded<Model>>(Position::ORIGIN, first.frontier)
                        .await
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap();
                    assert_eq!(
                        entries
                            .iter()
                            .map(|entry| entry.position)
                            .collect::<Vec<_>>(),
                        [Position(1), Position(2), Position(3)]
                    );
                    assert!(matches!(entries[0].value, Recorded::Transaction(_)));
                    assert!(matches!(entries[1].value, Recorded::System(_)));
                    assert!(matches!(entries[2].value, Recorded::Record(_)));
                    runtime.stop().await.unwrap();
                    let inspector = Runtime::<Model>::inspect(
                        State::default(),
                        Jsonl::<Model>::open(&path).unwrap(),
                    )
                    .await
                    .unwrap();
                    let read = inspector
                        .state
                        .query_through::<Recorded<Model>>(Position::ORIGIN, first.frontier)
                        .await
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap();
                    assert_eq!(
                        read.iter().map(|entry| entry.position).collect::<Vec<_>>(),
                        [Position(1), Position(2), Position(3)]
                    );
                    assert!(matches!(
                        inspector
                            .state
                            .query_through::<Recorded<Model>>(
                                later.frontier,
                                Position(later.frontier.0 + 1)
                            )
                            .await
                            .next(),
                        Some(Err(Error::FrontierAhead))
                    ));
                    drop(inspector);
                    let mut source = Jsonl::<Model>::open(&path).unwrap();
                    assert!(matches!(
                        source
                            .read(Position::ORIGIN, Position(later.frontier.0 + 1))
                            .await,
                        Err(Error::BeyondTail)
                    ));
                    source.prune(first.frontier).await;
                    assert!(matches!(
                        source.read(Position::ORIGIN, later.frontier).await,
                        Err(Error::BeneathHorizon)
                    ));
                });
        });
    }
    {
        scenario_runtime::check("contract_refuses_apply", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    struct Capture(Arc<AtomicUsize>);
                    impl harmos::state::JournalCapture<Model> for Capture {
                        async fn capture(
                            &mut self,
                            _: Vec<Arc<harmos::state::Entry<Recorded<Model>>>>,
                            _: Position,
                        ) -> Result<(), harmos::state::CaptureError> {
                            self.0.fetch_add(1, Ordering::SeqCst);
                            Ok(())
                        }
                    }
                    let captures = Arc::new(AtomicUsize::new(0));
                    let runtime = Runtime::<Model>::builder(State::default(), ())
                        .register(Changes::catalog())
                        .require_capture(
                            harmos::state::CaptureLimits {
                                pending_bytes: 4096,
                                max_group_bytes: 4096,
                            },
                            Capture(Arc::clone(&captures)),
                        )
                        .unwrap()
                        .runtime()
                        .await
                        .unwrap();
                    let refused = runtime.state.commit(add(7, 1, 0)).await;
                    assert!(matches!(
                        refused,
                        Err(Error::Preparation(
                            harmos::state::PreparationError::Membership
                        ))
                    ));
                    assert_eq!(runtime.state.applied(), Position::ORIGIN);
                    let state = runtime
                        .state
                        .state()
                        .read(|state| (state.balance, state.applications))
                        .await
                        .unwrap();
                    assert_eq!(state.value, (0, 0));
                    assert_eq!(state.position, Position::ORIGIN);
                    runtime.stop().await.unwrap();
                    assert_eq!(captures.load(Ordering::SeqCst), 0);
                });
        });
    }
}

/* === Tests === */
