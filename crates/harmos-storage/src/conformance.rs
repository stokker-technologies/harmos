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
use crate::query::RecordSelection;
use crate::{
    CompoundOutcome, Error, Identifier, MutationId, MutationItem, ObjectPoint, ObjectValue, ObjectVersion, ObjectVersionToken, RecordInput,
    Result, ScanPage, StorageManager, StoredRecord,
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

/* === Private Functions === */

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "backend conformance", reason }
}

/* === Tests === */
