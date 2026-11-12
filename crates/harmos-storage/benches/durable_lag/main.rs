/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure Durable Lag With SQLite WAL/FULL And A Native Worker.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
};

// Third Party Imports
use tokio::{sync::Mutex, task::JoinHandle};

// Local Imports
use harmos_storage::{
    bench_support::durable_lag::{self, Load},
    sqlite::{SqliteBackend, SqliteSettings},
    *,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Task(Mutex<Option<JoinHandle<Result<()>>>>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl WorkerOwner for Task {
    fn stop(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            if let Some(task) = self.0.lock().await.take() {
                task.await.map_err(|source| Error::OperationFailed {
                    operation: "join lag benchmark worker".into(),
                    source: Arc::new(source),
                })??;
            }
            Ok(())
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for load in [Load::SmallHighRate, Load::MultiMegabyte, Load::Mixed] {
        // Open An Isolated WAL/FULL Backend For Each Load.
        let root = tempfile::tempdir().unwrap();
        let history = StoreHistory {
            store: Identifier::new("durable-lag").unwrap(),
            epoch: Identifier::new("benchmark").unwrap(),
        };
        let primary = Arc::new(
            SqliteBackend::new(
                root.path().join("data.sqlite"),
                history.clone(),
                SqliteSettings {
                    chunk_rows: nz(16384),
                    chunk_bytes: nz(8 * 1024 * 1024),
                },
            )
            .unwrap(),
        );
        let recovery = Arc::new(state::file::FileBackend::new(
            root.path().join("recovery"),
            history,
            state::StorageStateSettings {
                max_entry_bytes: nz(16 * 1024 * 1024),
                max_checkpoint_bytes: nz(16 * 1024 * 1024),
            },
        ));
        let settings = StorageSettings {
            filesystem: StorageFilesystemSettings {
                poll_interval: std::time::Duration::from_millis(100),
                max_entries: nz(100_000),
                event_capacity: nz(1024),
            },
            request: StorageRequestSettings {
                max_pending_queries: nz(64),
                max_active_queries: nz(4),
                max_pending_write_batches: nz(256),
                max_active_file_operations: nz(1),
                max_pending_write_bytes: nz(16 * 1024 * 1024),
            },
            response: StorageResponseSettings {
                batch_rows: nz(64),
                batch_bytes: nz(64 * 1024),
            },
        };
        let storage = StorageManager::new(
            StorageBackends {
                primary: primary.clone(),
                state: recovery,
            },
            settings,
        )
        .unwrap();
        runtime.block_on(async {
            storage
                .start_with(Arc::new(()), |mut worker| {
                    let task = tokio::spawn(async move {
                        worker.run().await?;
                        worker.cleanup().await
                    });
                    Ok(Box::new(Task(Mutex::new(Some(task)))))
                })
                .await
                .unwrap();
            let schema = SchemaKey {
                id: Identifier::new("payload").unwrap(),
                version: NonZeroU32::MIN,
                payload_encoding: PayloadEncoding::PostcardV1,
            };
            // Persist The Same Payload Schema Used By Every Load.
            let graph = schema::graph::Schema::from_declaration(
                <durable_lag::Payload as schema::Described>::DEFINITION,
                &harmos_domain::config::SchemaSettings::default(),
            )
            .unwrap();
            storage.write_schema(schema.clone(), graph).await.unwrap();
            storage.flush().await.unwrap();
            let before = primary.get_work().await.unwrap();
            let result = durable_lag::run(&storage, &schema, load).await.unwrap();
            let after = primary.get_work().await.unwrap();
            println!(
                "STORAGE_DURABLE_LAG {}",
                serde_json::json!({
                    "case": format!("integration/durable_lag/{}", load.name()),
                    "p95_bound_ms": durable_lag::P95_BOUND_MS, "p95_within_bound": result.p95_ms <= durable_lag::P95_BOUND_MS,
                "measurement": result, "durability": "sqlite WAL synchronous=FULL",
                    "synchronous_commits": after.synchronous_commits - before.synchronous_commits,
                    "synchronous_commit_ms": (after.synchronous_commit_ns - before.synchronous_commit_ns) as f64 / 1_000_000.0,
                })
            );
            storage.close().await.unwrap();
        });
    }
}

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

/* === Tests === */
