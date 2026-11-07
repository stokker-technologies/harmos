/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Derived Data Shape Recovery Contracts

/* === Modules === */

mod relabelled {
    #[harmos_runtime::stream(id = "shape.samples", window = 4, label = "Renamed Samples")]
    pub struct Row {
        value: u64,
    }
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::Path;
use std::sync::{Arc, Mutex};

// Third Party Imports

// Local Imports
use harmos_runtime::state::{CatalogCodec, DecodeError, Position, SnapshotPolicy, Transaction};
use harmos_runtime::storage::{JsonStorage, JsonStorageSettings, StorageManager};
use harmos_runtime::{Application, Identified, Lifecycle, Runtime, Setup, Startup};
use serde::{Deserialize, Serialize};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Model<const CHANGED: bool>;

struct Recording {
    borrowed: Arc<Mutex<Vec<u64>>>,
    shared: Arc<Mutex<Vec<u64>>>,
    cursor: harmos_runtime::storage::Cursor,
}

#[harmos_runtime::transaction(id = "shape.add")]
#[derive(harmos_runtime::Described, Clone, Serialize, Deserialize)]
struct Add {
    amount: i64,
}

#[harmos_runtime::record(id = "shape.observed")]
#[derive(harmos_runtime::Described, Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Observed {
    before: i64,
}

#[harmos_runtime::record(id = "shape.observed")]
#[derive(harmos_runtime::Described, Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Changed {
    before: String,
}

#[harmos_runtime::stream(id = "shape.samples", window = 4, label = "Samples")]
struct Sample {
    value: u64,
}

/* === Enums === */

#[harmos_runtime::transactions(state = i64, error = std::convert::Infallible)]
#[derive(Clone)]
enum Changes {
    Add(Add),
}

#[harmos_runtime::records]
#[derive(Clone)]
enum Facts {
    Observed(Observed),
}

#[harmos_runtime::records]
#[derive(Clone)]
enum NewFacts {
    Changed(Changed),
}

#[harmos_runtime::streams]
enum Streams {
    Sample(Sample),
}

#[harmos_runtime::lanes]
enum Lanes {
    #[default]
    Open,
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::storage::StreamConsumer<Model<false>> for Recording {
    fn resume(&self) -> harmos_runtime::storage::Cursor {
        self.cursor
    }
    fn selections(&self) -> Vec<harmos_runtime::streams::Definition> {
        vec![Sample::definition()]
    }
    async fn drain(&mut self, batch: harmos_runtime::storage::Batch<Model<false>>) -> harmos_runtime::storage::Cursor {
        self.borrowed.lock().unwrap().extend(batch.select::<Sample>().map(|row| row.value.value));
        self.shared.lock().unwrap().extend(batch.select_shared::<Sample>().map(|row| row.value.value));
        self.cursor = batch.reaches();
        self.cursor
    }
}

impl Lifecycle for Model<false> {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = Startup<Self>;
    async fn prepare(_: (), _: &mut Setup<Self>) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl Lifecycle for Model<true> {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = Startup<Self>;
    async fn prepare(_: (), _: &mut Setup<Self>) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl Application for Model<false> {
    type Lanes = Lanes;
    type State = i64;
    type Transaction = Changes;
    type Record = Facts;
    type Streams = Streams;
}

impl Application for Model<true> {
    type Lanes = Lanes;
    type State = i64;
    type Transaction = Changes;
    type Record = NewFacts;
    type Streams = ();
}

impl Transaction<i64> for Add {
    type Error = std::convert::Infallible;
    fn check(&self, _: &i64) -> Result<(), Self::Error> {
        Ok(())
    }
    fn records(&self, state: &i64, out: &mut harmos_runtime::state::Records) {
        out.push(Observed { before: *state }).unwrap();
    }
    fn apply(&self, state: &mut i64) {
        *state += self.amount;
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn storage(root: &Path) -> StorageManager {
    let primary = JsonStorage::new(
        JsonStorageSettings {
            root: root.to_owned(),
            max_entry_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        },
        crate::storage::history(),
    );
    StorageManager::new(primary.into_backends(crate::storage::state_settings()), crate::storage::config()).unwrap()
}

fn managed(snapshot: bool) -> harmos_runtime::config::ManagedStorageSettings {
    harmos_runtime::config::ManagedStorageSettings {
        replay: harmos_runtime::storage::ReadLimits {
            rows: NonZeroUsize::new(16).unwrap(),
            bytes: NonZeroUsize::new(64 * 1024).unwrap(),
        },
        capture: harmos_runtime::storage::ReadLimits {
            rows: NonZeroUsize::new(16).unwrap(),
            bytes: NonZeroUsize::new(64 * 1024).unwrap(),
        },
        journal_tail: 1,
        snapshots: snapshot.then(|| SnapshotPolicy {
            schema: crate::storage::schema(),
            every: NonZeroU64::new(100).unwrap(),
            max_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
        }),
    }
}

async fn original(storage: &StorageManager, snapshot: bool) -> Runtime<Model<false>> {
    Runtime::<Model<false>>::launch_with(0, (), |setup| {
        setup
            .settings()
            .storage(storage)
            .register(Changes::catalog())
            .register(Facts::catalog())
            .register(Streams::catalog())
            .managed_storage(managed(snapshot));
    })
    .await
    .unwrap()
}

/* === Tests === */

#[tokio::test]
async fn unchanged_shapes_restart_on_one_root() {
    for snapshot in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let disk = storage(root.path());
        let runtime = original(&disk, snapshot).await;
        runtime.state.commit(Add { amount: 7 }).await.unwrap();
        runtime.records.publish(Observed { before: 7 }).await.unwrap();
        let expected = runtime.state.applied();
        runtime.shutdown().await.unwrap();

        let disk = storage(root.path());
        let runtime = original(&disk, snapshot).await;
        assert_eq!(runtime.state.state().read(|state| *state).await.unwrap().value, 7);
        assert_eq!(runtime.state.applied(), expected);
        let facts = runtime.state.query::<Facts>(Position(expected.0 - 1)).await.collect::<Result<Vec<_>, _>>().unwrap();
        assert!(matches!(&facts.last().unwrap().value, Facts::Observed(value) if value.before == 7));
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn changed_record_shape_refuses_root_and_preserves_history() {
    assert_ne!(Observed::definition().1, Changed::definition().1);
    for snapshot in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let disk = storage(root.path());
        let runtime = original(&disk, snapshot).await;
        runtime.records.publish(Observed { before: 3 }).await.unwrap();
        runtime.state.commit(Add { amount: 7 }).await.unwrap();
        runtime
            .records
            .publish_system(harmos_runtime::state::SystemFact::new(
                harmos_runtime::state::FactSource::Work("shape".into()),
                "tail".into(),
                vec![1],
            ))
            .await
            .unwrap();
        runtime.shutdown().await.unwrap();

        let disk = storage(root.path());
        let result = Runtime::<Model<true>>::launch_with(0, (), |setup| {
            setup.settings().storage(&disk).register(Changes::catalog()).register(NewFacts::catalog()).managed_storage(managed(snapshot));
        })
        .await;
        let error = match result {
            Ok(_) => panic!("changed shape was accepted"),
            Err(error) => error,
        };
        let physical = crate::storage::state::incompatible(error);
        let mut cause: &dyn std::error::Error = &physical;
        let refusal = loop {
            if let Some(refusal) = cause.downcast_ref::<DecodeError>() {
                break refusal;
            }
            cause = cause.source().expect("structured decode refusal");
        };
        assert!(matches!(refusal, DecodeError::UndeclaredVersion { id, version }
            if id.as_str() == Observed::ID && *version == Observed::definition().1));
        assert!(disk.status().closed);

        let disk = storage(root.path());
        let runtime = original(&disk, snapshot).await;
        assert_eq!(runtime.state.state().read(|state| *state).await.unwrap().value, 7);
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn native_stream_round_trip_uses_shared_revision() {
    let root = tempfile::tempdir().unwrap();
    let disk = storage(root.path());
    let borrowed = Arc::new(Mutex::new(Vec::new()));
    let shared = Arc::new(Mutex::new(Vec::new()));
    let runtime = Runtime::<Model<false>>::launch_with(0, (), |setup| {
        setup
            .settings()
            .storage(&disk)
            .register(Changes::catalog())
            .register(Facts::catalog())
            .register(Streams::catalog())
            .managed_storage(managed(false))
            .attach_streams(Recording {
                borrowed: borrowed.clone(),
                shared: shared.clone(),
                cursor: harmos_runtime::storage::Cursor::ORIGIN,
            });
    })
    .await
    .unwrap();
    runtime.streams.publish(Sample { value: 41 });
    let mut rows = runtime.streams.query::<Sample>(..).await.unwrap();
    assert_eq!(rows.next_back().unwrap().value.value, 41);
    let definition = Sample::definition();
    let revision = harmos_domain::streams::revision(definition.schema().unwrap(), definition.window()).unwrap();
    assert_eq!(definition.version(), revision);
    let stored = runtime.streams.definitions().into_iter().find(|item| item.id() == Sample::ID).unwrap();
    assert_eq!(stored.version(), revision);
    runtime.shutdown().await.unwrap();
    assert_eq!(*borrowed.lock().unwrap(), [41]);
    assert_eq!(*shared.lock().unwrap(), [41]);
}

#[test]
fn label_only_change_keeps_revision() {
    assert_ne!(Sample::definition().metadata(), relabelled::Row::definition().metadata());
    assert_eq!(Sample::definition().version(), relabelled::Row::definition().version());
    let value = Observed { before: 9 };
    let (id, revision) = value.identify();
    let encoded = serde_json::to_value(&value).unwrap();
    assert!(Facts::restore(&id, revision, &encoded).unwrap().is_some());
    assert!(Facts::restore(&id, Changed::definition().1, &encoded).unwrap().is_none());
}
