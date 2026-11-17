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

// Local Imports
use crate::query::{FileSelection, RecordSelection, StreamSelection};
use crate::{
    BackendSnapshot, CompoundOutcome, Error, FileMetadata, FileReference, Identifier, MutationId, MutationItem, ObjectPoint, ObjectValue,
    ObjectVersion, ObjectVersionToken, RecordInput, Result, ScanPage, StorageManager, StoredRecord, StoredRow, StreamInput,
};

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

/* === Private Functions === */

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "backend conformance", reason }
}

/* === Tests === */
