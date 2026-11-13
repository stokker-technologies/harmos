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

struct Authority;
impl harmos_runtime::operations::authorization::Authorizer for Authority {
    fn authorize(
        &self, _: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Authorization>> {
        Ok(Some(harmos_runtime::operations::authorization::Authorization::Restricted(
            harmos_runtime::operations::authorization::Required {
                contract: provider.contract.clone(),
                grants: vec![harmos_operations::plugin::Grant::of(
                    provider.contract.operation().id.to_string(),
                    "objects:schema:test.storage.sample",
                )],
            },
        )))
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
    runtime.operations.register_authorizer(Arc::new(Authority)).unwrap();
    let plugin = super::transfer_plugin::TransferPlugin::start().await;
    let attached = runtime
        .attach_grpc(
            harmos_runtime::plugin::grpc::Placement::Remote(plugin.address.clone()),
            harmos_runtime::plugin::grpc::PlacementSettings {
                connect_timeout: std::time::Duration::from_secs(30),
                reconnect_delay: std::time::Duration::from_secs(1),
                stop_grace: std::time::Duration::from_secs(30),
                stream_window: NonZeroUsize::new(64).unwrap(),
            },
        )
        .await
        .unwrap();
    let listener = runtime.grpc().attach(harmos_runtime::grpc::Settings::default()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
    let capabilities = runtime
        .operations
        .call_protobuf::<operations::Capabilities>(harmos_protocol::v1::StorageCapabilitiesRequest {})
        .await
        .unwrap()
        .capabilities
        .unwrap();
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
    let create = harmos_protocol::v1::StorageSetObjectRequest {
        mutation: Some(mutation.into()),
        expected_revision: vec![],
        create_if_absent: true,
        schema: Some(wire::SchemaInput {
            key: Some(value.schema.clone().into()),
            graph: Some(graph.into()),
        }),
        value: Some(value.into()),
    };
    data.control.unsupported_conditional.store(true, std::sync::atomic::Ordering::Release);
    assert!(runtime.operations.call_protobuf::<operations::SetObject>(create.clone()).await.is_err());
    assert_eq!(storage.cursor_accepted().sequence, 0);
    data.control.unsupported_conditional.store(false, std::sync::atomic::Ordering::Release);
    let created = super::transfer_plugin::client_call::<operations::SetObject>(&channel, create.clone()).await.unwrap();
    let replay = super::transfer_plugin::call::<operations::context::SetObject>(
        &runtime.operations,
        harmos_protocol::context::StorageSetObjectRequest {
            mutation: create.mutation.clone(),
            expected_revision: create.expected_revision.clone(),
            create_if_absent: true,
            schema: create.schema.clone(),
            value: create.value.clone(),
        },
    )
    .await
    .unwrap();
    let created_reply = created.clone();
    let harmos_protocol::v1::storage_set_object_response::Outcome::Applied(created) = created.outcome.unwrap() else {
        panic!("create applied")
    };
    let harmos_protocol::context::storage_set_object_response::Outcome::Applied(replay) = replay.outcome.unwrap() else {
        panic!("replay applied")
    };
    assert_eq!(created.value, replay.value);
    assert_eq!(created.revision, replay.revision);
    assert_eq!(created.receipt, replay.receipt);
    let get = harmos_protocol::v1::StorageGetObjectRequest {
        schema: create.value.as_ref().unwrap().schema.clone(),
        item: "one".into(),
        at: None,
    };
    let selected = super::transfer_plugin::client_call::<operations::GetObject>(&channel, get.clone()).await.unwrap();
    assert!(selected.exists);
    assert_eq!(selected.value, created.value);
    assert_eq!(selected.revision, created.revision);
    let issued = selected.resolved_at.unwrap();

    let receipt = created.clone();
    assert_eq!(runtime.operations.call_protobuf::<operations::SetObject>(create.clone()).await.unwrap(), created_reply);
    let mut conflict = create.clone();
    conflict.mutation.as_mut().unwrap().key = "conflict".into();
    let refused = runtime.operations.call_protobuf::<operations::SetObject>(conflict.clone()).await.unwrap();
    let harmos_protocol::v1::storage_set_object_response::Outcome::Conflict(current) = refused.outcome.clone().unwrap() else {
        panic!("stale create refused")
    };
    assert_eq!(current.current_revision, created.revision);
    assert_eq!(runtime.operations.call_protobuf::<operations::SetObject>(conflict).await.unwrap(), refused);

    // Retain the Saved Cut and Read Its Arrow Page.
    let retained = runtime
        .operations
        .call_protobuf::<operations::CreateSnapshot>(harmos_protocol::v1::StorageCreateSnapshotRequest { at: None })
        .await
        .unwrap();
    let saved_cut = retained.resolved_at.unwrap();
    let saved = retained.snapshot.unwrap();
    let limits = wire::ReadLimits { rows: 16, bytes: 65536 };
    let scan = wire::ColumnSelection {
        source: Some(source.clone()),
        columns: vec!["payload.value".into(), "payload.time_ns".into(), "payload.id".into()],
        work: Some(limits),
        ..Default::default()
    };
    let response = runtime
        .operations
        .call_protobuf::<operations::ListColumnRows>(harmos_protocol::v1::StorageListColumnRowsRequest {
            selection: Some(scan.clone()),
            at: Some(saved_cut.clone()),
            page: Some(harmos_protocol::v1::PageRequest {
                page_size: limits.rows as u32,
                max_bytes: limits.bytes,
                page_token: String::new(),
            }),
        })
        .await
        .unwrap();
    let page = response.page.unwrap();
    assert!(page.buffer_bytes() <= limits.bytes as usize);
    let batch = harmos_storage::query::protobuf::decode_batch(page.ipc, limits.try_into().unwrap()).unwrap();
    assert_eq!(batch.as_record_batch().num_rows(), 1);
    assert_eq!(page.complete, response.next_page_token.is_empty());
    let mut next_page_token = response.next_page_token;
    while !next_page_token.is_empty() {
        let tail = runtime
            .operations
            .call_protobuf::<operations::ListColumnRows>(harmos_protocol::v1::StorageListColumnRowsRequest {
                selection: Some(scan.clone()),
                at: response.resolved_at.clone(),
                page: Some(harmos_protocol::v1::PageRequest {
                    page_size: limits.rows as u32,
                    max_bytes: limits.bytes,
                    page_token: next_page_token,
                }),
            })
            .await
            .unwrap();
        assert_eq!(tail.resolved_at, response.resolved_at);
        let page = tail.page.unwrap();
        assert_eq!(page.complete, tail.next_page_token.is_empty());
        let batch = harmos_storage::query::protobuf::decode_batch(page.ipc, limits.try_into().unwrap()).unwrap();
        assert_eq!(batch.as_record_batch().num_rows(), 0);
        next_page_token = tail.next_page_token;
    }

    let window = storage.mutation_window().unwrap().id.to_string();
    let mutation = |key: &str| Some(wire::MutationId { window: window.clone(), key: key.into() });
    // Register an Ordered Index and Read the Same Cut.
    let index = wire::IndexDefinition {
        name: "sample.values".into(),
        version: 1,
        source: Some(source.clone()),
        columns: vec!["payload.value".into()],
    };
    runtime
        .operations
        .call_protobuf::<operations::SetIndexDefinition>(harmos_protocol::v1::StorageSetIndexDefinitionRequest {
            mutation: mutation("index-0"),
            definition: Some(index.clone()),
            expected_revision: vec![],
            create_if_absent: true,
            rebuild: false,
            work: Some(limits),
        })
        .await
        .unwrap();
    let current = runtime
        .operations
        .call_protobuf::<operations::GetIndexDefinition>(harmos_protocol::v1::StorageGetIndexDefinitionRequest {
            at: None,
            name: index.name.clone(),
            source: index.source.clone(),
        })
        .await
        .unwrap();
    assert_eq!(current.definition.as_ref(), Some(&index));
    let update = harmos_protocol::v1::StorageSetIndexDefinitionRequest {
        mutation: mutation("index-1"),
        definition: Some(index.clone()),
        expected_revision: current.revision.clone(),
        create_if_absent: false,
        rebuild: false,
        work: Some(limits),
    };
    assert_eq!(
        runtime.operations.call_protobuf::<operations::SetIndexDefinition>(update.clone()).await.unwrap().revision,
        current.revision
    );
    let mut absent = update;
    absent.mutation = mutation("index-absent-conflict");
    absent.expected_revision.clear();
    absent.create_if_absent = true;
    assert!(runtime.operations.call_protobuf::<operations::SetIndexDefinition>(absent).await.is_err());
    let index = harmos_protocol::v1::StorageListIndexEntriesRequest {
        at: Some(saved_cut.clone()),
        page: None,
        source: Some(source.clone()),
        selection: Some(wire::IndexSelection {
            name: "sample.values".into(),
            work: Some(limits),
            ..Default::default()
        }),
    };
    let indexed = runtime.operations.call_protobuf::<operations::ListIndexEntries>(index.clone()).await.unwrap().page.unwrap();
    assert_eq!(indexed.entries.len(), 1);
    let mut missing = index;
    missing.selection.as_mut().unwrap().name = "missing".into();
    assert!(runtime.operations.call_protobuf::<operations::ListIndexEntries>(missing).await.is_err());

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
    let revision_bytes = runtime
        .operations
        .call_protobuf::<operations::SetProjectionDefinition>(harmos_protocol::v1::StorageSetProjectionDefinitionRequest {
            mutation: mutation("projection-0"),
            definition: Some(definition),
            expected_revision: vec![],
            create_if_absent: true,
            rebuild: false,
        })
        .await
        .unwrap()
        .revision;
    let current = runtime
        .operations
        .call_protobuf::<operations::GetProjectionDefinition>(harmos_protocol::v1::StorageGetProjectionDefinitionRequest {
            at: None,
            name: "sample.buckets".into(),
            source: Some(source.clone()),
        })
        .await
        .unwrap();
    assert_eq!(current.revision, revision_bytes);
    let projection_cut = current.resolved_at.clone();
    let inspect = harmos_protocol::v1::StorageGetProjectionStatusRequest {
        name: "sample.buckets".into(),
        revision: revision_bytes.clone(),
        at: Some(saved_cut.clone()),
        source: Some(source.clone()),
    };
    assert!(runtime.operations.call_protobuf::<operations::GetProjectionStatus>(inspect.clone()).await.unwrap().status.is_none());
    runtime
        .operations
        .call_protobuf::<operations::AdvanceProjection>(harmos_protocol::v1::StorageAdvanceProjectionRequest {
            snapshot: Some(saved.clone()),
            name: "sample.buckets".into(),
            work: Some(limits),
            source: Some(source.clone()),
        })
        .await
        .unwrap();
    assert_eq!(runtime.operations.call_protobuf::<operations::GetProjectionStatus>(inspect).await.unwrap().status.unwrap().rows, 1);
    let read = harmos_protocol::v1::StorageListProjectionRowsRequest {
        source: Some(source.clone()),
        at: Some(saved_cut.clone()),
        page: None,
        selection: Some(wire::ProjectionSelection {
            name: "sample.buckets".into(),
            revision: revision_bytes.clone(),
            work: Some(limits),
        }),
    };
    let old = runtime.operations.call_protobuf::<operations::ListProjectionRows>(read.clone()).await.unwrap().page.unwrap();
    assert_eq!(old.changed_keys.len(), 1);

    // Correct the Object and Replace the Bucket Without Changing the Saved View.
    let mut correction = create;
    correction.mutation.as_mut().unwrap().key = "correct".into();
    correction.expected_revision = receipt.revision;
    correction.create_if_absent = false;
    correction.value = Some(catalog.encode(&Sample { value: 9.0, ..first }, NonZeroUsize::new(4096).unwrap()).unwrap().into());
    let changed = runtime.operations.call_protobuf::<operations::SetObject>(correction).await.unwrap();
    let harmos_protocol::v1::storage_set_object_response::Outcome::Applied(changed) = changed.outcome.unwrap() else {
        panic!("correction applied")
    };
    let at_old_cut = super::transfer_plugin::call::<operations::context::GetObject>(
        &runtime.operations,
        harmos_protocol::context::StorageGetObjectRequest {
            schema: get.schema.clone(),
            item: get.item.clone(),
            at: Some(issued.clone()),
        },
    )
    .await
    .unwrap();
    assert_eq!(at_old_cut.value, created.value);
    assert_eq!(at_old_cut.resolved_at, Some(issued));
    let changed_position = changed.receipt.as_ref().unwrap().durable_at.as_ref().unwrap();
    let live_lease = runtime
        .operations
        .call_protobuf::<operations::CreateSnapshot>(harmos_protocol::v1::StorageCreateSnapshotRequest { at: None })
        .await
        .unwrap();
    let live_cut = live_lease.resolved_at.unwrap();
    let live = live_lease.snapshot.unwrap();
    assert_eq!(live.at.as_ref().unwrap().sequence, changed_position.sequence);
    runtime
        .operations
        .call_protobuf::<operations::AdvanceProjection>(harmos_protocol::v1::StorageAdvanceProjectionRequest {
            snapshot: Some(live.clone()),
            name: "sample.buckets".into(),
            work: Some(limits),
            source: Some(source),
        })
        .await
        .unwrap();
    let mut latest = read.clone();
    latest.at = Some(live_cut);
    let replacement = runtime.operations.call_protobuf::<operations::ListProjectionRows>(latest).await.unwrap().page.unwrap();
    assert_eq!(replacement.changed_keys, old.changed_keys);
    assert_ne!(replacement.ipc, old.ipc);
    assert_eq!(runtime.operations.call_protobuf::<operations::ListProjectionRows>(read).await.unwrap().page.unwrap(), old);

    let current = super::transfer_plugin::client_call::<operations::GetObject>(&channel, get).await.unwrap();
    assert_eq!(current.value, changed.value);
    assert_eq!(current.revision, changed.revision);

    // Rebuild With a Different Source and Keep the Old Revision's Source Authority.
    let mut alternate = scan.source.clone().unwrap();
    alternate.schema.as_mut().unwrap().id = "test.storage.other".into();
    let rebuilt = wire::ProjectionDefinition {
        name: "sample.buckets".into(),
        version: 2,
        source: Some(alternate.clone()),
        transform: Some(wire::projection_definition::Transform::Select(wire::SelectColumns { columns: vec!["payload.value".into()] })),
    };
    runtime
        .operations
        .call_protobuf::<operations::SetProjectionDefinition>(harmos_protocol::v1::StorageSetProjectionDefinitionRequest {
            mutation: mutation("projection-1"),
            definition: Some(rebuilt),
            expected_revision: revision_bytes.clone(),
            create_if_absent: false,
            rebuild: true,
        })
        .await
        .unwrap();
    let old_definition = runtime
        .operations
        .call_protobuf::<operations::GetProjectionDefinition>(harmos_protocol::v1::StorageGetProjectionDefinitionRequest {
            name: "sample.buckets".into(),
            source: scan.source.clone(),
            at: projection_cut,
        })
        .await
        .unwrap();
    assert_eq!(old_definition.revision, revision_bytes);
    assert_eq!(old_definition.definition.as_ref().unwrap().source, scan.source);
    let old_read = harmos_protocol::v1::StorageListProjectionRowsRequest {
        source: scan.source.clone(),
        at: Some(saved_cut.clone()),
        page: None,
        selection: Some(wire::ProjectionSelection {
            name: "sample.buckets".into(),
            revision: revision_bytes.clone(),
            work: Some(limits),
        }),
    };
    assert_eq!(runtime.operations.call_protobuf::<operations::ListProjectionRows>(old_read).await.unwrap().page.unwrap(), old);

    // Release Both Cuts and Refuse Reusing the Handle.
    runtime
        .operations
        .call_protobuf::<operations::DeleteSnapshot>(harmos_protocol::v1::StorageDeleteSnapshotRequest { snapshot: Some(saved.clone()) })
        .await
        .unwrap();
    assert!(
        runtime
            .operations
            .call_protobuf::<operations::AdvanceProjection>(harmos_protocol::v1::StorageAdvanceProjectionRequest {
                snapshot: Some(saved),
                name: "sample.buckets".into(),
                source: Some(alternate),
                work: Some(limits),
            })
            .await
            .is_err()
    );
    runtime
        .operations
        .call_protobuf::<operations::DeleteSnapshot>(harmos_protocol::v1::StorageDeleteSnapshotRequest { snapshot: Some(live) })
        .await
        .unwrap();
    attached.stop("object journey complete").await.unwrap();
    plugin.stop().await;
    drop(channel);
    listener.detach().await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
}
