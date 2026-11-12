/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure Unary and Streaming File Transfers

/* === Modules === */

#[path = "../../../harmos-storage/tests/support/storage.rs"]
#[allow(dead_code, reason = "The deterministic backend also serves broader storage stories")]
mod backend;
mod carrier;
mod model;
#[path = "../../tests/storage/transfer_plugin.rs"]
#[allow(dead_code, reason = "The same plugin carrier also serves deterministic transfer stories")]
mod transfer_plugin;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_storage::bench_support::file_transfer;
use std::{num::NonZeroUsize, sync::Arc, time::Duration};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    let executor = tokio::runtime::Runtime::new().unwrap();
    for repetition in 1..=3 {
        for (plugin_carrier, streaming) in [(false, false), (false, true), (true, false)] {
            executor.block_on(async {
                // Compose an Isolated Host With the Same Reference Backend and Budgets.
                let data = Arc::new(backend::Data::new(backend::history()));
                let recovery = Arc::new(backend::Recovery::new(backend::history()));
                let settings = backend::storage_settings();
                let chunk_bytes = settings.response.batch_bytes;
                let storage = harmos_runtime::storage::StorageManager::new(harmos_storage::StorageBackends { primary: data, state: recovery }, settings).unwrap();
                let host = harmos_runtime::Runtime::<model::Model>::launch_with((), (), |setup| {
                    setup.settings().storage(&storage).operation_settings(harmos_runtime::operations::Settings {
                        preparation_bytes: Some(NonZeroUsize::new(16 * 1024 * 1024).unwrap()), ..Default::default()
                    });
                    setup.catalog(harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default());
                }).await.unwrap();
                host.operations.register_authorizer(Arc::new(model::AllowTransfers)).unwrap();

                // Attach Both Real Socket Carriers.
                let plugin = transfer_plugin::TransferPlugin::start().await;
                let placement = harmos_runtime::plugin::grpc::PlacementSettings { connect_timeout: Duration::from_secs(30), reconnect_delay: Duration::from_secs(1), stop_grace: Duration::from_secs(30), stream_window: NonZeroUsize::new(64).unwrap() };
                let attached = host.attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(plugin.address.clone()), placement).await.unwrap();
                let listener = host.grpc().attach(harmos_runtime::grpc::Settings::default()).await.unwrap();
                let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
                let carrier = carrier::Transport::new(host.operations.clone(), channel, storage.mutation_window().unwrap().id.to_string(), plugin_carrier);

                // Measure Upload and Download, Including Publication and Integrity.
                let measurements = file_transfer::run(&carrier, chunk_bytes, streaming).await.unwrap();
                for measurement in measurements {
                    println!("STORAGE_FILE_TRANSFER {}", serde_json::json!({ "case": format!("integration/file_transfer/{}/{}/{}", measurement.carrier, measurement.transfer, measurement.direction), "backend": "reference-memory", "repetition": repetition, "measurement": measurement }));
                }

                // Settle Owned Work and Release the Isolated Dataset.
                drop(carrier);
                listener.detach().await.unwrap();
                attached.stop("transfer benchmark complete").await.unwrap();
                plugin.stop().await;
                host.shutdown().await.unwrap();
            });
        }
    }
    println!(
        "STORAGE_FILE_TRANSFER_UNAVAILABLE {}",
        serde_json::json!({ "carrier": "plugin", "transfer": "streaming", "reason": "plugin child calls are unary; no retained streaming upload or download is callable through this carrier" })
    );
}

/* === Tests === */
