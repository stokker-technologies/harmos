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

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::Path;
use std::sync::{Arc, Mutex};

// Third Party Imports

// Local Imports
use harmos_runtime::state::{CatalogCodec, DecodeError, Position, SnapshotPolicy, Transaction};
use harmos_runtime::storage::{ObjectCatalog, ObjectRetention, StorageManager, StoredObject};
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

#[harmos_runtime::message]
#[derive(Serialize, Deserialize)]
struct PluginObject {
    id: String,
    value: u64,
}

#[harmos_runtime::message]
#[derive(Serialize, Deserialize)]
struct NewPluginObject {
    id: String,
    value: String,
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

#[harmos_domain::catalog]
enum PluginObjects {
    Object(PluginObject),
}

#[harmos_domain::catalog]
enum NewPluginObjects {
    Object(NewPluginObject),
}

#[harmos_domain::catalog]
enum StoredPluginObjects {
    Old(PluginObject),
    New(NewPluginObject),
}

/* === Traits === */

/* === Implementations === */

impl Identified for PluginObject {
    const ID: &'static str = "plugin.document";
}

impl StoredObject for PluginObject {
    type Key = String;
    const RETENTION: ObjectRetention = ObjectRetention::Latest;

    fn key(&self) -> &String {
        &self.id
    }
}

impl Identified for NewPluginObject {
    const ID: &'static str = "plugin.document";
}

impl StoredObject for NewPluginObject {
    type Key = String;
    const RETENTION: ObjectRetention = ObjectRetention::Latest;

    fn key(&self) -> &String {
        &self.id
    }
}

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

async fn original<C>(storage: &StorageManager<C>, snapshot: bool) -> Runtime<Model<false>> {
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

fn plugin_storage<C: ObjectCatalog>(root: &Path, data: &Arc<crate::storage::Data<StoredPluginObjects>>) -> StorageManager<C> {
    let backends = harmos_runtime::storage::StorageBackends {
        primary: data.clone(),
        state: Arc::new(harmos_runtime::storage::state::file::FileBackend::new(
            root.join("state"),
            crate::storage::history(),
            crate::storage::state_settings(),
        )),
    };
    StorageManager::new(backends, crate::storage::config()).unwrap()
}

async fn plugin_runtime<C>(storage: &StorageManager<C>, snapshot: bool) -> Runtime<Model<false>> {
    let mut settings = managed(snapshot);
    settings.journal_tail = 16;
    Runtime::<Model<false>>::launch_with(0, (), |setup| {
        setup.settings().storage(storage).register(Changes::catalog()).register(Facts::catalog()).managed_storage(settings);
    })
    .await
    .unwrap()
}

async fn historical_plugin_values<C>(storage: &StorageManager<C>, schema: &harmos_runtime::storage::SchemaKey) {
    use arrow_array::{StructArray, UInt64Array};
    use harmos_runtime::storage::query::{ObjectSelection, ObjectVersions, QueryArrow, RecordSelection, RowSlice, Source};

    for source in [
        Source::Records(RecordSelection {
            schema: schema.clone(),
            scope: None,
            item: None,
            source_start: None,
            source_end: None,
        }),
        Source::Objects(ObjectSelection {
            schema: Some(schema.clone()),
            key: None,
            versions: ObjectVersions::Revisions,
        }),
    ] {
        let mut reader = storage
            .query(harmos_runtime::storage::Query {
                at: storage.cursor_saved(),
                source,
                transforms: vec![],
                slice: RowSlice::default(),
            })
            .await
            .unwrap();
        let mut values = Vec::new();
        while let Some(output) = reader.next().await.unwrap() {
            if let QueryArrow::Batch(batch) = output {
                let payload = batch.as_record_batch().column_by_name("payload").unwrap().as_any().downcast_ref::<StructArray>().unwrap();
                let column = payload.column_by_name("value").unwrap().as_any().downcast_ref::<UInt64Array>().unwrap();
                values.extend(column.values().iter().copied());
            }
        }
        assert_eq!(values, [41]);
    }
}

/* === Tests === */

#[tokio::test]
async fn plugin_history_survives_shape_change_and_absence_on_one_root() {
    use harmos_runtime::schema::Described;
    use harmos_runtime::state::{FactSource, SystemFact};
    use harmos_runtime::storage::{ObjectVersion, RecordInput};
    use harmos_runtime::{Identifier, Message};

    for snapshot in [false, true] {
        // Write Plugin History And Host State.
        let root = tempfile::tempdir().unwrap();
        let data = Arc::new(crate::storage::Data::<StoredPluginObjects>::with_catalog(crate::storage::history()));
        let disk = plugin_storage::<PluginObjects>(root.path(), &data);
        let runtime = plugin_runtime(&disk, snapshot).await;
        let value = PluginObject { id: "document".into(), value: 41 };
        let receipt = disk.write_object(&value).await.unwrap();
        let encoded = PluginObjects::encode(&value, NonZeroUsize::new(4096).unwrap()).unwrap();
        let schema = encoded.schema;
        disk.write_records(vec![RecordInput {
            schema: schema.clone(),
            scope: None,
            item: Identifier::new("plugin-row").unwrap(),
            source: None,
            source_time: None,
            payload: Message::encode(&value),
        }])
        .await
        .unwrap();
        disk.flush().await.unwrap();
        let old = SystemFact::encode(FactSource::Sidecar("probe".into()), "reading".into(), &value).unwrap();
        runtime.records.publish_system(old.clone()).await.unwrap();
        runtime.state.commit(Add { amount: 7 }).await.unwrap();
        runtime.shutdown().await.unwrap();

        // Reopen With The Changed Plugin Definition And Write Current State.
        let disk = plugin_storage::<NewPluginObjects>(root.path(), &data);
        let runtime = plugin_runtime(&disk, snapshot).await;
        let history = runtime.state.query::<SystemFact>(Position::ORIGIN).await.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(history[0].value, old);
        let written = history[0].value.schema().unwrap();
        let graph = harmos_runtime::schema::graph::Schema::from_declaration(PluginObject::DEFINITION, &Default::default()).unwrap();
        assert_eq!(written, &graph);
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&history[0].value.payload).unwrap()["value"], 41);
        let current = NewPluginObject {
            id: value.id.clone(),
            value: "current".into(),
        };
        assert_ne!(PluginObject::definition().1, NewPluginObject::definition().1);
        // A Typed Read Must Never Interpret Old Bytes As The New Definition.
        assert!(disk.read_object::<NewPluginObject>(&value.id, ObjectVersion::Exact(receipt.version)).await.is_err());
        disk.write_object(&current).await.unwrap();
        disk.write_object(&NewPluginObject {
            id: value.id.clone(),
            value: "latest".into(),
        })
        .await
        .unwrap();
        disk.flush().await.unwrap();
        assert_eq!(disk.read_object::<NewPluginObject>(&value.id, ObjectVersion::Latest).await.unwrap().unwrap().value.value, "latest");
        let next = SystemFact::encode(FactSource::Sidecar("probe".into()), "reading".into(), &current).unwrap();
        assert_ne!(old.identify().1, next.identify().1);
        runtime.records.publish_system(next.clone()).await.unwrap();
        historical_plugin_values(&disk, &schema).await;
        runtime.shutdown().await.unwrap();

        // Reopen Without Any Plugin Object Catalog Or Installed Plugin.
        let disk = plugin_storage::<()>(root.path(), &data);
        let runtime = plugin_runtime(&disk, snapshot).await;
        historical_plugin_values(&disk, &schema).await;
        assert_eq!(disk.lookup_schema(&schema).await.unwrap().schema.as_ref(), &graph);
        let history = runtime.state.query::<SystemFact>(Position::ORIGIN).await.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(history.into_iter().map(|entry| entry.value).collect::<Vec<_>>(), [old, next]);
        assert_eq!(runtime.state.state().read(|state| *state).await.unwrap().value, 7);
        runtime.shutdown().await.unwrap();

        // The Same Root Still Refuses A Changed Host Record Definition.
        let disk = plugin_storage::<()>(root.path(), &data);
        let result = Runtime::<Model<true>>::launch_with(0, (), |setup| {
            setup.settings().storage(&disk).register(Changes::catalog()).register(NewFacts::catalog()).managed_storage(managed(snapshot));
        })
        .await;
        let error = match result {
            Ok(_) => panic!("changed host record was accepted"),
            Err(error) => crate::storage::state::incompatible(error),
        };
        assert!(error.to_string().contains(Observed::ID), "{error}");
        assert!(error.to_string().contains("fresh"), "{error}");
        let disk = plugin_storage::<()>(root.path(), &data);
        let runtime = plugin_runtime(&disk, snapshot).await;
        historical_plugin_values(&disk, &schema).await;
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn unchanged_shapes_restart_on_one_root() {
    for snapshot in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let data = Arc::new(crate::storage::Data::<StoredPluginObjects>::with_catalog(crate::storage::history()));
        let disk = plugin_storage::<()>(root.path(), &data);
        let runtime = original(&disk, snapshot).await;
        runtime.state.commit(Add { amount: 7 }).await.unwrap();
        runtime.records.publish(Observed { before: 7 }).await.unwrap();
        let expected = runtime.state.applied();
        runtime.shutdown().await.unwrap();

        let disk = plugin_storage::<()>(root.path(), &data);
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
        let data = Arc::new(crate::storage::Data::<StoredPluginObjects>::with_catalog(crate::storage::history()));
        let disk = plugin_storage::<()>(root.path(), &data);
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

        let disk = plugin_storage::<()>(root.path(), &data);
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

        let disk = plugin_storage::<()>(root.path(), &data);
        let runtime = original(&disk, snapshot).await;
        assert_eq!(runtime.state.state().read(|state| *state).await.unwrap().value, 7);
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn native_stream_round_trip_uses_shared_revision() {
    let root = tempfile::tempdir().unwrap();
    let data = Arc::new(crate::storage::Data::<StoredPluginObjects>::with_catalog(crate::storage::history()));
    let disk = plugin_storage::<()>(root.path(), &data);
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
    let revision = harmos_domain::streams::revision(definition.schema(), definition.window()).unwrap();
    assert_eq!(definition.version(), revision);
    let stored = runtime.streams.definitions().into_iter().find(|item| item.id() == Sample::ID).unwrap();
    assert_eq!(stored.version(), revision);
    runtime.shutdown().await.unwrap();
    assert_eq!(*borrowed.lock().unwrap(), [41]);
    assert_eq!(*shared.lock().unwrap(), [41]);
}

#[test]
fn label_only_change_keeps_revision() {
    #[harmos_runtime::stream(id = "shape.samples", window = 4, label = "Renamed Samples")]
    struct Relabelled {
        value: u64,
    }

    #[harmos_runtime::record(id = "shape.observed")]
    #[harmos_runtime::message]
    #[derive(serde::Serialize, serde::Deserialize)]
    struct LabelledRecord {
        #[row(label = "Previous Value")]
        before: i64,
    }

    assert_eq!(Observed::definition().1, LabelledRecord::definition().1);
    assert_ne!(Sample::definition().metadata(), Relabelled::definition().metadata());
    assert_eq!(Sample::definition().version(), Relabelled::definition().version());
    let value = Observed { before: 9 };
    let (id, revision) = value.identify();
    let encoded = serde_json::to_value(&value).unwrap();
    assert!(Facts::restore(&id, revision, &encoded).unwrap().is_some());
    assert!(Facts::restore(&id, Changed::definition().1, &encoded).unwrap().is_none());
}
