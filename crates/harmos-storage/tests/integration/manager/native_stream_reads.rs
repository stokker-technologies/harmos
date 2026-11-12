/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use super::nominal_fixture::*;

/* === Tests === */

#[tokio::test]
async fn native_stream_reads_preserve_snapshot_and_cursor() {
    let fixture = Nominal::new().await;
    // Read The Same Snapshot Through Its Query And Native Interfaces
    let read = batches(
        &fixture.storage,
        fixture.query(fixture.period(), vec![project(&[("_storage.position", "position"), ("payload.ordinal", "ordinal")])]),
    )
    .await;
    assert!(read.schema_first);
    assert_eq!(read.schemas, 1);
    let expected = positions(&read);
    assert_eq!(read.completed_rows, expected.len() as u64);
    let mut reader = fixture
        .storage
        .list_samples(StreamRead {
            at: fixture.durable.clone(),
            selection: fixture.period(),
        })
        .await
        .unwrap();
    let mut actual = Vec::new();
    while let Some(page) = reader.next().await.unwrap() {
        assert!(page.len() <= storage_settings().response.batch_rows.get());
        for row in page.iter() {
            assert_eq!(row.record.schema, fixture.schema);
            actual.push((row.record.position.sequence, row.sequence));
        }
    }
    assert_eq!(actual, expected);
    assert!(reader.next().await.unwrap().is_none());
    let after = actual[actual.len() / 2].0;
    let mut selection = fixture.period();
    selection.after = after;
    let mut reader = fixture.storage.list_samples(StreamRead { at: fixture.durable.clone(), selection }).await.unwrap();
    let mut resumed = Vec::new();
    while let Some(page) = reader.next().await.unwrap() {
        resumed.extend(page.iter().map(|row| (row.record.position.sequence, row.sequence)));
    }
    assert_eq!(resumed, actual.into_iter().filter(|row| row.0 > after).collect::<Vec<_>>());
    fixture.finish().await;
}
