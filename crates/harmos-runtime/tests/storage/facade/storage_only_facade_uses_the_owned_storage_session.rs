/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Facade Integration

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::support::Model;
use harmos_runtime::Runtime;
use harmos_runtime::catalog;
use harmos_runtime::facade::storage::InfoRequest;
use harmos_runtime::operations::{CallContext, Lane, Registration};

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

#[tokio::test(start_paused = true)]
async fn storage_only_facade_uses_the_owned_storage_session() {
    // Read Storage Before Full Startup
    let mut builder = Runtime::<Model>::builder(());
    builder.with_registration(Registration::catalog(catalog::storage_catalog(), Lane::default()).unwrap());
    let storage = builder.storage_only().await.unwrap();
    let call = storage.for_call(&CallContext::default());
    let info = call.storage().info(InfoRequest {}).await.unwrap();
    assert!(info.saved.is_some());

    // Closing Fences the Retained Facade
    storage.close().await.unwrap();
    assert!(call.storage().info(InfoRequest {}).await.is_err());
    drop(storage);

    // Full Startup Reopens the Same Backend History
    let runtime = builder.start().await.unwrap();
    let reopened = runtime.for_call(&CallContext::default()).storage().info(InfoRequest {}).await.unwrap();
    assert_eq!(info.saved, reopened.saved);
    runtime.shutdown().await.unwrap();
}
