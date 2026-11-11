/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Record exchange receipts.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn record_exchange_receipts() {
    let f = Fixture::new(config()).await;
    f.runtime.operations.register_catalog(harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default()).await.unwrap();
    seed(&f).await;
    let mut records = f
        .runtime
        .operations
        .exchange_protobuf::<WriteRecords>(harmos_protocol::storage::WriteRecords { scope: None }, settings(), settings())
        .unwrap();
    let first = batch(1, "bench");
    records.send(first.into()).await.unwrap();
    records.finish_input().unwrap();
    let mut accepted = None;
    let mut durable = None;
    while let Some(receipt) = records.next().await.unwrap() {
        match receipt.receipt.as_ref().unwrap() {
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
