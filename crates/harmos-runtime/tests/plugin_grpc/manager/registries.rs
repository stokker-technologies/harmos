/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Registry Journeys

/* === Imports === */

// Standard Library Imports
use std::sync::atomic::Ordering;

// Third Party Imports
use tonic::Code;

// Local Imports
use super::support::{Backends, Host};
use harmos_conformance::host::{operations_client::OperationsClient, schemas_client::SchemasClient, streams_client::StreamsClient};
use harmos_protocol::host::SchemaKey;
use harmos_protocol::host::{OperationsGetRequest, OperationsListRequest, SchemaDescription, SchemasListRequest, StreamsListRequest};
use harmos_runtime::schema::{Described, graph::Schema};
use harmos_runtime::{
    Identifier,
    storage::{RecordInput, StreamInput},
};

/* === Tests === */

#[tokio::test]
async fn operations_get_hides_unavailable_identities_over_grpc() {
    // Start the Product and Confirm the Plugin-Only Operation Is Registered
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let host = Host::start(root.path(), &backends).await;
    host.runtime.operations.definition("harmos.plugin.v1.Time/Get").unwrap();
    let mut operations = OperationsClient::new(host.channel.clone());

    // A Registered Plugin-Only Operation Is Absent from Client Discovery
    let request = OperationsListRequest {};
    let listed = operations.list(request).await.unwrap().into_inner().operations;
    assert!(listed.iter().all(|entry| entry.contract.as_ref().unwrap().id != "harmos.plugin.v1.Time/Get"));
    let request = OperationsGetRequest { id: "harmos.plugin.v1.Time/Get".into() };
    let hidden = operations.get(request).await.unwrap_err();
    assert_eq!(hidden.code(), Code::NotFound);

    // A Missing Operation Has the Same Status and Message
    let request = OperationsGetRequest { id: "missing.operation".into() };
    let missing = operations.get(request).await.unwrap_err();
    assert_eq!(missing.code(), Code::NotFound);
    assert_eq!(missing.message(), hidden.message());

    // Malformed Identities Remain Invalid Arguments
    let request = OperationsGetRequest { id: "malformed identity".into() };
    let malformed = operations.get(request).await.unwrap_err();
    assert_eq!(malformed.code(), Code::InvalidArgument);

    // Stop the Product
    host.close().await;
}

#[tokio::test]
async fn typed_registries_preserve_models_and_hide_private_operations() {
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let host = Host::start(root.path(), &backends).await;
    host.runtime.operations.register_catalog(harmos_operations::host_catalog(), Default::default()).await.unwrap();
    let key = crate::storage::fixture::schema();
    let graph = Schema::from_declaration(u64::DEFINITION, &Default::default()).unwrap();
    let position = host.storage.write_schema(key.clone(), graph.clone()).await.unwrap();
    host.storage
        .append_samples(vec![StreamInput {
            record: RecordInput {
                schema: key.clone(),
                scope: Some(Identifier::new("scope").unwrap()),
                item: Identifier::new("item").unwrap(),
                source: None,
                source_time: None,
                payload: vec![7],
            },
            stream: Identifier::new("stream").unwrap(),
            sequence: 1,
        }])
        .await
        .unwrap();
    host.storage.flush().await.unwrap();

    // The Client Receives Typed Inventories and the Exact Discovered Operation.
    let mut operations = OperationsClient::new(host.channel.clone());
    let discovered = operations.list(OperationsListRequest {}).await.unwrap().into_inner().operations;
    assert!(discovered.iter().all(|entry| !entry.contract.as_ref().unwrap().id.starts_with("storage.")));
    let selected = discovered.iter().find(|entry| entry.contract.as_ref().unwrap().id == "application.host.v1.Schemas/List").unwrap();
    let got = operations
        .get(OperationsGetRequest {
            id: selected.contract.as_ref().unwrap().id.clone(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(got.operation.as_ref(), Some(selected));
    assert!(operations.get(OperationsGetRequest { id: "storage.info".into() }).await.is_err());
    let mut schemas = SchemasClient::new(host.channel.clone());
    let schemas = schemas.list(SchemasListRequest { schema: Some(key.id.to_string()) }).await.unwrap().into_inner().schemas;
    assert_eq!(
        schemas,
        [SchemaDescription {
            position: Some(position.into()),
            key: Some(key.clone().into()),
            schema: Some(graph.into())
        }]
    );
    let mut streams = StreamsClient::new(host.channel.clone());
    let identities = streams.list(StreamsListRequest { schema: Some(key.id.to_string()) }).await.unwrap().into_inner().streams;
    assert_eq!(identities.len(), 1);
    assert_eq!(identities[0].schema, Some(SchemaKey::from(key)));
    assert_eq!(identities[0].scope.as_deref(), Some("scope"));
    assert_eq!(identities[0].item, "item");
    assert_eq!(identities[0].stream, "stream");
    assert!(identities[0].first.is_some());
    assert!(streams.list(StreamsListRequest { schema: Some("absent".into()) }).await.unwrap().into_inner().streams.is_empty());

    // A Revoked Client Cannot Use Previously Discovered Registry Contracts.
    host.policy.0.store(false, Ordering::SeqCst);
    assert!(operations.list(OperationsListRequest {}).await.is_err());
    assert!(streams.list(StreamsListRequest { schema: None }).await.is_err());
    host.close().await;
}
