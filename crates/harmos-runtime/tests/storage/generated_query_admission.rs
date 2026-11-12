/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Retain IPC Ownership Until Destination Admission and Consumption.

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;
use prost::Message as _;

/* === Tests === */

#[tokio::test]
async fn ipc_admission_retains() {
    let fixture = Fixture::new(config()).await;
    let schema = SchemaKey {
        id: Identifier::new("admitted-query").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = harmos_runtime::schema::graph::Schema::from_declaration(
        <Reading as harmos_runtime::schema::Described>::DEFINITION,
        &harmos_runtime::config::SchemaSettings::default(),
    )
    .unwrap();
    fixture.storage.write_schema(schema.clone(), graph).await.unwrap();
    fixture
        .storage
        .write_records(vec![RecordInput {
            schema: schema.clone(),
            scope: None,
            item: Identifier::new("row").unwrap(),
            source: None,
            source_time: None,
            payload: Reading { value: 7 }.encode(),
        }])
        .await
        .unwrap();
    fixture.storage.flush().await.unwrap();
    for cancel in [true, false] {
        let mut reader = fixture
            .storage
            .query(harmos_runtime::storage::Query {
                at: fixture.storage.cursor_saved(),
                source: Source::Records(RecordSelection {
                    schema: schema.clone(),
                    scope: None,
                    item: None,
                    source_start: None,
                    source_end: None,
                }),
                transforms: Vec::new(),
                slice: RowSlice::default(),
            })
            .await
            .unwrap();
        let item = reader.next().await.unwrap().unwrap();
        // Isolate The Pending Output From The Reader.
        drop(reader);
        fixture.data.snapshots_released().await;
        let bytes = 4 * 1024 * 1024;
        let (sender, receiver) = harmos_runtime::operations::stream::<Vec<u8>>(harmos_runtime::operations::StreamSettings {
            items: NonZeroUsize::MIN,
            bytes: NonZeroUsize::new(bytes).unwrap(),
        })
        .unwrap();
        let blocked = sender.reserve(bytes).await.unwrap();
        let converted = std::cell::Cell::new(false);
        let mut sending = Box::pin(sender.send_with(item.output_memory_size(), || {
            converted.set(true);
            Ok(item.output().encode_to_vec())
        }));
        assert!(
            std::future::poll_fn(|context| std::task::Poll::Ready(std::future::Future::poll(sending.as_mut(), context).is_pending())).await
        );
        assert!(!converted.get());
        drop(blocked);
        if cancel {
            drop(sending);
            assert!(!converted.get());
        } else {
            sending.await.unwrap();
            assert!(converted.get());
            let output = receiver.recv().await.unwrap();
            let decoded = harmos_protocol::storage::QueryOutput::decode(output.as_slice()).unwrap();
            assert!(matches!(decoded.output, Some(harmos_protocol::storage::query_output::Output::Schema(_))));
            // Destination Admission Now Covers The Wire Bytes Until Consumption.
            let mut next = Box::pin(sender.reserve(bytes));
            assert!(
                std::future::poll_fn(|context| std::task::Poll::Ready(std::future::Future::poll(next.as_mut(), context).is_pending()))
                    .await
            );
            drop(output);
            drop(next.await.unwrap());
            continue;
        }
    }
    fixture.finish().await;
}
