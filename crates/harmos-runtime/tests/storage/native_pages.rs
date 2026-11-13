/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Continue Native Lists Across Faces At Their Issued Composite Cuts

/* === Imports === */

use super::{Fixture, config};
use harmos_domain::Message;
use harmos_operations::storage::{self as operations, context};
use harmos_protocol::{context as child, storage as wire, v1};
use harmos_storage::{Identifier, PayloadEncoding, RecordInput, SchemaKey};
use std::{
    num::{NonZeroU32, NonZeroUsize},
    sync::{Arc, atomic::Ordering},
};

/* === Structs === */

#[derive(harmos_domain::Message)]
struct Reading {
    value: u64,
}
struct Authority;

/* === Implementations === */

impl harmos_runtime::operations::authorization::Authorizer for Authority {
    fn authorize(
        &self, _: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Authorization>> {
        Ok(Some(harmos_runtime::operations::authorization::Authorization::Restricted(
            harmos_runtime::operations::authorization::Required {
                contract: provider.contract.clone(),
                grants: vec![harmos_operations::plugin::Grant::of(
                    provider.contract.operation().id.to_string(),
                    "records:schema:native-reading",
                )],
            },
        )))
    }
}

/* === Tests === */

#[tokio::test]
async fn native_lists_continue_filtered_pages_and_refuse_changed_revisions() {
    let fixture = Fixture::new(config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let schema = SchemaKey {
        id: Identifier::new("native-reading").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = harmos_domain::schema::graph::Schema::from_declaration(
        <Reading as harmos_domain::schema::Described>::DEFINITION,
        &Default::default(),
    )
    .unwrap();
    fixture.storage.write_schema(schema.clone(), graph).await.unwrap();
    for (item, value) in [("first", 0), ("second", 0), ("third", 2), ("fourth", 3)] {
        fixture
            .storage
            .append_records(vec![RecordInput {
                schema: schema.clone(),
                scope: None,
                item: Identifier::new(item).unwrap(),
                source: None,
                source_time: None,
                payload: Reading { value }.encode(),
            }])
            .await
            .unwrap();
    }
    fixture.storage.flush().await.unwrap();
    fixture.runtime.operations.register_authorizer(Arc::new(Authority)).unwrap();
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
    let source = wire::ColumnSource {
        kind: wire::ColumnKind::Records as i32,
        schema: Some(schema.clone().into()),
    };
    let work = wire::ReadLimits { rows: 64, bytes: 65536 };
    let page = v1::PageRequest {
        page_size: 1,
        max_bytes: 8192,
        page_token: String::new(),
    };
    fixture.data.control.record_page_rows.store(1, Ordering::Relaxed);
    let mut request = v1::StorageListColumnRowsRequest {
        selection: Some(wire::ColumnSelection {
            source: Some(source.clone()),
            columns: vec!["payload.value".into()],
            predicate: Some(wire::ColumnPredicate {
                column: "payload.value".into(),
                lower: Some(2.0),
                upper: None,
            }),
            work: Some(work),
            ..Default::default()
        }),
        at: None,
        page: Some(page.clone()),
    };
    let first = match super::transfer_plugin::client_call::<operations::ListColumnRows>(&channel, request.clone()).await {
        Ok(first) => first,
        Err(error) => panic!(
            "client: {error:?}; native: {:?}",
            fixture.runtime.operations.call_protobuf::<operations::ListColumnRows>(request.clone()).await
        ),
    };
    assert!(prost::Message::encoded_len(&first) as u64 <= request.page.as_ref().unwrap().max_bytes);
    let decoded = harmos_storage::query::protobuf::decode_batch(first.page.unwrap().ipc, work.try_into().unwrap()).unwrap();
    assert_eq!(decoded.as_record_batch().num_rows(), 0);
    assert!(!first.next_page_token.is_empty());
    request.at = first.resolved_at.clone();
    request.page.as_mut().unwrap().page_token = first.next_page_token;
    let mut changed = request.clone();
    changed.selection.as_mut().unwrap().columns = vec!["_storage.item".into()];
    assert!(super::transfer_plugin::client_call::<operations::ListColumnRows>(&channel, changed).await.is_err());
    let mut tampered = request.clone();
    tampered.page.as_mut().unwrap().page_token.push('0');
    assert!(super::transfer_plugin::client_call::<operations::ListColumnRows>(&channel, tampered).await.is_err());
    let mut values = vec![];
    loop {
        let response = super::transfer_plugin::call::<context::ListColumnRows>(
            &fixture.runtime.operations,
            child::StorageListColumnRowsRequest {
                selection: request.selection.clone(),
                at: request.at.clone(),
                page: request.page.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(response.resolved_at, request.at);
        let decoded = harmos_storage::query::protobuf::decode_batch(response.page.unwrap().ipc, work.try_into().unwrap()).unwrap();
        let column = decoded.as_record_batch().column(0).as_any().downcast_ref::<arrow_array::UInt64Array>().unwrap();
        values.extend(column.values().iter().copied());
        if response.next_page_token.is_empty() {
            break;
        }
        request.page.as_mut().unwrap().page_token = response.next_page_token;
    }
    assert_eq!(values, vec![2, 3]);
    fixture.data.control.record_page_rows.store(0, Ordering::Relaxed);
    let window = fixture.storage.mutation_window().unwrap().id.to_string();
    let mutation = |key: &str| Some(wire::MutationId { window: window.clone(), key: key.into() });
    let definition = wire::IndexDefinition {
        name: "ordered".into(),
        version: 1,
        source: Some(source.clone()),
        columns: vec!["payload.value".into()],
    };
    let created = super::transfer_plugin::client_call::<operations::SetIndexDefinition>(
        &channel,
        v1::StorageSetIndexDefinitionRequest {
            definition: Some(definition.clone()),
            expected_revision: vec![],
            create_if_absent: true,
            rebuild: false,
            work: Some(work),
            mutation: mutation("create-index"),
        },
    )
    .await
    .unwrap();
    let mut index_request = v1::StorageListIndexEntriesRequest {
        selection: Some(wire::IndexSelection {
            name: "ordered".into(),
            work: Some(work),
            ..Default::default()
        }),
        source: Some(source.clone()),
        at: None,
        page: Some(page.clone()),
    };
    let indexed = super::transfer_plugin::client_call::<operations::ListIndexEntries>(&channel, index_request.clone()).await.unwrap();
    assert_eq!(indexed.page.unwrap().entries.len(), 1);
    assert!(!indexed.next_page_token.is_empty());
    index_request.at = indexed.resolved_at;
    index_request.page.as_mut().unwrap().page_token = indexed.next_page_token;
    let next = super::transfer_plugin::call::<context::ListIndexEntries>(
        &fixture.runtime.operations,
        child::StorageListIndexEntriesRequest {
            selection: index_request.selection.clone(),
            source: index_request.source.clone(),
            at: index_request.at.clone(),
            page: index_request.page.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(next.resolved_at, index_request.at);
    let mut rebuilt = definition;
    rebuilt.version += 1;
    super::transfer_plugin::client_call::<operations::SetIndexDefinition>(
        &channel,
        v1::StorageSetIndexDefinitionRequest {
            definition: Some(rebuilt),
            expected_revision: created.revision,
            create_if_absent: false,
            rebuild: true,
            work: Some(work),
            mutation: mutation("rebuild-index"),
        },
    )
    .await
    .unwrap();
    let old_index_cut = index_request.at.clone();
    assert!(super::transfer_plugin::client_call::<operations::ListIndexEntries>(&channel, index_request).await.is_err());
    assert!(
        super::transfer_plugin::client_call::<operations::GetIndexDefinition>(
            &channel,
            v1::StorageGetIndexDefinitionRequest {
                name: "ordered".into(),
                source: Some(source.clone()),
                at: old_index_cut,
            }
        )
        .await
        .is_err()
    );
    let definition = wire::ProjectionDefinition {
        name: "projected".into(),
        version: 1,
        source: Some(source.clone()),
        transform: Some(wire::projection_definition::Transform::Select(wire::SelectColumns {
            columns: vec!["payload.value".into(), "_storage.item".into()],
        })),
    };
    let created = super::transfer_plugin::client_call::<operations::SetProjectionDefinition>(
        &channel,
        v1::StorageSetProjectionDefinitionRequest {
            definition: Some(definition),
            expected_revision: vec![],
            create_if_absent: true,
            rebuild: false,
            mutation: mutation("create-projection"),
        },
    )
    .await
    .unwrap();
    let lease = fixture
        .runtime
        .operations
        .call_protobuf::<operations::CreateSnapshot>(v1::StorageCreateSnapshotRequest { at: None })
        .await
        .unwrap();
    let projected = fixture
        .runtime
        .operations
        .call_protobuf::<operations::AdvanceProjection>(v1::StorageAdvanceProjectionRequest {
            snapshot: lease.snapshot.clone(),
            source: Some(source.clone()),
            name: "projected".into(),
            work: Some(work),
        })
        .await
        .unwrap();
    let mut request = v1::StorageListProjectionRowsRequest {
        selection: Some(wire::ProjectionSelection {
            name: "projected".into(),
            revision: created.revision,
            work: Some(work),
        }),
        source: Some(source),
        at: projected.resolved_at.clone(),
        page: Some(page),
    };
    let first = super::transfer_plugin::client_call::<operations::ListProjectionRows>(&channel, request.clone()).await.unwrap();
    assert_eq!(first.resolved_at, projected.resolved_at);
    assert!(!first.next_page_token.is_empty());
    request.page.as_mut().unwrap().page_token = first.next_page_token;
    let next = super::transfer_plugin::call::<context::ListProjectionRows>(
        &fixture.runtime.operations,
        child::StorageListProjectionRowsRequest {
            selection: request.selection,
            source: request.source,
            at: request.at.clone(),
            page: request.page,
        },
    )
    .await
    .unwrap();
    assert_eq!(next.resolved_at, request.at);
    assert_ne!(first.page.unwrap().ipc, next.page.unwrap().ipc);
    fixture
        .runtime
        .operations
        .call_protobuf::<operations::DeleteSnapshot>(v1::StorageDeleteSnapshotRequest { snapshot: lease.snapshot })
        .await
        .unwrap();
    attached.stop("native paging journey complete").await.unwrap();
    plugin.stop().await;
    drop(channel);
    listener.detach().await.unwrap();
    fixture.finish().await;
}
