/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Complete Metadata Mutations After Caller Loss And Replay Them Across Faces

/* === Imports === */

use harmos_operations::storage::{self as operations, context};
use harmos_protocol::{context as child, storage as wire, v1};
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/* === Structs === */

#[derive(harmos_domain::Message)]
struct Reading {
    value: u64,
}
struct AllowMetadata(Arc<AtomicBool>);

/* === Implementations === */

impl harmos_runtime::operations::authorization::Authorizer for AllowMetadata {
    fn authorize(
        &self, _: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Authorization>> {
        let mut scopes = vec!["records:schema:metadata-b"];
        if !self.0.load(Ordering::Acquire) {
            scopes.push("records:schema:metadata-a");
        }
        Ok(Some(harmos_runtime::operations::authorization::Authorization::Restricted(
            harmos_runtime::operations::authorization::Required {
                contract: provider.contract.clone(),
                grants: scopes
                    .into_iter()
                    .map(|scope| harmos_operations::plugin::Grant::of(provider.contract.operation().id.to_string(), scope))
                    .collect(),
            },
        )))
    }
}

/* === Tests === */

#[tokio::test]
async fn definition_receipts_survive_caller_loss_and_preserve_source_authority() {
    let fixture = super::Fixture::new(super::config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let deny_old = Arc::new(AtomicBool::new(false));
    fixture.runtime.operations.register_authorizer(Arc::new(AllowMetadata(deny_old.clone()))).unwrap();
    let plugin = super::transfer_plugin::TransferPlugin::start().await;
    let attached = fixture
        .runtime
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
    let listener = fixture.runtime.grpc().attach(harmos_runtime::grpc::Settings::default()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
    let capabilities =
        super::transfer_plugin::client_call::<operations::Capabilities>(&channel, v1::StorageCapabilitiesRequest {}).await.unwrap();
    let window = capabilities.mutations.unwrap().id;
    let mutation = |key: &str| Some(wire::MutationId { window: window.clone(), key: key.into() });
    let key = |id: &str| harmos_storage::SchemaKey {
        id: harmos_storage::Identifier::new(id).unwrap(),
        version: std::num::NonZeroU32::MIN,
        payload_encoding: harmos_storage::PayloadEncoding::PostcardV1,
    };
    let graph = harmos_domain::schema::graph::Schema::from_declaration(
        <Reading as harmos_domain::schema::Described>::DEFINITION,
        &Default::default(),
    )
    .unwrap();
    fixture.storage.write_schema(key("metadata-a"), graph.clone()).await.unwrap();
    fixture.storage.write_schema(key("metadata-b"), graph).await.unwrap();
    let source_a = wire::ColumnSource {
        kind: wire::ColumnKind::Records as i32,
        schema: Some(key("metadata-a").into()),
    };
    let source_b = wire::ColumnSource {
        kind: wire::ColumnKind::Records as i32,
        schema: Some(key("metadata-b").into()),
    };
    let create = v1::StorageSetIndexDefinitionRequest {
        definition: Some(wire::IndexDefinition {
            name: "ordered".into(),
            version: 1,
            source: Some(source_a.clone()),
            columns: vec!["payload.value".into()],
        }),
        expected_revision: vec![],
        create_if_absent: true,
        rebuild: false,
        work: Some(wire::ReadLimits { rows: 16, bytes: 65536 }),
        mutation: mutation("create-index"),
    };
    fixture.data.control.definition_lost_replies.store(1, Ordering::Release);
    let first = super::transfer_plugin::client_call::<operations::SetIndexDefinition>(&channel, create.clone()).await.unwrap();
    let replay = super::transfer_plugin::call::<context::SetIndexDefinition>(
        &fixture.runtime.operations,
        child::StorageSetIndexDefinitionRequest {
            definition: create.definition.clone(),
            expected_revision: create.expected_revision.clone(),
            create_if_absent: true,
            rebuild: false,
            work: create.work,
            mutation: create.mutation.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(replay.definition, first.definition);
    assert_eq!(replay.revision, first.revision);
    assert_eq!(replay.receipt, first.receipt);
    let mut changed = create.clone();
    changed.definition.as_mut().unwrap().version += 1;
    assert!(super::transfer_plugin::client_call::<operations::SetIndexDefinition>(&channel, changed).await.is_err());
    let mut update = create.clone();
    update.definition.as_mut().unwrap().version += 1;
    update.expected_revision = first.revision.clone();
    update.create_if_absent = false;
    update.rebuild = true;
    update.mutation = mutation("update-index");
    fixture.data.control.block_definition.store(true, Ordering::Release);
    let dropped_channel = channel.clone();
    let dropped_request = update.clone();
    let caller = tokio::spawn(async move {
        super::transfer_plugin::client_call::<operations::SetIndexDefinition>(&dropped_channel, dropped_request).await
    });
    fixture.data.control.definition_entered.notified().await;
    caller.abort();
    let _ = caller.await;
    fixture.data.control.block_definition.store(false, Ordering::Release);
    fixture.data.control.definition_release.notify_waiters();
    let completed = super::transfer_plugin::client_call::<operations::SetIndexDefinition>(&channel, update.clone()).await.unwrap();
    assert_ne!(completed.revision, first.revision);
    assert_eq!(super::transfer_plugin::client_call::<operations::SetIndexDefinition>(&channel, update).await.unwrap(), completed);
    let mut move_source = create.clone();
    move_source.definition.as_mut().unwrap().source = Some(source_b.clone());
    move_source.definition.as_mut().unwrap().version += 2;
    move_source.expected_revision = completed.revision.clone();
    move_source.create_if_absent = false;
    move_source.rebuild = true;
    move_source.mutation = mutation("move-source");
    deny_old.store(true, Ordering::Release);
    assert!(super::transfer_plugin::client_call::<operations::SetIndexDefinition>(&channel, move_source.clone()).await.is_err());
    deny_old.store(false, Ordering::Release);
    let current = super::transfer_plugin::client_call::<operations::GetIndexDefinition>(
        &channel,
        v1::StorageGetIndexDefinitionRequest {
            name: "ordered".into(),
            source: Some(source_a.clone()),
        },
    )
    .await
    .unwrap();
    assert_eq!(current.revision, completed.revision);
    assert_eq!(current.definition, completed.definition);
    let moved = super::transfer_plugin::client_call::<operations::SetIndexDefinition>(&channel, move_source).await.unwrap();
    assert_ne!(moved.revision, completed.revision);
    // The Old Successful Request Still Replays After A Later Revision And Source Change.
    let replay = super::transfer_plugin::client_call::<operations::SetIndexDefinition>(&channel, create).await.unwrap();
    assert_eq!(replay, first);
    let projection = child::StorageSetProjectionDefinitionRequest {
        definition: Some(wire::ProjectionDefinition {
            name: "projected".into(),
            version: 1,
            source: Some(source_b),
            transform: Some(wire::projection_definition::Transform::Select(wire::SelectColumns { columns: vec!["payload.value".into()] })),
        }),
        expected_revision: vec![],
        create_if_absent: true,
        rebuild: false,
        mutation: mutation("create-projection"),
    };
    let first =
        super::transfer_plugin::call::<context::SetProjectionDefinition>(&fixture.runtime.operations, projection.clone()).await.unwrap();
    let replay = super::transfer_plugin::client_call::<operations::SetProjectionDefinition>(
        &channel,
        v1::StorageSetProjectionDefinitionRequest {
            definition: projection.definition,
            expected_revision: projection.expected_revision,
            create_if_absent: true,
            rebuild: false,
            mutation: projection.mutation,
        },
    )
    .await
    .unwrap();
    assert_eq!(first.definition, replay.definition);
    assert_eq!(first.revision, replay.revision);
    assert_eq!(first.receipt, replay.receipt);
    assert!(replay.receipt.unwrap().completed);
    attached.stop("metadata story complete").await.unwrap();
    plugin.stop().await;
    drop(channel);
    listener.detach().await.unwrap();
    fixture.finish().await;
}
