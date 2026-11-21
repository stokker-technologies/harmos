/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Configured Transfer Peer Admission

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::support::{self, Remote};
use crate::storage::fixture::{Backend, RecoveryBackend, history, storage_settings};
use harmos_runtime::{
    plugin::grpc::Placement,
    storage::{StorageBackends, StorageManager},
};
use std::sync::Arc;

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
async fn peer_admission_uses_configured_chunk_plus_framing() {
    // Configure a Small Transfer Without Shrinking the Transport Ceiling.
    let mut settings = storage_settings();
    settings.transfer.chunk_bytes = Some(1024);
    let required = harmos_protocol::transfer::required_message_bytes(&settings.transfer).unwrap().get() as u64;
    let storage = StorageManager::with_catalog(
        harmos_runtime::plugin::manager::catalog(),
        StorageBackends {
            primary: Arc::new(Backend::with_catalog(harmos_runtime::plugin::manager::catalog(), history())),
            state: Arc::new(RecoveryBackend::new(history())),
        },
        settings,
    )
    .unwrap();
    let runtime = crate::support::booted(Default::default(), |setup| {
        setup.settings().storage(&storage);
    })
    .await;
    let remote = Remote::start().await;

    // Refuse Exactly One Byte Below the Required Bound and Name the Action.
    remote.message_bytes(required - 1);
    let result = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await;
    let error = result.err().expect("undersized peer must refuse").to_string();
    assert!(error.contains("Instance.Get"));

    // Admit the Exact Required Bound on the Same Endpoint.
    remote.message_bytes(required);
    let attached = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    assert!(attached.activation().unwrap().is_connected());
    attached.stop("Story Complete").await.unwrap();
    runtime.stop().await.unwrap();
}
