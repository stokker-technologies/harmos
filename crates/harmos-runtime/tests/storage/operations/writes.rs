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
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports
use arrow_array::{Array, ArrayRef, RecordBatch, StringArray, StructArray, UInt64Array};
use arrow_schema::{DataType, Field, Fields, Schema};
use harmos_operations::storage::{WriteRecords, WriteStreams};
use harmos_runtime::operations::StreamSettings;
use harmos_runtime::storage::arrow::ArrowBatch;
use harmos_runtime::storage::writes::{DataBatch, WriteReceipt};
use harmos_runtime::{Identifier, MemorySize, Message};

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

pub(super) fn settings() -> StreamSettings {
    StreamSettings {
        items: NonZeroUsize::new(4).unwrap(),
        bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
    }
}

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

#[tokio::test]
async fn record_exchange_receipts() {
    let f = Fixture::new(config()).await;
    f.runtime
        .operations
        .register(harmos_runtime::operations::RegistrationExt::lane(
            harmos_runtime::builtin::storage_registration(Arc::new(f.storage.clone())).unwrap(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    seed(&f).await;
    let mut records = f
        .runtime
        .operations
        .exchange::<WriteRecords>(harmos_operations::storage::WriteRecords { scope: None }, settings(), settings())
        .unwrap();
    let first = batch(1, "bench");
    records.send_with(first.memory_size(), || Ok(first)).await.unwrap();
    records.finish_input();
    let mut accepted = None;
    let mut durable = None;
    while let Some(receipt) = records.next().await.unwrap() {
        match &*receipt {
            WriteReceipt::Accepted(receipt) => {
                assert!(accepted.is_none());
                accepted = Some(receipt.clone());
            }
            WriteReceipt::Durable(receipt) => {
                assert_eq!(accepted.as_ref(), Some(receipt));
                durable = Some(receipt.clone());
            }
        }
    }
    let completed = records.join().await.unwrap();
    assert_eq!(completed.batches, 1);
    assert_eq!(completed.cursor, durable.unwrap().cursor);
    assert_eq!(f.data.records.lock().unwrap().len(), 2);
    f.finish().await;
}

#[tokio::test]
async fn cancelled_exchange_retains() {
    let f = Fixture::new(config()).await;
    f.runtime
        .operations
        .register(harmos_runtime::operations::RegistrationExt::lane(
            harmos_runtime::builtin::storage_registration(Arc::new(f.storage.clone())).unwrap(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    seed(&f).await;

    // Cancelling Delivery Does Not Retract Storage-Owned Samples.
    let mut stream = f
        .runtime
        .operations
        .exchange::<WriteStreams>(harmos_operations::storage::WriteStreams { scope: None }, settings(), settings())
        .unwrap();
    let first = batch(1, "voltage");
    stream.send(first).await.unwrap();
    let accepted = loop {
        let receipt = stream.next().await.unwrap().unwrap();
        if let WriteReceipt::Accepted(receipt) = &*receipt {
            break receipt.cursor.clone();
        }
    };
    stream.cancel_and_settle().await.unwrap();
    f.storage.flush_until(&accepted).await.unwrap();
    assert_eq!(f.data.rows.lock().unwrap().len(), 2);
    assert_eq!(f.storage.cursor_saved(), accepted);
    f.finish().await;
}

#[test]
fn batch_descriptor_roundtrips() {
    // Portable Batch Descriptors Preserve Identity And Native Arrow Values.
    let original = batch(42, "roundtrip");
    let encoded = original.encode();
    assert!(DataBatch::decoding_memory(&encoded).unwrap() >= encoded.len());
    let decoded = DataBatch::decode(&encoded).unwrap();
    assert_eq!(decoded.sequence, 42);
    assert_eq!(decoded.destination, original.destination);
    assert_eq!(decoded.data, original.data);
}

/* === Private Functions === */

/* === Tests === */
