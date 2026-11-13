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

fn schema_request(f: &Fixture, name: &str) -> harmos_protocol::storage::StorageSetSchemaRequest {
    harmos_protocol::storage::StorageSetSchemaRequest {
        mutation: Some(
            MutationId {
                window: f.storage.mutation_window().unwrap().id,
                key: Identifier::new(name).unwrap(),
            }
            .into(),
        ),
        schema: Some(schema(name)),
        expected_revision: Vec::new(),
        create_if_absent: true,
    }
}

fn assert_conflict(error: harmos_runtime::Error) {
    let harmos_runtime::Error::OperationFailed { operation, source } = error else { panic!("unexpected failure: {error:?}") };
    assert_eq!(operation, "storage.set_schema");
    let Some(harmos_operations::execution::ServingError::Handler(failure)) = source.downcast_ref() else {
        panic!("missing typed failure: {source:?}");
    };
    assert_eq!(failure.detail("storage.kind"), Some("STORAGE_FAILURE_CONFLICT"));
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
    let request = schema_request(&f, "nested");
    let accepted = f.runtime.operations.call_protobuf::<harmos_operations::storage::SetSchema>(request.clone()).await.unwrap();
    let repeated = f.runtime.operations.call_protobuf::<harmos_operations::storage::SetSchema>(request.clone()).await.unwrap();
    assert_eq!(accepted, repeated);
    assert_eq!(accepted.receipt.as_ref().unwrap().durable_at.as_ref().unwrap().sequence, f.storage.cursor_accepted().sequence);
    let mut conflict = request.clone();
    conflict.schema.as_mut().unwrap().key.as_mut().unwrap().version = 2;
    assert!(f.runtime.operations.call_protobuf::<harmos_operations::storage::SetSchema>(conflict).await.is_err());
    let mut malformed = request.clone();
    malformed.mutation.as_mut().unwrap().key = "malformed".into();
    malformed.schema.as_mut().unwrap().graph.as_mut().unwrap().root = u64::MAX;
    assert!(f.runtime.operations.call_protobuf::<harmos_operations::storage::SetSchema>(malformed).await.is_err());
    assert_eq!(accepted.receipt.as_ref().unwrap().durable_at.as_ref().unwrap().sequence, f.storage.cursor_accepted().sequence);
    let get = harmos_protocol::storage::StorageGetSchemaRequest {
        key: request.schema.as_ref().unwrap().key.clone(),
        at: None,
    };
    let first = f.runtime.operations.call_protobuf::<harmos_operations::storage::GetSchema>(get.clone()).await.unwrap();
    assert!(first.exists);
    assert_eq!(first.revision, accepted.revision);
    let mut stale = request.clone();
    stale.mutation.as_mut().unwrap().key = "stale-create".into();
    let refused = f.runtime.operations.call_protobuf::<harmos_operations::storage::SetSchema>(stale.clone()).await.unwrap_err();
    assert_conflict(refused);
    let mut replacement = request.clone();
    replacement.mutation.as_mut().unwrap().key = "same-graph".into();
    replacement.create_if_absent = false;
    replacement.expected_revision = first.revision.clone();
    let next = f.runtime.operations.call_protobuf::<harmos_operations::storage::SetSchema>(replacement.clone()).await.unwrap();
    assert_ne!(next.revision, first.revision);
    assert_eq!(f.runtime.operations.call_protobuf::<harmos_operations::storage::SetSchema>(request.clone()).await.unwrap(), accepted);
    assert_conflict(f.runtime.operations.call_protobuf::<harmos_operations::storage::SetSchema>(stale).await.unwrap_err());
    let mut changed = replacement.clone();
    changed.mutation.as_mut().unwrap().key = "changed-graph".into();
    changed.expected_revision = next.revision.clone();
    changed.schema.as_mut().unwrap().graph.as_mut().unwrap().definitions[1].fields[0].name = "different".into();
    assert_conflict(f.runtime.operations.call_protobuf::<harmos_operations::storage::SetSchema>(changed).await.unwrap_err());
    let old = f
        .runtime
        .operations
        .call_protobuf::<harmos_operations::storage::GetSchema>(harmos_protocol::storage::StorageGetSchemaRequest {
            at: first.resolved_at.clone(),
            ..get.clone()
        })
        .await
        .unwrap();
    assert_eq!(old.revision, first.revision);
    let latest = f.runtime.operations.call_protobuf::<harmos_operations::storage::GetSchema>(get).await.unwrap();
    assert_eq!(latest.revision, next.revision);
    let mut version = schema_request(&f, "nested-v2");
    version.schema = request.schema.clone();
    version.schema.as_mut().unwrap().key.as_mut().unwrap().version = 2;
    version.schema.as_mut().unwrap().graph.as_mut().unwrap().definitions[1].fields[0].name = "new-value".into();
    assert!(f.runtime.operations.call_protobuf::<harmos_operations::storage::SetSchema>(version).await.unwrap().receipt.unwrap().completed);
    // The admitted owner survives loss of its caller before the provider commits.
    use std::sync::atomic::Ordering;
    f.data.control.block_write.store(true, Ordering::SeqCst);
    let lost = schema_request(&f, "lost-schema-reply");
    {
        let mut call = Box::pin(f.runtime.operations.call_protobuf::<harmos_operations::storage::SetSchema>(lost.clone()));
        assert!(crate::support::events::until(call.as_mut(), f.data.control.write_entered.notified()).await.is_ok());
    }
    f.data.control.block_write.store(false, Ordering::SeqCst);
    f.data.control.write_release.notify_one();
    let settled = f.runtime.operations.call_protobuf::<harmos_operations::storage::SetSchema>(lost.clone()).await.unwrap();
    assert!(settled.receipt.unwrap().completed);
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
    let mut listing = harmos_protocol::v1::StorageListRowsRequest {
        source: query.source,
        at: None,
        transforms: query.transforms,
        page: None,
    };
    let mut announced = false;
    let mut rows = 0;
    loop {
        let response = f.runtime.operations.call_protobuf::<ListRows>(listing.clone()).await.unwrap();
        assert_eq!(response.resolved_at.as_ref().unwrap().positions[0].sequence, at.sequence);
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
        if response.next_page_token.is_empty() {
            break;
        }
        listing.page = Some(harmos_protocol::v1::PageRequest {
            page_token: response.next_page_token,
            ..Default::default()
        });
    }
    assert_eq!(rows, 2);
    let persisted = f.data.schemas.lock().unwrap().values().next().unwrap().clone();
    let expected: Schema = request.schema.unwrap().graph.unwrap().try_into().unwrap();
    assert_eq!(*persisted.schema, expected);
    let encoded = serde_json::to_value(&persisted).unwrap();
    let decoded: harmos_runtime::storage::StorageSchema = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, persisted);
    f.finish().await;
}
