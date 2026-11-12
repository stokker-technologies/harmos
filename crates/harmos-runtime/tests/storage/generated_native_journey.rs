/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Observe Storage Frontiers Through Generated Messages.

/* === Imports === */

use harmos_domain::schema::Described;
use harmos_operations::storage as operations;
use harmos_protocol::storage as wire;
use harmos_runtime::storage::{MutationId, ObjectRetention, StorageBackends, StorageManager, StoredObject};
use harmos_runtime::{Identified, Message};
use std::{num::NonZeroUsize, sync::Arc};

/* === Structs === */

#[derive(Message, Clone, Debug, Default)]
struct Sample {
    id: String,
    time_ns: i64,
    value: f64,
}

/* === Implementations === */

impl Identified for Sample {
    const ID: &'static str = "test.storage.sample";
}
impl StoredObject for Sample {
    type Key = String;
    const RETENTION: ObjectRetention = ObjectRetention::All;
    fn key(&self) -> &String {
        &self.id
    }
}

/* === Tests === */

#[tokio::test]
async fn saved_queries_and_conditional_updates_cross_the_protocol() {
    // Compose the Catalog and Backend Through the Host.
    let catalog =
        harmos_domain::model::storage::ObjectCatalog::new().entry(harmos_domain::model::storage::ObjectDefinition::of::<Sample>());
    let data = Arc::new(super::Data::with_catalog(catalog.clone(), super::history()));
    let storage = StorageManager::with_catalog(
        catalog.clone(),
        StorageBackends {
            primary: data.clone(),
            state: Arc::new(super::Recovery::new(super::history())),
        },
        super::config(),
    )
    .unwrap();
    let runtime = crate::support::app::launch::<super::Bank>(super::Ledger::default(), (), |setup| {
        setup.settings().storage(&storage).register(super::catalog()).operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(16 * 1024 * 1024).unwrap()),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    crate::support::app::register(&runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let capabilities = runtime.operations.call_protobuf::<operations::Capabilities>(wire::Capabilities {}).await.unwrap();
    assert!(capabilities.conditional_objects && capabilities.durable_projections);

    // Create an Object Atomically and Retry Its Mutation.
    let first = Sample { id: "one".into(), time_ns: 2, value: 7.0 };
    let value = catalog.encode(&first, NonZeroUsize::new(4096).unwrap()).unwrap();
    let source = wire::ColumnSource {
        kind: wire::ColumnKind::Objects as i32,
        schema: Some(value.schema.clone().into()),
    };
    let graph = harmos_domain::schema::graph::Schema::from_declaration(Sample::DEFINITION, &Default::default()).unwrap();
    let mutation = MutationId {
        window: storage.mutation_window().unwrap().id,
        key: harmos_runtime::Identifier::new("create").unwrap(),
    };
    let create = wire::CompareExchangeObject {
        mutation: Some(mutation.into()),
        expected: None,
        schema: Some(wire::SchemaInput {
            key: Some(value.schema.clone().into()),
            graph: Some(graph.into()),
        }),
        value: Some(value.into()),
    };
    data.control.unsupported_conditional.store(true, std::sync::atomic::Ordering::Release);
    assert!(runtime.operations.call_protobuf::<operations::CompareExchangeObject>(create.clone()).await.is_err());
    assert_eq!(storage.cursor_accepted().sequence, 0);
    data.control.unsupported_conditional.store(false, std::sync::atomic::Ordering::Release);
    let created = runtime.operations.call_protobuf::<operations::CompareExchangeObject>(create.clone()).await.unwrap();
    let wire::compare_exchange_outcome::Outcome::Applied(receipt) = created.outcome.clone().unwrap() else { panic!("create applied") };
    assert_eq!(runtime.operations.call_protobuf::<operations::CompareExchangeObject>(create.clone()).await.unwrap(), created);
    let mut conflict = create.clone();
    conflict.mutation.as_mut().unwrap().key = "conflict".into();
    let refused = runtime.operations.call_protobuf::<operations::CompareExchangeObject>(conflict).await.unwrap();
    assert!(matches!(refused.outcome, Some(wire::compare_exchange_outcome::Outcome::Conflict(_))));

    // Retain the Saved Cut and Read Its Arrow Page.
    let saved =
        runtime.operations.call_protobuf::<operations::OpenSnapshot>(wire::OpenSnapshot { at: receipt.position.clone() }).await.unwrap();
    let limits = wire::ReadLimits { rows: 16, bytes: 65536 };
    let scan = wire::ArrowScan {
        source: Some(source.clone()),
        columns: vec!["payload.value".into(), "payload.time_ns".into(), "payload.id".into()],
        limits: Some(limits),
        work: Some(limits),
        ..Default::default()
    };
    let page = runtime
        .operations
        .call_protobuf::<operations::ScanArrow>(wire::ScanArrow {
            snapshot: Some(saved.clone()),
            read: Some(scan.clone()),
        })
        .await
        .unwrap();
    assert!(page.buffer_bytes() <= limits.bytes as usize);
    let batch = harmos_storage::query::protobuf::decode_batch(page.ipc, limits.try_into().unwrap()).unwrap();
    assert_eq!(batch.as_record_batch().num_rows(), 1);
    assert!(page.complete);

    // Register an Ordered Index and Read the Same Cut.
    let index = wire::IndexDefinition {
        name: "sample.values".into(),
        version: 1,
        source: Some(source.clone()),
        columns: vec!["payload.value".into()],
    };
    runtime
        .operations
        .call_protobuf::<operations::RegisterIndex>(wire::RegisterIndex {
            definition: Some(index),
            rebuild: false,
            work: Some(limits),
        })
        .await
        .unwrap();
    let index = wire::ScanIndex {
        snapshot: Some(saved.clone()),
        source: Some(source.clone()),
        read: Some(wire::IndexRead {
            name: "sample.values".into(),
            limits: Some(limits),
            work: Some(limits),
            ..Default::default()
        }),
    };
    let indexed = runtime.operations.call_protobuf::<operations::ScanIndex>(index.clone()).await.unwrap();
    assert_eq!(indexed.entries.len(), 1);
    let mut missing = index;
    missing.read.as_mut().unwrap().name = "missing".into();
    assert!(runtime.operations.call_protobuf::<operations::ScanIndex>(missing).await.is_err());

    // Materialize Buckets at an Exact Durable Frontier.
    let plan = wire::TimeBucketPlan {
        origin_ns: 0,
        width_ns: 10,
        time: "payload.time_ns".into(),
        value: "payload.value".into(),
        identity: "payload.id".into(),
        group: vec![],
        max_samples: 16,
        max_bytes: 65536,
    };
    let definition = wire::ProjectionDefinition {
        name: "sample.buckets".into(),
        version: 1,
        source: Some(source.clone()),
        transform: Some(wire::projection_definition::Transform::TimeBuckets(plan)),
    };
    let revision = runtime
        .operations
        .call_protobuf::<operations::RegisterProjection>(wire::RegisterProjection {
            definition: Some(definition),
            rebuild: false,
        })
        .await
        .unwrap()
        .revision;
    let inspect = wire::InspectProjection {
        name: "sample.buckets".into(),
        revision,
        frontier: saved.at.clone(),
        source: Some(source.clone()),
    };
    assert!(runtime.operations.call_protobuf::<operations::InspectProjection>(inspect.clone()).await.unwrap().status.is_none());
    runtime
        .operations
        .call_protobuf::<operations::AdvanceProjection>(wire::AdvanceProjection {
            snapshot: Some(saved.clone()),
            name: "sample.buckets".into(),
            work: Some(limits),
            source: Some(source.clone()),
        })
        .await
        .unwrap();
    assert_eq!(runtime.operations.call_protobuf::<operations::InspectProjection>(inspect).await.unwrap().status.unwrap().rows, 1);
    let read = wire::ReadProjection {
        source: Some(source.clone()),
        read: Some(wire::ProjectionRead {
            name: "sample.buckets".into(),
            revision,
            frontier: saved.at.clone(),
            after: 0,
            limits: Some(limits),
            work: Some(limits),
        }),
    };
    let old = runtime.operations.call_protobuf::<operations::ReadProjection>(read.clone()).await.unwrap();
    assert_eq!(old.changed_keys.len(), 1);

    // Correct the Object and Replace the Bucket Without Changing the Saved View.
    let mut correction = create;
    correction.mutation.as_mut().unwrap().key = "correct".into();
    correction.expected = receipt.version;
    correction.value = Some(catalog.encode(&Sample { value: 9.0, ..first }, NonZeroUsize::new(4096).unwrap()).unwrap().into());
    let changed = runtime.operations.call_protobuf::<operations::CompareExchangeObject>(correction).await.unwrap();
    let wire::compare_exchange_outcome::Outcome::Applied(changed) = changed.outcome.unwrap() else { panic!("correction applied") };
    let live = runtime.operations.call_protobuf::<operations::OpenSnapshot>(wire::OpenSnapshot { at: changed.position }).await.unwrap();
    runtime
        .operations
        .call_protobuf::<operations::AdvanceProjection>(wire::AdvanceProjection {
            snapshot: Some(live.clone()),
            name: "sample.buckets".into(),
            work: Some(limits),
            source: Some(source),
        })
        .await
        .unwrap();
    let mut latest = read.clone();
    latest.read.as_mut().unwrap().frontier = live.at.clone();
    let replacement = runtime.operations.call_protobuf::<operations::ReadProjection>(latest).await.unwrap();
    assert_eq!(replacement.changed_keys, old.changed_keys);
    assert_ne!(replacement.ipc, old.ipc);
    assert_eq!(runtime.operations.call_protobuf::<operations::ReadProjection>(read).await.unwrap(), old);

    // Rebuild With a Different Source and Keep the Old Revision's Source Authority.
    let mut alternate = scan.source.clone().unwrap();
    alternate.schema.as_mut().unwrap().id = "test.storage.other".into();
    let rebuilt = wire::ProjectionDefinition {
        name: "sample.buckets".into(),
        version: 2,
        source: Some(alternate),
        transform: Some(wire::projection_definition::Transform::Select(wire::SelectColumns { columns: vec!["payload.value".into()] })),
    };
    runtime
        .operations
        .call_protobuf::<operations::RegisterProjection>(wire::RegisterProjection { definition: Some(rebuilt), rebuild: true })
        .await
        .unwrap();
    let old_read = wire::ReadProjection {
        source: scan.source.clone(),
        read: Some(wire::ProjectionRead {
            name: "sample.buckets".into(),
            revision,
            frontier: saved.at.clone(),
            after: 0,
            limits: Some(limits),
            work: Some(limits),
        }),
    };
    assert_eq!(runtime.operations.call_protobuf::<operations::ReadProjection>(old_read).await.unwrap(), old);

    // Release Both Cuts and Refuse Reusing the Handle.
    runtime.operations.call_protobuf::<operations::ReleaseSnapshot>(wire::ReleaseSnapshot { snapshot: Some(saved.clone()) }).await.unwrap();
    assert!(
        runtime
            .operations
            .call_protobuf::<operations::ScanArrow>(wire::ScanArrow { snapshot: Some(saved), read: Some(scan) })
            .await
            .is_err()
    );
    runtime.operations.call_protobuf::<operations::ReleaseSnapshot>(wire::ReleaseSnapshot { snapshot: Some(live) }).await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
}
