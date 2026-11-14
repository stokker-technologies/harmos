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

use super::support::{Backends, Host};
use harmos_protocol::{client, v1};
use harmos_runtime::{
    Identifier,
    storage::{RecordInput, StreamInput},
};
use std::sync::atomic::Ordering;

/* === Tests === */

#[tokio::test]
async fn typed_registries_preserve_models_and_hide_private_operations() {
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let host = Host::start(root.path(), &backends).await;
    host.runtime.operations.register_catalog(harmos_operations::host_catalog(), Default::default()).await.unwrap();
    let key = crate::storage::fixture::schema();
    let graph = harmos_runtime::schema::graph::Schema::from_declaration(
        <u64 as harmos_runtime::schema::Described>::DEFINITION,
        &Default::default(),
    )
    .unwrap();
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

    // The Client Receives Typed Inventories And The Exact Discovered Operation.
    let mut operations = client::operations_client::OperationsClient::new(host.channel.clone());
    let discovered = operations.list(client::OperationsListRequest {}).await.unwrap().into_inner().operations;
    assert!(discovered.iter().all(|entry| !entry.contract.as_ref().unwrap().id.starts_with("storage.")));
    let selected = discovered.iter().find(|entry| entry.contract.as_ref().unwrap().id == "harmos.client.v1.Schemas/List").unwrap();
    let got = operations
        .get(client::OperationsGetRequest {
            id: selected.contract.as_ref().unwrap().id.clone(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(got.operation.as_ref(), Some(selected));
    assert!(operations.get(client::OperationsGetRequest { id: "storage.info".into() }).await.is_err());
    let mut schemas = client::schemas_client::SchemasClient::new(host.channel.clone());
    let schemas = schemas.list(client::SchemasListRequest { schema: Some(key.id.to_string()) }).await.unwrap().into_inner().schemas;
    assert_eq!(
        schemas,
        [client::SchemaDescription {
            position: Some(position.into()),
            key: Some(key.clone().into()),
            schema: Some(graph.into())
        }]
    );
    let mut streams = client::streams_client::StreamsClient::new(host.channel.clone());
    let identities = streams.list(client::StreamsListRequest { schema: Some(key.id.to_string()) }).await.unwrap().into_inner().streams;
    assert_eq!(identities.len(), 1);
    assert_eq!(identities[0].schema, Some(v1::SchemaKey::from(key)));
    assert_eq!(identities[0].scope.as_deref(), Some("scope"));
    assert_eq!(identities[0].item, "item");
    assert_eq!(identities[0].stream, "stream");
    assert!(identities[0].first.is_some());
    assert!(streams.list(client::StreamsListRequest { schema: Some("absent".into()) }).await.unwrap().into_inner().streams.is_empty());

    // A Revoked Client Cannot Use Previously Discovered Registry Contracts.
    host.policy.0.store(false, Ordering::SeqCst);
    assert!(operations.list(client::OperationsListRequest {}).await.is_err());
    assert!(streams.list(client::StreamsListRequest { schema: None }).await.is_err());
    host.close().await;
}
