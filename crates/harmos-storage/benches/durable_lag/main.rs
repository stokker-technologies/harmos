/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure Saved-Position Lag Through The In-Memory Adapter.

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
    for (repetition, load) in (1..=5)
        .flat_map(|repetition| [Load::SmallHighRate, Load::MultiMegabyte, Load::Mixed].into_iter().map(move |load| (repetition, load)))
    {
        // Open An Isolated WAL/FULL Backend For Each Load.
        let activity_before = activity();
        let history = StoreHistory {
            store: Identifier::new("durable-lag").unwrap(),
            epoch: Identifier::new("benchmark").unwrap(),
        };
        let primary = Arc::new(memory::Backend::new(history.clone()));
        let recovery = Arc::new(memory::RecoveryBackend::new(history));
        let settings = StorageSettings {
            transfer: Default::default(),
            policy: Default::default(),
            filesystem: StorageFilesystemSettings {
                poll_interval: std::time::Duration::from_millis(100),
                max_entries: nz(100_000),
                event_capacity: nz(1024),
            },
            request: StorageRequestSettings {
                max_file_productions: std::num::NonZeroUsize::new(256).unwrap(),
                max_mutation_receipts: std::num::NonZeroUsize::new(1024).unwrap(),
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
                "repetition": repetition, "activity_before": activity_before, "activity_after": activity(),
                "measurement": result, "backend": "memory",
                    "synchronous_commits": after.synchronous_commits - before.synchronous_commits,
                    "synchronous_commit_ms": (after.synchronous_commit_ns - before.synchronous_commit_ns) as f64 / 1_000_000.0,
                    "maximum_synchronous_commit_ms": after.maximum_synchronous_commit_ns as f64 / 1_000_000.0,
                    "writing_ms": (after.writing_ns - before.writing_ns) as f64 / 1_000_000.0,
                    "maximum_writing_ms": after.maximum_writing_ns as f64 / 1_000_000.0,
                })
            );
            storage.close().await.unwrap();
        });
    }
}

fn activity() -> serde_json::Value {
    #[cfg(windows)]
    let output = std::process::Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command",
        "@(Get-Process -Name cargo,rustc,cl,link,cmake,ninja,python -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,CPU) | ConvertTo-Json -Compress"])
        .output();
    #[cfg(not(windows))]
    let output = std::process::Command::new("sh")
        .args(["-c", "ps -eo pid,comm,time | awk '$2 ~ /^(cargo|rustc|cc|ld|cmake|ninja|python|python3)$/ {print}'"])
        .output();
    output
        .map(|output| {
            serde_json::json!({"processes": String::from_utf8_lossy(&output.stdout),
        "logical_cpus": std::thread::available_parallelism().map(|value| value.get()).unwrap_or(0)})
        })
        .unwrap_or_else(|error| serde_json::json!({"unavailable": error.to_string()}))
}

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

/* === Tests === */
