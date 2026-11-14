/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports
use arrow_array::{Array, ArrayRef, RecordBatch, StringArray, StructArray, UInt64Array};
use arrow_schema::{DataType, Field, Fields, Schema};

// Local Imports
use super::Fixture;
use harmos_query::arrow::ArrowBatch;
use harmos_runtime::Identifier;
use harmos_storage::writes::DataBatch;

/* === Public Functions === */

pub(super) fn batch(sequence: u64, destination: &str) -> DataBatch {
    let fields: Fields = vec![Field::new("value", DataType::UInt64, false)].into();
    let payload = StructArray::new(fields, vec![Arc::new(UInt64Array::from(vec![7, 11]))], None);
    let items = vec!["first", "second"];
    let data = envelope(payload, &items, &[0, 1]);
    DataBatch {
        sequence,
        destination: Identifier::new(destination).unwrap(),
        schema: super::super::schema(),
        data: data.into(),
    }
}

pub(super) fn envelope(payload: StructArray, items: &[&str], sequences: &[u64]) -> ArrowBatch {
    let metadata_fields: Fields = vec![Field::new("item", DataType::Utf8, false), Field::new("sequence", DataType::UInt64, false)].into();
    let metadata = StructArray::new(
        metadata_fields,
        vec![Arc::new(StringArray::from(items.to_vec())), Arc::new(UInt64Array::from(sequences.to_vec()))],
        None,
    );
    let fields =
        vec![Field::new("_storage", metadata.data_type().clone(), false), Field::new("payload", payload.data_type().clone(), false)];
    let columns: Vec<ArrayRef> = vec![Arc::new(metadata), Arc::new(payload)];
    ArrowBatch::try_new(RecordBatch::try_new(Arc::new(Schema::new(fields)), columns).unwrap()).unwrap()
}

pub(super) async fn seed(f: &Fixture) {
    let graph = harmos_runtime::schema::graph::Schema {
        representations: Default::default(),
        root: 0,
        definitions: vec![vec![harmos_runtime::schema::graph::Field {
            name: "value".into(),
            tag: 1,
            kind: harmos_runtime::schema::graph::Kind::U64,
            cardinality: harmos_runtime::schema::Cardinality::Required,
        }]],
    };
    f.storage.write_schema(super::super::schema(), graph.clone()).await.unwrap();
    f.storage.flush().await.unwrap();
}
