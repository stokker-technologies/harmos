/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "native_operations.rs"]
mod native;

#[path = "operation_deletion.rs"]
mod deletion;
#[path = "operation_objects.rs"]
mod objects;
#[path = "operation_schemas.rs"]
mod schemas;
#[path = "operation_writes.rs"]
mod writes;

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports

// Local Imports
use super::{Fixture, config, write};
use harmos_operations::storage::storage_catalog;

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn private_operations_run_against_the_registered_backend() {
    let fixture = Fixture::new(config()).await;
    crate::support::app::register(&fixture.runtime, storage_catalog(), harmos_runtime::operations::Lane::default()).await.unwrap();
    let info = fixture
        .runtime
        .operations
        .call_native::<harmos_operations::storage::Info>(harmos_operations::storage::InfoRequest {})
        .await
        .unwrap();
    assert_eq!(info.accepted.unwrap(), fixture.storage.cursor_accepted());
    assert_eq!(info.saved.unwrap(), fixture.storage.get_saved_position());
    let private =
        fixture.runtime.operations.definitions().into_iter().filter(|entry| entry.id.as_str().starts_with("storage.")).collect::<Vec<_>>();
    assert!(!private.is_empty());
    assert!(private.iter().all(|entry| entry.visibility == harmos_runtime::operations::Visibility::Private));
    assert!(storage_catalog().operations.iter().all(|factory| factory.contract().is_err()));
    fixture.finish().await;
}
