/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Cancelled exchange retains.

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
async fn cancelled_exchange_retains() {
    let f = Fixture::new(config()).await;
    f.runtime.operations.register_catalog(harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default()).await.unwrap();
    seed(&f).await;

    // Cancelling Delivery Does Not Retract Storage-Owned Samples.
    let mut stream = f
        .runtime
        .operations
        .exchange_protobuf::<WriteStreams>(harmos_protocol::storage::WriteStreams { scope: None }, settings(), settings())
        .unwrap();
    let first = batch(1, "voltage");
    stream.send(first.into()).await.unwrap();
    let accepted = loop {
        let receipt = stream.next().await.unwrap().unwrap();
        if let Some(WriteReceipt::Accepted(receipt)) = receipt.receipt {
            break harmos_runtime::storage::StorageCursor::try_from(receipt.cursor.unwrap()).unwrap();
        }
    };
    stream.cancel_and_settle().await.unwrap();
    f.storage.flush_until(&accepted).await.unwrap();
    assert_eq!(f.data.rows.lock().unwrap().len(), 2);
    assert_eq!(f.storage.cursor_saved(), accepted);
    f.finish().await;
}
