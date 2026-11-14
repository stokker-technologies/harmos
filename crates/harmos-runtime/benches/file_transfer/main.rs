/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure Unary File Transfers

/* === Modules === */

#[path = "../../../harmos-storage/tests/support/storage.rs"]
#[allow(dead_code, reason = "The deterministic backend also serves broader storage stories")]
mod backend;
mod carrier;
mod model;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_protocol::transfer::default_message_bytes;
use harmos_runtime::{
    Runtime,
    operations::{Lane, Settings},
    storage::StorageManager,
};
use harmos_storage::StorageBackends;
use harmos_storage::bench_support::file_transfer;
use std::{num::NonZeroUsize, sync::Arc};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[tokio::main]
async fn main() {
    for repetition in 1..=3 {
        // Compose an Isolated Host With the Reference Backend and Consumer Budgets.
        let data = Arc::new(backend::Backend::new(backend::history()));
        let recovery = Arc::new(backend::RecoveryBackend::new(backend::history()));
        let settings = backend::storage_settings();
        let chunk_bytes = settings.transfer_chunk_bytes();
        let storage = StorageManager::new(StorageBackends { primary: data, state: recovery }, settings).unwrap();
        let host = Runtime::<model::Model>::launch_with((), (), |setup| {
            setup.settings().storage(&storage).operation_settings(Settings {
                preparation_bytes: Some(default_message_bytes().checked_mul(NonZeroUsize::new(6).unwrap()).unwrap()),
                ..Default::default()
            });
            setup.catalog(harmos_operations::storage_catalog(), Lane::default());
        })
        .await
        .unwrap();
        let carrier = carrier::Native::new(host.operations.clone(), storage.mutation_window().unwrap().id);

        // Measure Upload and Download, Including Publication and Integrity.
        let measurements = file_transfer::run(&carrier, chunk_bytes).await.unwrap();
        for measurement in measurements {
            println!(
                "STORAGE_FILE_TRANSFER {}",
                serde_json::json!({ "case": format!("integration/file_transfer/{}/{}/{}", measurement.carrier, measurement.transfer, measurement.direction), "backend": "reference-memory", "repetition": repetition, "measurement": measurement })
            );
        }

        // Settle Owned Work and Release the Isolated Dataset.
        drop(carrier);
        host.shutdown().await.unwrap();
    }
}

/* === Tests === */
