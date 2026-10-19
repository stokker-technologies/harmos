/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;

// Third Party Imports
use harmos_runtime::Identifier;
use harmos_runtime::schema::Cardinality;
use harmos_runtime::schema::graph::{Field, Kind, Schema};
use harmos_runtime::storage::operations::Query as QueryRequest;
use harmos_runtime::storage::operations::{Query, WriteSchema};
use harmos_runtime::storage::query::{QueryArrow, RowSlice, SchemaSelection, Source};
use harmos_runtime::storage::{MutationId, SchemaInput, SchemaKey};

// Local Imports
use super::{Fixture, Lanes, config};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn schema(name: &str) -> SchemaInput {
    SchemaInput::new(
        SchemaKey {
            payload_encoding: harmos_runtime::storage::PayloadEncoding::PostcardV1,
            id: Identifier::new(name).unwrap(),
            version: NonZeroU32::MIN,
        },
        &Schema {
            root: 0,
            definitions: vec![
                vec![Field {
                    name: Cow::Borrowed("rows"),
                    tag: 1,
                    cardinality: Cardinality::Repeated,
                    kind: Kind::Message { definition: 1 },
                }],
                vec![
                    Field { name: Cow::Borrowed("value"), tag: 1, cardinality: Cardinality::Required, kind: Kind::F64 },
                    Field {
                        name: Cow::Borrowed("next"),
                        tag: 2,
                        cardinality: Cardinality::Optional,
                        kind: Kind::Message { definition: 1 },
                    },
                ],
            ],
        },
    )
    .unwrap()
}
fn request(f: &Fixture, name: &str) -> WriteSchema {
    WriteSchema {
        mutation: MutationId { window: f.storage.mutation_window().unwrap().id, key: Identifier::new(name).unwrap() },
        schema: schema(name),
    }
}
async fn setup() -> Fixture {
    let f = Fixture::new(config()).await;
    harmos_runtime::storage::operations::register(&f.runtime.operations, "storage", Lanes::Open, Arc::new(f.storage.clone())).unwrap();
    f
}

/* === Tests === */

#[tokio::test]
async fn schema_registry_roundtrip() {
    let f = setup().await;
    let request = request(&f, "nested");
    let accepted = f.runtime.operations.call(request.clone()).await.unwrap();
    let repeated = f.runtime.operations.call(request.clone()).await.unwrap();
    assert_eq!(*accepted, *repeated);
    assert_eq!(f.storage.cursor_accepted(), *accepted);
    let mut conflict = request.clone();
    conflict.schema.key.version = NonZeroU32::new(2).unwrap();
    assert!(f.runtime.operations.call(conflict).await.is_err());
    let at = f.storage.flush().await.unwrap();
    let query = QueryRequest {
        at: at.clone(),
        source: Source::Schemas(SchemaSelection { schema: Some(request.schema.key.clone()) }),
        transforms: vec![],
        slice: RowSlice { offset: 0, limit: None },
    };
    let mut response = f
        .runtime
        .operations
        .download::<Query>(
            query,
            harmos_runtime::operations::StreamSettings { items: NonZeroUsize::MIN, bytes: NonZeroUsize::new(64 * 1024).unwrap() },
        )
        .unwrap();
    let mut announced = false;
    let mut rows = 0;
    while let Some(output) = response.next().await.unwrap() {
        match &QueryArrow::try_from(&*output).unwrap() {
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
    let completed = response.join().await.unwrap();
    assert_eq!(completed.at, at);
    assert_eq!(completed.rows, 1);
    assert_eq!(rows, 1);
    let persisted = f.data.schemas.lock().unwrap().values().next().unwrap().clone();
    assert_eq!(persisted.schema, *request.schema.graph());
    let encoded = serde_json::to_value(&persisted).unwrap();
    let decoded: harmos_runtime::storage::StorageSchema = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, persisted);
    f.finish().await;
}
