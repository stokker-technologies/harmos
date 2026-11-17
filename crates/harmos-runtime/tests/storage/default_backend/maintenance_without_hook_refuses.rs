/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Unsupported Backend Maintenance

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use crate::storage::{Backend, Fixture, config, history};
use harmos_operations::execution::operations::ServingError;
use harmos_operations::storage::{self, CopyBackend, CopyBackendRequest};
use harmos_runtime::Error;

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
async fn maintenance_without_hook_refuses() {
    let fixture = Fixture::new(config()).await;
    fixture.runtime.operations.register_catalog(storage::storage_catalog(), Default::default()).await.unwrap();
    fixture.data.control.unsupported_maintenance.store(true, Ordering::SeqCst);
    let request = CopyBackendRequest {
        at: fixture.storage.get_saved_position(),
        target: Arc::new(Backend::new(history())),
    };
    let error = fixture.runtime.operations.call_native::<CopyBackend>(request).await.unwrap_err();
    let Error::OperationFailed { source, .. } = error else { panic!("unexpected failure: {error:?}") };
    let Some(ServingError::Handler(failure)) = source.downcast_ref() else {
        panic!("missing native failure");
    };
    assert_eq!(failure.detail("storage.kind"), Some("STORAGE_FAILURE_UNSUPPORTED"));
    fixture.finish().await;
}
