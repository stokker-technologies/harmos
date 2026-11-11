/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Automatic Plugin Stream Revisions Across Reload And Durable Restart

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroU32;
use std::sync::Arc;

// Third Party Imports
use arrow_array::{StringArray, StructArray, UInt64Array};

// Local Imports
use crate::support::{Bank, Ledger};
use harmos_runtime::guest::{
    PluginArtifact, PluginBackend, PluginContributions, PluginDefinition, PluginDefinitions, PluginEndpoint, PluginLifecycle,
    PluginLifecycleAuthority, PluginManager, PluginTransition, Router,
};
use harmos_runtime::storage::query::{QueryArrow, RowSlice, Source, StreamSelection};
use harmos_runtime::storage::{Cursor, PayloadEncoding, RecordInput, SchemaKey, StorageManager, StreamInput};
use harmos_runtime::streams::{PublicationRow, PublishedStream, StreamManager, StreamRegistration, StreamWriter};
use harmos_runtime::{Error, Message, Result, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Backends = PluginBackend<Resources, Resources>;

/* === Structs === */

#[harmos_runtime::message]
struct Number {
    value: u64,
}

#[harmos_runtime::message]
struct Text {
    value: String,
}

#[derive(Default)]
struct Resources {
    endpoint: Option<PluginEndpoint>,
    operations: Option<harmos_runtime::operations::PluginRegistration>,
    streams: Option<StreamRegistration>,
}

struct Host {
    streams: StreamManager<Bank>,
    operations: harmos_runtime::operations::OperationManager<crate::support::Lanes>,
    lease: Option<Arc<StreamWriter>>,
    fail: bool,
    before: Option<u32>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl PluginLifecycle for Host {
    type Guest = Resources;

    type Embedded = Resources;

    fn prepare(&mut self, _: &PluginDefinition) -> Result<Backends> {
        Ok(PluginBackend::Embedded(Resources::default()))
    }

    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> Result<()> {
        Ok(())
    }

    async fn start(
        &mut self, definition: &PluginDefinition, router: &Router, _: &PluginLifecycleAuthority<'_>, backend: &mut Backends,
    ) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else { unreachable!() };
        resources.endpoint = Some(router.enter("revision_plugin").unwrap());
        let scope = resources.endpoint.as_ref().unwrap().scope().unwrap();
        resources.operations = Some(self.operations.stage_plugin(scope, |_| Ok(()))?);
        resources.streams = Some(self.streams.stage_plugin(scope, definition.contributions.streams.clone()).await.unwrap());
        self.lease = Some(resources.streams.as_ref().unwrap().leases()[0].clone());
        let row = if definition.configuration == b"true" { Text { value: "seven".into() }.encode() } else { Number { value: 7 }.encode() };
        self.lease
            .as_ref()
            .unwrap()
            .stage_derived_row(
                PublicationRow {
                    payload: row,
                    published_unix_micros: None,
                    context: None,
                },
                0,
            )
            .unwrap();
        assert_eq!(self.streams.current_publication("revision.samples").map(|stream| stream.definition().version()), self.before);
        if std::mem::take(&mut self.fail) {
            return Err(Error::ResourceExhausted { resource: "scripted reload failure" });
        }
        Ok(())
    }

    async fn stop(_: &PluginDefinition, _: &PluginLifecycleAuthority<'_>, backend: &mut Backends) -> Result<()> {
        let PluginBackend::Embedded(resources) = backend else { unreachable!() };
        if let Some(registration) = &mut resources.operations {
            registration.shutdown().await?;
        }
        resources.streams.take();
        resources.endpoint.take();
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn candidate(text: bool, label: &str) -> PluginDefinitions {
    let row = if text {
        <Text as harmos_runtime::schema::Described>::DEFINITION
    } else {
        <Number as harmos_runtime::schema::Described>::DEFINITION
    };
    let stream = harmos_domain::StreamDeclaration {
        id: "revision.samples".into(),
        metadata: harmos_runtime::Metadata::new(harmos_runtime::Identifier::new("revision.samples").unwrap(), label, None),
        row_metadata: Vec::new(),
        window: 4,
        row: harmos_runtime::schema::graph::Schema::from_declaration(row, &Default::default()).unwrap(),
    };
    PluginDefinitions::new(
        [PluginDefinition {
            id: "revision_plugin".into(),
            version: "1.0.0".parse().unwrap(),
            artifact: PluginArtifact::Embedded { revision: label.into() },
            configuration: if text { b"true".to_vec() } else { b"false".to_vec() },
            requires: Vec::new(),
            contributions: PluginContributions {
                streams: vec![stream],
                ..Default::default()
            },
        }],
        [],
    )
    .unwrap()
}

async fn setup(text: bool) -> (Runtime<Bank>, PluginManager<Resources, Resources>, Host) {
    let runtime = crate::support::booted(Ledger::default(), |_| {}).await;
    let host = Host {
        streams: runtime.streams.clone(),
        operations: runtime.operations.clone(),
        lease: None,
        fail: false,
        before: None,
    };
    (runtime, PluginManager::new(candidate(text, "Samples")).unwrap(), host)
}

fn current(host: &Host) -> PublishedStream {
    host.streams.current_publication("revision.samples").unwrap()
}

async fn capture(storage: &StorageManager, feed: &mut harmos_runtime::streams::StreamFeed<Bank>) -> SchemaKey {
    let batch = feed.next().await.unwrap();
    let (definition, _, row) = batch.publications().next().unwrap();
    let key = SchemaKey {
        id: harmos_runtime::Identifier::new(definition.id()).unwrap(),
        version: NonZeroU32::new(definition.version()).unwrap(),
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    storage.write_schema(key.clone(), definition.schema().clone()).await.unwrap();
    storage
        .write_stream(vec![StreamInput {
            record: RecordInput {
                schema: key.clone(),
                scope: None,
                item: harmos_runtime::Identifier::new("sample").unwrap(),
                source: None,
                source_time: None,
                payload: row.value.payload.clone(),
            },
            stream: harmos_runtime::Identifier::new("samples").unwrap(),
            sequence: 1,
        }])
        .await
        .unwrap();
    storage.flush().await.unwrap();
    key
}

/* === Tests === */

#[tokio::test]
async fn changed_row_commits_a_new_current_revision() {
    let (runtime, mut manager, mut host) = setup(false).await;
    manager.initialize(&mut host).await.unwrap();
    let old = current(&host);
    let old_revision = old.definition().version();
    host.before = Some(old_revision);
    manager.reload(candidate(true, "Text Samples"), &mut host).await.unwrap();
    let new = current(&host);
    assert_ne!(new.definition().version(), old_revision);
    assert_eq!(
        host.streams
            .definitions()
            .iter()
            .filter(|definition| definition.owner().is_some())
            .map(|definition| definition.version())
            .collect::<Vec<_>>(),
        [new.definition().version()]
    );
    let rows = old.query(..).await.unwrap().collect::<Vec<_>>();
    assert_eq!(Number::decode(&rows[0].value.payload).unwrap().value, 7);
    let rows = new.query(..).await.unwrap().collect::<Vec<_>>();
    assert_eq!(Text::decode(&rows[0].value.payload).unwrap().value, "seven");
    let lease = host.lease.as_ref().unwrap();
    assert_eq!(lease.stream().definition().version(), new.definition().version());
    host.before = Some(new.definition().version());
    manager.reload(candidate(false, "Numbers Again"), &mut host).await.unwrap();
    assert_eq!(current(&host).definition().version(), old_revision);
    manager.shutdown(&mut host).await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn failed_reload_keeps_old_revision_current() {
    let (runtime, mut manager, mut host) = setup(false).await;
    manager.initialize(&mut host).await.unwrap();
    let old_revision = current(&host).definition().version();
    host.before = Some(old_revision);
    host.fail = true;
    assert!(manager.reload(candidate(true, "Failed Text"), &mut host).await.is_err());
    assert_eq!(current(&host).definition().version(), old_revision);
    let rows = current(&host).query(..).await.unwrap().collect::<Vec<_>>();
    assert!(rows.iter().all(|row| Number::decode(&row.value.payload).unwrap().value == 7));
    manager.reload(candidate(false, "Recovered Numbers"), &mut host).await.unwrap();
    assert_eq!(current(&host).definition().version(), old_revision);
    host.lease.as_ref().unwrap().publish(Number { value: 9 }.encode(), 0).unwrap();
    let rows = current(&host).query(..).await.unwrap().collect::<Vec<_>>();
    assert_eq!(Number::decode(&rows.last().unwrap().value.payload).unwrap().value, 9);
    manager.shutdown(&mut host).await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn label_only_reload_preserves_revision() {
    let (runtime, mut manager, mut host) = setup(false).await;
    manager.initialize(&mut host).await.unwrap();
    let old = current(&host);
    host.before = Some(old.definition().version());
    manager.reload(candidate(false, "Renamed Samples"), &mut host).await.unwrap();
    assert_eq!(current(&host).definition().version(), old.definition().version());
    assert_ne!(current(&host).definition().metadata(), old.definition().metadata());
    manager.shutdown(&mut host).await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn restart_on_one_root_reads_each_revision_with_its_own_schema() {
    let data = Arc::new(crate::storage::fixture::Data::new(crate::storage::fixture::history()));
    let mut keys = Vec::new();
    for text in [false, true] {
        let (runtime, mut manager, mut host) = setup(text).await;
        let backends = harmos_storage::StorageBackends {
            primary: data.clone(),
            state: Arc::new(crate::storage::fixture::Recovery::new(crate::storage::fixture::history())),
        };
        let storage = StorageManager::new(backends, crate::storage::fixture::config()).unwrap();
        storage.start(&runtime).await.unwrap();
        let mut feed = host.streams.follow(Cursor::ORIGIN);
        manager.initialize(&mut host).await.unwrap();
        keys.push(capture(&storage, &mut feed).await);
        assert_eq!(keys.last().unwrap().version.get(), current(&host).definition().version());
        for (index, key) in keys.iter().enumerate() {
            let mut reader = storage
                .query(harmos_runtime::storage::Query {
                    at: storage.cursor_saved(),
                    source: Source::Stream(StreamSelection {
                        schema: key.clone(),
                        scope: None,
                        item: None,
                        stream: None,
                        source_start: None,
                        source_end: None,
                        after: 0,
                    }),
                    transforms: Vec::new(),
                    slice: RowSlice::default(),
                })
                .await
                .unwrap();
            let mut count = 0;
            while let Some(output) = reader.next().await.unwrap() {
                let QueryArrow::Batch(batch) = output else {
                    continue;
                };
                let payload = batch.as_record_batch().column_by_name("payload").unwrap().as_any().downcast_ref::<StructArray>().unwrap();
                let values = payload.column_by_name("value").unwrap();
                if index == 0 {
                    assert_eq!(values.as_any().downcast_ref::<UInt64Array>().unwrap().value(0), 7);
                } else {
                    assert_eq!(values.as_any().downcast_ref::<StringArray>().unwrap().value(0), "seven");
                }
                count += batch.as_record_batch().num_rows();
            }
            assert_eq!(count, 1);
        }
        manager.shutdown(&mut host).await.unwrap();
        drop(feed);
        storage.close().await.unwrap();
        runtime.shutdown().await.unwrap();
    }
    assert_ne!(keys[0], keys[1]);
}
