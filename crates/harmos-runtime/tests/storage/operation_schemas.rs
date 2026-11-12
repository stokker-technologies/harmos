/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
use std::num::NonZeroU32;

// Third Party Imports

// Local Imports
use super::{Fixture, config};
use harmos_operations::storage::ListRows;
use harmos_runtime::Identifier;
use harmos_runtime::schema::Cardinality;
use harmos_runtime::schema::graph::{Field, Kind, Schema};
use harmos_runtime::storage::query::{QueryArrow, RowSlice, SchemaSelection, Source};
use harmos_runtime::storage::{MutationId, SchemaKey};

/* === Private Functions === */

fn schema(name: &str) -> harmos_protocol::storage::SchemaInput {
    harmos_protocol::storage::SchemaInput {
        key: Some(
            SchemaKey {
                payload_encoding: harmos_runtime::storage::PayloadEncoding::PostcardV1,
                id: Identifier::new(name).unwrap(),
                version: NonZeroU32::MIN,
            }
            .into(),
        ),
        graph: Some(
            Schema {
                representations: ::std::collections::BTreeMap::from([(
                    0,
                    harmos_runtime::schema::wire::Representation {
                        omissions: ::std::collections::BTreeMap::from([("rows".into(), harmos_runtime::schema::wire::Omission::Empty)]),
                        ..Default::default()
                    },
                )]),
                root: 0,
                definitions: vec![
                    vec![Field {
                        name: Cow::Borrowed("rows"),
                        tag: 1,
                        cardinality: Cardinality::Repeated,
                        kind: Kind::Message { definition: 1 },
                    }],
                    vec![
                        Field {
                            name: Cow::Borrowed("value"),
                            tag: 1,
                            cardinality: Cardinality::Required,
                            kind: Kind::F64,
                        },
                        Field {
                            name: Cow::Borrowed("next"),
                            tag: 2,
                            cardinality: Cardinality::Optional,
                            kind: Kind::Message { definition: 1 },
                        },
                    ],
                ],
            }
            .into(),
        ),
    }
}

fn request(f: &Fixture, name: &str) -> harmos_protocol::storage::WriteSchema {
    harmos_protocol::storage::WriteSchema {
        mutation: Some(
            MutationId {
                window: f.storage.mutation_window().unwrap().id,
                key: Identifier::new(name).unwrap(),
            }
            .into(),
        ),
        schema: Some(schema(name)),
    }
}

async fn setup() -> Fixture {
    let f = Fixture::new(config()).await;
    crate::support::app::register(&f.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    f
}

#[tokio::test]
async fn schema_registry_roundtrip() {
    let f = setup().await;
    let request = request(&f, "nested");
    let accepted = f.runtime.operations.call_protobuf::<harmos_operations::storage::WriteSchema>(request.clone()).await.unwrap();
    let repeated = f.runtime.operations.call_protobuf::<harmos_operations::storage::WriteSchema>(request.clone()).await.unwrap();
    assert_eq!(accepted, repeated);
    assert_eq!(harmos_protocol::storage::StorageCursor::from(f.storage.cursor_accepted()), accepted);
    let mut conflict = request.clone();
    conflict.schema.as_mut().unwrap().key.as_mut().unwrap().version = 2;
    assert!(f.runtime.operations.call_protobuf::<harmos_operations::storage::WriteSchema>(conflict).await.is_err());
    let mut malformed = request.clone();
    malformed.mutation.as_mut().unwrap().key = "malformed".into();
    malformed.schema.as_mut().unwrap().graph.as_mut().unwrap().root = u64::MAX;
    assert!(f.runtime.operations.call_protobuf::<harmos_operations::storage::WriteSchema>(malformed).await.is_err());
    assert_eq!(harmos_protocol::storage::StorageCursor::from(f.storage.cursor_accepted()), accepted);
    let at = f.storage.flush().await.unwrap();
    let query = harmos_runtime::storage::Query {
        at: at.clone(),
        source: Source::Schemas(SchemaSelection {
            schema: Some(request.schema.as_ref().unwrap().key.clone().unwrap().try_into().unwrap()),
        }),
        transforms: vec![],
        slice: RowSlice { offset: 0, limit: None },
    };
    let query: harmos_protocol::storage::Query = query.try_into().unwrap();
    let response = f
        .runtime
        .operations
        .call_protobuf::<ListRows>(harmos_protocol::v1::StorageListRowsRequest {
            source: query.source,
            at: None,
            transforms: query.transforms,
            page: None,
        })
        .await
        .unwrap();
    assert_eq!(response.resolved_at.as_ref().unwrap().positions[0].sequence, at.sequence);
    assert!(response.next_page_token.is_empty());
    let mut announced = false;
    let mut rows = 0;
    for output in response.rows {
        match &QueryArrow::try_from(output).unwrap() {
            QueryArrow::Schema(_) => {
                assert!(!announced);
                announced = true;
            }
            QueryArrow::Batch(batch) => {
                assert!(announced);
                rows += batch.as_record_batch().num_rows();
            }
        }
    }
    assert_eq!(rows, 1);
    let persisted = f.data.schemas.lock().unwrap().values().next().unwrap().clone();
    let expected: Schema = request.schema.unwrap().graph.unwrap().try_into().unwrap();
    assert_eq!(*persisted.schema, expected);
    let encoded = serde_json::to_value(&persisted).unwrap();
    let decoded: harmos_runtime::storage::StorageSchema = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, persisted);
    f.finish().await;
}
