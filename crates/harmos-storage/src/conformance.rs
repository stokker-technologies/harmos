/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Backend Contract Proofs Shared With Product Backends.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use arrow_array::{StructArray, UInt64Array};

// Local Imports
use crate::query::{FileSelection, RecordSelection, StreamSelection};
use crate::writes::{DecodedBatch, convert};
use crate::{
    BackendSnapshot, CompoundOutcome, Error, FileMetadata, FileReference, Identifier, MemorySize, MutationId, MutationItem, ObjectPoint,
    ObjectValue, ObjectVersion, ObjectVersionToken, RecordInput, Result, ScanPage, StorageCursor, StorageManager, StoredRecord, StoredRow,
    StreamInput,
};
use harmos_query::arrow::ArrowBatch;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Fresh keys in the supplied manager's catalog; replacement keeps the first key.
pub struct CompoundFixture {
    pub first: ObjectValue,
    pub replacement: ObjectValue,
    pub second: ObjectValue,
    pub record: RecordInput,
}

/// Fresh Scoped Data for the Backend's Logical Deletion Proof.
#[derive(Clone)]
pub struct ScopeFixture {
    pub scope: Identifier,
    pub object: ObjectValue,
    pub record: RecordInput,
    pub sample: StreamInput,
    pub file: FileMetadata,
    pub bytes: Vec<u8>,
}

/// Persist This Proof Between Closing and Reopening the Product Backend.
pub struct RetainedScope {
    pub snapshot: BackendSnapshot,
    pub fixture: ScopeFixture,
    pub file: FileReference,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Exercise ordered commits, late conflicts, retained cuts and receipt replay.
/// Start the manager with the product backend before running this proof.
pub async fn compound_mutations(storage: &StorageManager, fixture: CompoundFixture) -> Result<()> {
    let selection = RecordSelection {
        schema: fixture.record.schema.clone(),
        scope: fixture.record.scope.clone(),
        item: Some(fixture.record.item.clone()),
        source_start: None,
        source_end: None,
    };
    let window = storage.mutation_window()?.id;
    let opening = storage.get_saved_position();
    let mut follower = storage.follow::<StoredRecord>(Some(&opening), selection.clone()).await?;
    let mutation = MutationId {
        window: window.clone(),
        key: Identifier::new("conformance.compound.create").expect("valid conformance identity"),
    };
    let point = ObjectPoint {
        schema: fixture.first.schema.id.clone(),
        item: fixture.first.item.clone(),
        version: ObjectVersion::Latest,
    };
    let second = ObjectPoint {
        schema: fixture.second.schema.id.clone(),
        item: fixture.second.item.clone(),
        version: ObjectVersion::Latest,
    };
    let items = vec![MutationItem::Create(fixture.first.clone()), MutationItem::Append(fixture.record.clone())];
    let created = storage.mutate_once(&mutation, items.clone()).await?;
    let CompoundOutcome::Applied { positions } = &created.outcome else {
        return Err(invalid("use fresh fixture keys or repair backend atomic create"));
    };
    if positions.len() != 2 || positions.last() != Some(&created.position) {
        return Err(invalid("repair backend ordered compound positions"));
    }
    if storage.mutate_once(&mutation, items).await? != created {
        return Err(invalid("repair backend durable compound receipt replay"));
    }
    let (observed, frontier) = follower.next_durable().await?;
    if observed.entries().len() != 1 || frontier != created.position {
        return Err(invalid("include committed compound entries at their complete frontier"));
    }
    let before = storage.snapshot(&created.position).await?;
    let expected = ObjectVersionToken::new(positions[0].clone());

    // A Late Conflict Leaves Earlier Objects and Records Unchanged.
    let conflict = MutationId {
        window: window.clone(),
        key: Identifier::new("conformance.compound.conflict").expect("valid conformance identity"),
    };
    let items = vec![
        MutationItem::Create(fixture.second.clone()),
        MutationItem::Append(fixture.record.clone()),
        MutationItem::Set {
            expected: ObjectVersionToken::new(created.position.clone()),
            value: fixture.replacement.clone(),
        },
    ];
    let refused = storage.mutate_once(&conflict, items.clone()).await?;
    if refused.outcome
        != (CompoundOutcome::Conflict {
            index: 2,
            current: Some(expected.clone()),
        })
        || storage.mutate_once(&conflict, items).await? != refused
    {
        return Err(invalid("repair backend atomic late conflict and refusal replay"));
    }
    let cut = storage.snapshot(&refused.position).await?;
    let (observed, frontier) = follower.next_durable().await?;
    if !observed.entries().is_empty() || frontier != refused.position {
        return Err(invalid("advance conflicts without observing refused compound entries"));
    }
    let page = ScanPage {
        after: created.position.sequence,
        rows: storage.settings().response.batch_rows,
        bytes: storage.settings().response.batch_bytes,
    };
    let (records, _) = cut.scan::<StoredRecord>(&selection, page).await?;
    if cut.get_object(&second).await?.is_some() || !records.is_empty() {
        return Err(invalid("repair backend rollback of earlier compound items"));
    }
    drop(cut);

    // Replace and Delete Together Without Changing the Retained Image.
    let mutation = MutationId {
        window,
        key: Identifier::new("conformance.compound.replace").expect("valid conformance identity"),
    };
    let items = vec![
        MutationItem::Set {
            expected,
            value: fixture.replacement.clone(),
        },
        MutationItem::Create(fixture.second),
        MutationItem::Delete {
            schema: second.schema.clone(),
            item: second.item.clone(),
            expected: None,
        },
        MutationItem::Append(fixture.record),
    ];
    let changed = storage.mutate_once(&mutation, items.clone()).await?;
    if !matches!(changed.outcome, CompoundOutcome::Applied { .. }) || storage.mutate_once(&mutation, items).await? != changed {
        return Err(invalid("repair backend ordered replacement, deletion and replay"));
    }
    let after = storage.snapshot(&changed.position).await?;
    if !matches!(follower.next_durable().await, Err(Error::Gap { .. })) {
        return Err(invalid("terminate deletion observation with a resync gap"));
    }
    let (records, _) = after.scan::<StoredRecord>(&selection, ScanPage { after: refused.position.sequence, ..page }).await?;
    let original = before.get_object(&point).await?.map(|entry| entry.value);
    let replacement = after.get_object(&point).await?.map(|entry| entry.value);
    if original != Some(fixture.first)
        || replacement != Some(fixture.replacement)
        || after.get_object(&second).await?.is_some()
        || records.len() != 1
    {
        return Err(invalid("repair backend retained cuts and ordered compound deletion"));
    }
    Ok(())
}

/// Prove Exact Scope Selection, Retained Reads and Replay Without Backend Internals.
/// Products Separately Verify Physical Reclamation After Their Retained Cuts Release.
pub async fn scoped_deletion(storage: &StorageManager, fixture: ScopeFixture) -> Result<()> {
    if fixture.object.scope.as_ref() != Some(&fixture.scope)
        || fixture.record.scope.as_ref() != Some(&fixture.scope)
        || fixture.sample.record.scope.as_ref() != Some(&fixture.scope)
        || fixture.file.scope.as_ref() != Some(&fixture.scope)
        || fixture.bytes.len() > storage.settings().response.batch_bytes.get()
    {
        return Err(invalid("supply matching fixture scopes and file bytes within response.batch_bytes"));
    }
    let window = storage.mutation_window()?.id;
    let mutation = MutationId {
        window: window.clone(),
        key: Identifier::new("conformance.scope.seed").expect("valid conformance identity"),
    };
    let point = ObjectPoint {
        schema: fixture.object.schema.id.clone(),
        item: fixture.object.item.clone(),
        version: ObjectVersion::Latest,
    };
    storage
        .mutate_once(&mutation, vec![MutationItem::Create(fixture.object.clone()), MutationItem::Append(fixture.record.clone())])
        .await?;
    storage.append_samples(vec![fixture.sample.clone()]).await?;
    let mut file = storage.produce_file(fixture.file).await?;
    file.write_all(fixture.bytes.clone()).await?;
    let file = file.finish().await?;
    storage.flush().await?;
    let before = storage.snapshot(&storage.get_saved_position()).await?;
    let retained = before.retain(Some(&fixture.scope)).await?;
    let reader = storage.open_retained_snapshot(&retained).await?;
    let independent = reader.retain(None).await?;
    let mutation = MutationId {
        window,
        key: Identifier::new("conformance.scope.delete").expect("valid conformance identity"),
    };
    let items = vec![MutationItem::DeleteScope(fixture.scope.clone())];
    let deleted = storage.mutate_once(&mutation, items.clone()).await?;
    if storage.open_retained_snapshot(&retained).await.is_ok() {
        return Err(invalid("release scope-owned retention atomically with scope deletion"));
    }
    let independent_reader = storage.open_retained_snapshot(&independent).await?;
    if storage.mutate_once(&mutation, items).await? != deleted {
        return Err(invalid("repair backend scoped deletion receipt replay"));
    }
    let after = storage.snapshot(&deleted.position).await?;
    let page = ScanPage {
        after: 0,
        rows: storage.settings().response.batch_rows,
        bytes: storage.settings().response.batch_bytes,
    };
    let records = RecordSelection {
        schema: fixture.record.schema,
        scope: Some(fixture.scope.clone()),
        item: None,
        source_start: None,
        source_end: None,
    };
    let samples = StreamSelection {
        schema: fixture.sample.record.schema,
        scope: Some(fixture.scope.clone()),
        item: None,
        stream: None,
        source_start: None,
        source_end: None,
        after: 0,
    };
    let files = FileSelection { scope: Some(fixture.scope), item: None };
    let (records, _) = after.scan::<StoredRecord>(&records, page).await?;
    let (samples, _) = after.scan::<StoredRow>(&samples, page).await?;
    let (files, _) = after.scan::<FileReference>(&files, page).await?;
    if after.get_object(&point).await?.is_some() || !records.is_empty() || !samples.is_empty() || !files.is_empty() {
        return Err(invalid("repair backend scoped deletion visibility across every storage family"));
    }
    if before.get_object(&point).await?.map(|entry| entry.value) != Some(fixture.object)
        || before.read_file(&file, 0, storage.settings().response.batch_bytes).await? != fixture.bytes
        || reader.get_object(&point).await?.is_none()
        || reader.read_file(&file, 0, storage.settings().response.batch_bytes).await? != fixture.bytes
        || independent_reader.get_object(&point).await?.is_none()
        || independent_reader.read_file(&file, 0, storage.settings().response.batch_bytes).await? != fixture.bytes
    {
        return Err(invalid("retain deleted objects and file bytes until their pinned cuts release"));
    }
    storage.release_retained_snapshot(&independent).await?;
    Ok(())
}

/// Seed and Persist a Scope Cut. Close and Reopen the Backend Before Calling `check_retained_scope`.
pub async fn retain_scope(storage: &StorageManager, fixture: ScopeFixture) -> Result<RetainedScope> {
    let mutation = MutationId {
        window: storage.mutation_window()?.id,
        key: Identifier::new("conformance.retained.seed").expect("valid conformance identity"),
    };
    storage
        .mutate_once(&mutation, vec![MutationItem::Create(fixture.object.clone()), MutationItem::Append(fixture.record.clone())])
        .await?;
    storage.append_samples(vec![fixture.sample.clone()]).await?;
    let mut writer = storage.produce_file(fixture.file.clone()).await?;
    writer.write_all(fixture.bytes.clone()).await?;
    let file = writer.finish().await?;
    storage.flush().await?;
    let image = storage.snapshot(&storage.get_saved_position()).await?;
    let snapshot = image.retain(Some(&fixture.scope)).await?;
    Ok(RetainedScope { snapshot, fixture, file })
}

/// Prove Durable Ownership After Restart Without a Live Manager or Snapshot Registry.
pub async fn check_retained_scope(storage: &StorageManager, proof: &RetainedScope) -> Result<()> {
    let image = storage.open_retained_snapshot(&proof.snapshot).await?;
    let point = ObjectPoint {
        schema: proof.fixture.object.schema.id.clone(),
        item: proof.fixture.object.item.clone(),
        version: ObjectVersion::Latest,
    };
    let page = ScanPage {
        after: 0,
        rows: storage.settings().response.batch_rows,
        bytes: storage.settings().response.batch_bytes,
    };
    let records = RecordSelection {
        schema: proof.fixture.record.schema.clone(),
        scope: None,
        item: None,
        source_start: None,
        source_end: None,
    };
    let samples = StreamSelection {
        schema: proof.fixture.sample.record.schema.clone(),
        scope: None,
        item: None,
        stream: None,
        source_start: None,
        source_end: None,
        after: 0,
    };
    let (records, _) = image.scan::<StoredRecord>(&records, page).await?;
    let (samples, _) = image.scan::<StoredRow>(&samples, page).await?;
    let (files, _) = image.scan::<FileReference>(&FileSelection { scope: None, item: None }, page).await?;
    let scope = Some(&proof.fixture.scope);
    if image.get_object(&point).await?.map(|entry| entry.value) != Some(proof.fixture.object.clone())
        || image.read_file(&proof.file, 0, page.bytes).await? != proof.fixture.bytes
        || records.len() != 1
        || records[0].scope.as_ref() != scope
        || samples.len() != 1
        || samples[0].record.scope.as_ref() != scope
        || files.len() != 1
        || files[0].metadata.scope.as_ref() != scope
    {
        return Err(invalid("persist the complete scoped image and reopen it after restart"));
    }
    Ok(())
}

/// Prove Column Order, Authorized Scope, Immutable Replay and Durable Receipts.
/// Supply a Fresh Destination With at Least Two Rows and a Scope-Owned Item.
/// Return the Identity and Frontier for the Product's Restart Proof.
pub async fn columnar_appends(storage: &StorageManager, scope: &Identifier, batch: &DecodedBatch) -> Result<(MutationId, StorageCursor)> {
    if batch.data.as_record_batch().num_rows() < 2 {
        return Err(invalid("supply at least two sample rows for the ordering proof"));
    }
    let mutation = MutationId {
        window: storage.mutation_window()?.id,
        key: Identifier::new("conformance.sample.columns").expect("valid conformance identity"),
    };
    let before = storage.status().accepted;
    let accepted = storage.append_sample_batch(&mutation, Some(scope), batch).await?;
    if accepted.sequence != before.sequence + batch.data.as_record_batch().num_rows() as u64
        || storage.append_sample_batch(&mutation, Some(scope), batch).await? != accepted
    {
        return Err(invalid("assign contiguous sample positions once and preserve replay's accepted frontier"));
    }
    storage.flush().await?;
    storage.wait_mutation_durable(&mutation, &accepted).await?;
    if !storage.mutation_durable(&mutation, &accepted)? {
        return Err(invalid("acknowledge sample completion only after data and receipt are durable"));
    }
    let schema = storage.lookup_schema(&batch.schema).await?;
    let expected = convert::records(Some(scope), batch, &schema.schema)?;
    let metadata = batch.data.as_record_batch().column_by_name("_storage").unwrap().as_any().downcast_ref::<StructArray>().unwrap();
    let sequence = metadata.column_by_name("sequence").unwrap().as_any().downcast_ref::<UInt64Array>().unwrap();
    let cut = storage.snapshot(&accepted).await?;
    let selection = StreamSelection {
        schema: batch.schema.clone(),
        scope: Some(scope.clone()),
        item: None,
        stream: Some(batch.destination.clone()),
        source_start: None,
        source_end: None,
        after: 0,
    };
    let mut after = before.sequence;
    let mut index = 0;
    while index < expected.len() {
        let page = ScanPage {
            after,
            rows: storage.settings().response.batch_rows,
            bytes: storage.settings().response.batch_bytes,
        };
        let (rows, _) = cut.scan::<StoredRow>(&selection, page).await?;
        if rows.is_empty() {
            return Err(invalid("return every committed columnar sample within bounded response pages"));
        }
        for row in rows {
            let Some(expected) = expected.get(index) else {
                return Err(invalid("never duplicate columnar samples"));
            };
            if row.record.position.sequence != before.sequence + index as u64 + 1
                || row.record.payload != expected.payload
                || row.record.scope.as_ref() != Some(scope)
                || row.record.item != expected.item
                || row.record.source != expected.source
                || row.record.source_time != expected.source_time
                || row.stream != batch.destination
                || row.sequence != sequence.value(index)
            {
                return Err(invalid("preserve columnar sample order, payloads, metadata and exact authorized scope"));
            }
            let frontier = storage.watch_frontier(&before, &row.record.position).await?;
            let expected_frontier = if row.record.position == accepted { &accepted } else { &before };
            if &frontier != expected_frontier {
                return Err(invalid("publish columnar sample cursors only at the complete append frontier"));
            }
            after = row.record.position.sequence;
            index += 1;
        }
    }
    let foreign = Identifier::new(format!("{}.other", scope.as_str())).expect("valid conformance scope");
    let selection = StreamSelection { scope: Some(foreign), ..selection };
    let (rows, _) = cut
        .scan::<StoredRow>(
            &selection,
            ScanPage {
                after: before.sequence,
                rows: storage.settings().response.batch_rows,
                bytes: storage.settings().response.batch_bytes,
            },
        )
        .await?;
    if !rows.is_empty() {
        return Err(invalid("isolate columnar samples by exact scope"));
    }
    let mut changed = batch.clone();
    changed.data = ArrowBatch::try_new(batch.data.as_record_batch().slice(0, expected.len() - 1))?;
    if storage.append_sample_batch(&mutation, Some(scope), &changed).await.is_ok() {
        return Err(invalid("refuse changed sample batches under an existing mutation identity"));
    }
    Ok((mutation, accepted))
}

/// Gate Backend Completion Before Calling; Acceptance Must Remain Replayable Without Durability.
pub async fn columnar_pending(storage: &StorageManager, scope: &Identifier, batch: &DecodedBatch) -> Result<(MutationId, StorageCursor)> {
    let mutation = MutationId {
        window: storage.mutation_window()?.id,
        key: Identifier::new("conformance.sample.pending").expect("valid conformance identity"),
    };
    let accepted = storage.append_sample_batch(&mutation, Some(scope), batch).await?;
    if storage.mutation_durable(&mutation, &accepted)? || storage.append_sample_batch(&mutation, Some(scope), batch).await? != accepted {
        return Err(invalid("report acceptance without durable completion while the backend commit is gated"));
    }
    let status = storage.status();
    if status.pending_batches > storage.settings().request.max_pending_write_batches.get()
        || status.pending_bytes > storage.settings().request.max_pending_write_bytes.get()
    {
        return Err(invalid("keep outstanding columnar batches and retained bytes within consumer admission bounds"));
    }
    Ok((mutation, accepted))
}

/// Run After Reopening the Product Backend to Prove Receipt Recovery.
pub async fn columnar_reopened(
    storage: &StorageManager, scope: &Identifier, batch: &DecodedBatch, mutation: &MutationId, accepted: &StorageCursor,
) -> Result<()> {
    let before = storage.status().accepted;
    let replay = storage.append_sample_batch(mutation, Some(scope), batch).await?;
    if &replay != accepted || storage.status().accepted != before || !storage.mutation_durable(mutation, accepted)? {
        return Err(invalid("restore the original durable sample receipt without admitting duplicate rows"));
    }
    Ok(())
}

/// Oversized Retained Arrow Allocations Must Refuse Before Taking Queue Capacity.
pub async fn columnar_admission_bounds(storage: &StorageManager, scope: &Identifier, oversized: &DecodedBatch) -> Result<()> {
    let before = storage.status();
    let mutation = MutationId {
        window: storage.mutation_window()?.id,
        key: Identifier::new("conformance.sample.capacity").expect("valid conformance identity"),
    };
    if oversized.data.memory_size() <= storage.settings().request.max_pending_write_bytes.get() {
        return Err(invalid("supply a sample batch retaining more than max_pending_write_bytes"));
    }
    if !matches!(storage.append_sample_batch(&mutation, Some(scope), oversized).await, Err(Error::ResourceExhausted { .. })) {
        return Err(invalid("refuse sample allocations exceeding the consumer's pending byte budget"));
    }
    let after = storage.status();
    if after.accepted != before.accepted || after.pending_bytes != before.pending_bytes || after.pending_batches != before.pending_batches {
        return Err(invalid("release columnar admission capacity on refusal without advancing acceptance"));
    }
    Ok(())
}

/* === Private Functions === */

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "backend conformance", reason }
}

/* === Tests === */
