/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Prove Bounded Storage Work

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports

// Local Imports
use harmos_storage::query::{
    columnar::Cancellation,
    index::{IndexSeek, list_index_entries},
    projection::{ProjectionRead, advance_projection},
};
use harmos_storage::sqlite::{SqliteBackend, SqliteSettings};
use harmos_storage::{
    bench_support::workload::{Workload, sample},
    *,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

async fn projection_all(backend: &dyn StorageBackend, mut read: ProjectionRead) -> arrow_array::RecordBatch {
    let mut batches = Vec::new();
    loop {
        let page = backend.list_projection_rows(&read).await.unwrap();
        batches.push(page.batch);
        let Some(next) = page.next else {
            break;
        };
        read.after = next;
    }
    arrow_select::concat::concat_batches(&batches[0].schema(), &batches).unwrap()
}

/* === Tests === */

#[tokio::test]
async fn corrections_and_tied_seeks_do_not_revisit_unaffected_history() {
    // Commit A History With Tied Index Keys And Many Independent Buckets.
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("bounded.sqlite");
    let workload = Workload::new(512);
    let settings = SqliteSettings {
        chunk_rows: NonZeroUsize::new(32).unwrap(),
        chunk_bytes: workload.page().bytes,
    };
    let backend = SqliteBackend::new(&path, workload.history.clone(), settings).unwrap();
    backend.create_session().await.unwrap();
    backend.append_schemas(StorageBatch::new(vec![workload.schema(1)])).await.unwrap();
    let records = (0..workload.rows)
        .map(|index| {
            let mut value = sample(index);
            value.value = 1.0;
            workload.record(index, index as u64 + 2, value)
        })
        .collect();
    backend.append_records(StorageBatch::new(records)).await.unwrap();
    let snapshot = backend.create_snapshot(&workload.cut()).await.unwrap();
    let cancel = Cancellation::default();

    // Build Once Per Physical Chunk With One Prepared Insertion.
    let before = backend.get_work().await.unwrap();
    backend
        .set_index_definition(&workload.index(), None, false, workload.work(), &cancel, None)
        .await
        .and_then(|outcome| outcome.into_result(harmos_storage::DefinitionKind::Index))
        .unwrap();
    let after = backend.get_work().await.unwrap();
    assert_eq!(after.source_rows - before.source_rows, workload.rows as u64);
    assert_eq!(after.decoded_chunks - before.decoded_chunks, (workload.rows / settings.chunk_rows.get()) as u64);
    assert_eq!(after.index_prepares - before.index_prepares, 1);

    // Seek Deep Inside One Tie Without Scanning The Equal-Key Prefix.
    let mut read = workload.index_read();
    read.limits.rows = NonZeroUsize::new(8).unwrap();
    let first = list_index_entries(&backend, &snapshot, &read, &cancel).await.unwrap();
    let before = backend.get_work().await.unwrap();
    read.after = Some(IndexSeek {
        snapshot: snapshot.token.clone(),
        plan: read.identity(&workload.index()).unwrap(),
        revision: 1,
        key: first.entries[0].key.clone(),
        position: 480,
    });
    let deep = list_index_entries(&backend, &snapshot, &read, &cancel).await.unwrap();
    let after = backend.get_work().await.unwrap();
    assert_eq!(deep.entries.iter().map(|entry| entry.position).collect::<Vec<_>>(), (481..489).collect::<Vec<_>>());
    assert_eq!(after.index_entries - before.index_entries, read.limits.rows.get() as u64 + 1);
    assert!(after.index_steps - before.index_steps < (read.limits.rows.get() as u64 + 1) * 80);
    assert_eq!(after.source_rows, before.source_rows);

    // Correct One Identity Across A Boundary And Rebuild Exactly Two Buckets.
    let definition = workload.buckets();
    backend
        .set_projection_definition(&definition, None, false, None)
        .await
        .and_then(|outcome| outcome.into_result(harmos_storage::DefinitionKind::Projection))
        .unwrap();
    advance_projection(&backend, &snapshot, &definition.name, None, workload.work(), &cancel).await.unwrap();
    let original = backend
        .list_projection_rows(&ProjectionRead {
            name: definition.name.clone(),
            revision: 1,
            frontier: workload.cut(),
            after: 0,
            limits: workload.page(),
            work: workload.work(),
        })
        .await
        .unwrap();
    let mut value = sample(0);
    value.time_ns -= 60 * 1_000_000_000;
    value.value = 2.0;
    let record = workload.record(0, workload.cut().sequence + 1, value);
    backend.append_records(StorageBatch::new(vec![record])).await.unwrap();
    let next = backend.get_saved_position().await.unwrap();
    let late = backend.create_snapshot(&next).await.unwrap();
    let before = backend.get_work().await.unwrap();
    advance_projection(&backend, &late, &definition.name, None, workload.work(), &cancel).await.unwrap();
    let after = backend.get_work().await.unwrap();
    assert_eq!(after.source_rows - before.source_rows, 1);
    assert_eq!(after.buckets - before.buckets, 2);
    assert!(after.bucket_steps - before.bucket_steps < (after.bucket_rows - before.bucket_rows) * 40 + 60);
    assert!(after.bucket_rows - before.bucket_rows < workload.rows as u64 / 8);
    let corrected = backend
        .list_projection_rows(&ProjectionRead {
            name: definition.name.clone(),
            revision: 1,
            frontier: next.clone(),
            after: 0,
            limits: workload.page(),
            work: workload.work(),
        })
        .await
        .unwrap();
    assert_eq!(corrected.changed_keys.len(), 2);
    let saved = backend
        .list_projection_rows(&ProjectionRead {
            name: definition.name.clone(),
            revision: 1,
            frontier: workload.cut(),
            after: 0,
            limits: workload.page(),
            work: workload.work(),
        })
        .await
        .unwrap();
    assert_eq!(saved.batch, original.batch);
    backend.delete_snapshot(&late).await.unwrap();
    backend.delete_snapshot(&snapshot).await.unwrap();
    backend.delete_session().await.unwrap();
    drop(backend);

    // Reopen The Checkpoint And Replay Without Reexamining Any Source Rows.
    let backend = SqliteBackend::new(&path, workload.history.clone(), settings).unwrap();
    backend.create_session().await.unwrap();
    let snapshot = backend.create_snapshot(&next).await.unwrap();
    let before = backend.get_work().await.unwrap();
    advance_projection(&backend, &snapshot, &definition.name, None, workload.work(), &cancel).await.unwrap();
    assert_eq!(backend.get_work().await.unwrap(), before);
    let reopened = backend
        .list_projection_rows(&ProjectionRead {
            name: definition.name,
            revision: 1,
            frontier: next,
            after: 0,
            limits: workload.page(),
            work: workload.work(),
        })
        .await
        .unwrap();
    assert_eq!(reopened.batch, corrected.batch);
    let full = projection_all(
        &backend,
        ProjectionRead {
            name: workload.buckets().name,
            revision: 1,
            frontier: reopened.frontier.clone(),
            after: 0,
            limits: workload.page(),
            work: workload.work(),
        },
    )
    .await;

    backend.delete_snapshot(&snapshot).await.unwrap();
    backend.delete_session().await.unwrap();
    drop(backend);

    // Reopen A Version-One Store Whose Published Outputs Use Legacy Arrow Pages.
    let connection = rusqlite::Connection::open(&path).unwrap();
    for (cut, batch) in [(workload.cut(), &original.batch), (reopened.frontier.clone(), &full)] {
        let mut data = Vec::new();
        let mut writer = arrow_ipc::writer::StreamWriter::try_new(&mut data, &batch.schema()).unwrap();
        writer.write(batch).unwrap();
        writer.finish().unwrap();
        drop(writer);
        connection
            .execute(
                "INSERT INTO projection_pages VALUES(?1,1,?2,0,?3,?4)",
                rusqlite::params![workload.buckets().name.as_str(), cut.sequence as i64, batch.num_rows() as i64, data],
            )
            .unwrap();
    }
    connection.execute_batch("DELETE FROM projection_bucket_members; DELETE FROM projection_bucket_outputs; DELETE FROM projection_bucket_state; DELETE FROM projection_bucket_cuts;").unwrap();
    drop(connection);
    let backend = SqliteBackend::new(&path, workload.history.clone(), settings).unwrap();
    backend.create_session().await.unwrap();
    let mut value = sample(0);
    value.time_ns -= 60 * 1_000_000_000;
    value.value = 3.0;
    let cut = backend.append_records(StorageBatch::new(vec![workload.record(0, reopened.frontier.sequence + 1, value)])).await.unwrap();
    let snapshot = backend.create_snapshot(&cut).await.unwrap();
    advance_projection(&backend, &snapshot, &workload.buckets().name, None, workload.work(), &cancel).await.unwrap();
    let migrated = backend
        .list_projection_rows(&ProjectionRead {
            name: workload.buckets().name,
            revision: 1,
            frontier: cut,
            after: 0,
            limits: workload.page(),
            work: workload.work(),
        })
        .await
        .unwrap();
    assert_eq!(migrated.changed_keys.len(), 1);
    let saved = backend
        .list_projection_rows(&ProjectionRead {
            name: workload.buckets().name,
            revision: 1,
            frontier: workload.cut(),
            after: 0,
            limits: workload.page(),
            work: workload.work(),
        })
        .await
        .unwrap();
    assert_eq!(saved.batch, original.batch);
    backend.delete_snapshot(&snapshot).await.unwrap();

    // Retire Every Published Cut And Continue From The Private Checkpoint.
    let status = backend.get_projection_status(&workload.buckets().name, 1, Some(&migrated.frontier)).await.unwrap().unwrap();
    backend.delete_projection_history(&workload.buckets().name, 1, &migrated.frontier).await.unwrap();
    let mut value = sample(1);
    value.value = 2.0;
    let cut = backend.append_records(StorageBatch::new(vec![workload.record(1, migrated.frontier.sequence + 1, value)])).await.unwrap();
    let snapshot = backend.create_snapshot(&cut).await.unwrap();
    advance_projection(&backend, &snapshot, &workload.buckets().name, None, workload.work(), &cancel).await.unwrap();
    let continued = projection_all(
        &backend,
        ProjectionRead {
            name: workload.buckets().name,
            revision: 1,
            frontier: cut,
            after: 0,
            limits: workload.page(),
            work: workload.work(),
        },
    )
    .await;
    assert_eq!(continued.num_rows(), status.rows);
    let sums = continued.column_by_name("sum").unwrap().as_any().downcast_ref::<arrow_array::Float64Array>().unwrap();
    assert_eq!(sums.values().iter().sum::<f64>(), workload.rows as f64 + 3.0);
    backend.delete_snapshot(&snapshot).await.unwrap();
    backend.delete_session().await.unwrap();
}
