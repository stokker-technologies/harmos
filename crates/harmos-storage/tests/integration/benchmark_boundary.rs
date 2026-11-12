/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Keep The Public Workload's Smallest Physical Boundary Crossing In Ordinary Checks.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports
use arrow_array::Float64Array;

// Local Imports
use crate::manager::backend;
use harmos_storage::bench_support::workload::{CHUNK_ROWS, Workload, sample};
use harmos_storage::query::{
    columnar::{Cancellation, list_column_rows},
    projection::{ProjectionRead, advance_projection},
};
use harmos_storage::sqlite::{SqliteBackend, SqliteSettings};
use harmos_storage::*;

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
async fn shared_workload_keeps_boundary_pages_and_replayed_projection_identical() {
    // Populate Native And Fallback Backends With One Boundary-Crossing Source.
    let workload = Workload::new(CHUNK_ROWS + 1);
    let native = SqliteBackend::in_memory(
        workload.history.clone(),
        SqliteSettings {
            chunk_rows: NonZeroUsize::new(CHUNK_ROWS).unwrap(),
            chunk_bytes: workload.page().bytes,
        },
    )
    .unwrap();
    let fallback = backend::Data::new(workload.history.clone());
    for backend in [&native as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        backend.create_session().await.unwrap();
        backend.append_schemas(StorageBatch::new(vec![workload.schema(1)])).await.unwrap();
        backend.append_records(workload.records(0, CHUNK_ROWS)).await.unwrap();
        backend.append_records(workload.records(CHUNK_ROWS, workload.rows)).await.unwrap();
        let snapshot = backend.create_snapshot(&workload.cut()).await.unwrap();

        // Read Across The Chunk Boundary Without Repeating Or Losing A Row.
        let mut read = workload.scan(CHUNK_ROWS - 1);
        read.limits.rows = NonZeroUsize::new(2).unwrap();
        let mut values = Vec::new();
        loop {
            let page = list_column_rows(backend, &snapshot, &read, &Cancellation::default()).await.unwrap();
            values.extend_from_slice(page.batch.column(0).as_any().downcast_ref::<Float64Array>().unwrap().values());
            if page.complete {
                break;
            }
            read.after = page.next;
        }
        assert_eq!(values, vec![sample(CHUNK_ROWS - 1).value, sample(CHUNK_ROWS).value]);

        // Publish, Replay And Read The Same Boundary From A Durable Projection.
        let definition = workload.projection();
        let revision = backend.set_projection_definition(&definition, false).await.unwrap();
        advance_projection(backend, &snapshot, &definition.name, None, workload.work(), &Cancellation::default()).await.unwrap();
        advance_projection(backend, &snapshot, &definition.name, None, workload.work(), &Cancellation::default()).await.unwrap();
        let mut read = ProjectionRead {
            name: definition.name,
            revision,
            frontier: workload.cut(),
            after: CHUNK_ROWS - 1,
            limits: workload.page(),
            work: workload.work(),
        };
        let mut projected = Vec::new();
        loop {
            let page = backend.list_projection_rows(&read).await.unwrap();
            projected.extend_from_slice(page.batch.column(0).as_any().downcast_ref::<Float64Array>().unwrap().values());
            let Some(next) = page.next else { break };
            read.after = next;
        }
        assert_eq!(projected, values);
        assert_eq!(backend.get_saved_position().await.unwrap(), workload.cut());
        backend.delete_snapshot(&snapshot).await.unwrap();
        backend.delete_session().await.unwrap();
    }
}
