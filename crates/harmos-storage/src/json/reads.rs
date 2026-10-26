/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Historical Object Selection, Identity Paging And Lifecycle Retention.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// Local Imports
use super::{
    format,
    log::{Log, Read, add, exhausted, reserve},
};
use crate::MemorySize;
use crate::arrow::RetainedMemory;
use crate::query::{ObjectSelection, ObjectVersions};
use crate::{
    BackendSnapshot, ObjectEntry, ObjectKey, ObjectPoint, ObjectRetention, ObjectVersion, Result, ScanPage, SchemaKey, StorageCursor,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Serialize, Deserialize)]
struct Release {
    schema: SchemaKey,
    item: ObjectKey,
    through: StorageCursor,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Pruning {
    pub through: u64,
    pub positions: std::collections::BTreeMap<u64, u64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Log {
    pub fn catalog(
        &self, snapshot: &BackendSnapshot, selection: &crate::IdentitySelection, page: ScanPage, memory: &RetainedMemory, base: usize,
    ) -> Result<Vec<crate::StoredIdentity>> {
        self.pin(snapshot)?;
        let mut output = Vec::new();
        let mut bytes = 0usize;
        let mut ordered: Vec<&crate::StoredIdentity> = Vec::new();
        let read = Read::new(memory, base);
        read.retained.set(std::mem::size_of::<Vec<crate::StoredIdentity>>());
        let mut heap = 0usize;
        for identity in self.identities.values() {
            let sequence = identity.first().sequence;
            if sequence <= page.after || sequence > snapshot.at.sequence || !selection.matches(identity) {
                continue;
            }
            let insertion = ordered.partition_point(|prior| prior.first().sequence < sequence);
            if insertion >= page.rows.get() {
                continue;
            }
            if ordered.len() == page.rows.get() {
                ordered.pop();
            }
            let slots = ordered.capacity().checked_mul(std::mem::size_of::<&crate::StoredIdentity>()).ok_or_else(exhausted)?;
            let live = read.admit(slots)?;
            reserve(&mut ordered, page.rows.get(), &read, live)?;
            ordered.insert(insertion, identity);
        }
        let ordered_bytes = ordered.capacity().checked_mul(std::mem::size_of::<&crate::StoredIdentity>()).ok_or_else(exhausted)?;
        for identity in &ordered {
            let size = identity_size(identity);
            if !output.is_empty() && size > page.bytes.get().saturating_sub(bytes) {
                break;
            }
            let resident = identity.memory_size();
            let live = read.admit(add(ordered_bytes, resident)?)?;
            reserve(&mut output, page.rows.get(), &read, live)?;
            bytes = add(bytes, size)?;
            let row = (*identity).clone();
            heap = add(heap, row.memory_size().saturating_sub(std::mem::size_of::<crate::StoredIdentity>()))?;
            output.push(row);
            let slots = output.capacity().checked_mul(std::mem::size_of::<crate::StoredIdentity>()).ok_or_else(exhausted)?;
            read.retained.set(add(add(std::mem::size_of::<Vec<crate::StoredIdentity>>(), slots)?, heap)?);
            if output.len() == page.rows.get() || bytes >= page.bytes.get() {
                break;
            }
        }
        Ok(output)
    }

    pub fn objects(
        &self, snapshot: &BackendSnapshot, selection: &ObjectSelection, page: ScanPage, memory: &RetainedMemory, base: usize,
    ) -> Result<Vec<ObjectEntry>> {
        let generation = self.pin(snapshot)?;
        let reclaimed = self.pruned.iter().any(|(position, pruned)| *pruned <= generation && *position > page.after);
        if selection.versions == ObjectVersions::Revisions && reclaimed {
            return Err(crate::Error::Gap { requested: page.after + 1, available: self.pruned_through + 1 });
        }
        let read = Read::new(memory, base);
        self.scan_read(snapshot.at.sequence, generation, page, &read, |row: &ObjectEntry, live| {
            if !crate::selection::Entry::matches(row, selection) {
                return Ok(false);
            }
            if selection.versions == ObjectVersions::Revisions {
                return Ok(true);
            }
            let nested = Read::new(memory, live);
            let latest = self.latest(snapshot.at.sequence, generation, &row.value.schema.id, &row.value.item, &nested)?;
            Ok(latest.as_ref().is_some_and(|latest| latest.position == row.position))
        })
    }

    pub fn object(
        &self, snapshot: &BackendSnapshot, point: &ObjectPoint, memory: &RetainedMemory, base: usize,
    ) -> Result<Option<ObjectEntry>> {
        let generation = self.pin(snapshot)?;
        match &point.version {
            ObjectVersion::Latest => self.latest(snapshot.at.sequence, generation, &point.schema, &point.item, &Read::new(memory, base)),
            ObjectVersion::Exact(version) => {
                self.history.check(version.history())?;
                let sequence = version.revision().get();
                if sequence > snapshot.at.sequence || self.pruned.get(&sequence).is_some_and(|pruned| *pruned <= generation) {
                    return Ok(None);
                }
                Ok(self
                    .one::<ObjectEntry>(sequence, memory, base)?
                    .filter(|row| row.value.schema.id == point.schema && row.value.item == point.item && row.version == *version))
            }
        }
    }

    fn latest(
        &self, at: u64, generation: u64, schema: &crate::Identifier, item: &ObjectKey, read: &Read<'_>,
    ) -> Result<Option<ObjectEntry>> {
        let mut latest = None;
        self.visit_sized::<ObjectEntry>(at, 0, read, |row, _, _| {
            if row.value.schema.id == *schema
                && row.value.item == *item
                && !self.pruned.get(&row.position.sequence).is_some_and(|pruned| *pruned <= generation)
            {
                read.retained.set(row.memory_size());
                latest = Some(row);
            }
            Ok(true)
        })?;
        Ok(latest)
    }

    pub fn release(&self, schema: SchemaKey, item: ObjectKey, through: StorageCursor) -> Result<()> {
        self.history.check(&through.history)?;
        if through.sequence > self.saved {
            return Err(format::invalid("object release exceeds saved frontier"));
        }
        let bytes = serde_json::to_vec(&(&schema, &item)).map_err(format::json)?;
        let key = format::hex(&Sha256::digest(bytes));
        let path = self.root.join("releases").join(format!("{key}.json"));
        if let Some(prior) = format::load::<Release>(&path, self.maximum)?
            && prior.through.sequence >= through.sequence
        {
            return Ok(());
        }
        format::store(&path, &Release { schema, item, through }, self.maximum)
    }

    pub fn prune(
        &mut self, through: &StorageCursor, maximum: NonZeroUsize, retention: fn(&SchemaKey) -> harmos_domain::Result<ObjectRetention>,
    ) -> Result<bool> {
        self.history.check(&through.history)?;
        if through.sequence > self.saved {
            return Err(format::invalid("object pruning exceeds saved frontier"));
        }
        let mut remove = Vec::new();
        let mut complete = true;
        self.bootstrap_visit::<ObjectEntry>(through.sequence, 0, |row| {
            if self.pruned.contains_key(&row.position.sequence) {
                return Ok(true);
            }
            let retention = retention(&row.value.schema)?;
            if retention == ObjectRetention::UntilReleased {
                let bytes = serde_json::to_vec(&(&row.value.schema, &row.value.item)).map_err(format::json)?;
                let key = format::hex(&Sha256::digest(bytes));
                let path = self.root.join("releases").join(format!("{key}.json"));
                let release = format::load::<Release>(&path, self.maximum)?;
                if release.is_none_or(|release| release.through.sequence < row.position.sequence) {
                    return Ok(true);
                }
            }
            let keep = retention.versions().get();
            let mut newer = 0;
            self.bootstrap_visit::<ObjectEntry>(through.sequence, row.position.sequence, |other| {
                if other.value.schema.id == row.value.schema.id
                    && other.value.item == row.value.item
                    && !self.pruned.contains_key(&other.position.sequence)
                {
                    newer += 1;
                }
                Ok(newer < keep)
            })?;
            if newer == keep {
                if remove.len() == maximum.get() {
                    complete = false;
                    return Ok(false);
                }
                remove.push(row.position.sequence);
            }
            Ok(true)
        })?;
        if remove.is_empty() {
            return Ok(complete);
        }
        let generation = self.generation.checked_add(1).ok_or_else(|| format::invalid("object pruning generation overflow"))?;
        let mut positions = self.pruned.clone();
        for sequence in remove {
            positions.insert(sequence, generation);
        }
        let pruning = Pruning { through: self.pruned_through.max(through.sequence), positions };
        format::store(&self.root.join("pruned.json"), &pruning, self.maximum)?;
        self.pruned = pruning.positions;
        self.pruned_through = pruning.through;
        self.generation = generation;
        Ok(complete)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn identity_size(identity: &crate::StoredIdentity) -> usize {
    use crate::{MemorySize, StoredIdentity};
    let base = std::mem::size_of::<StoredIdentity>() + identity.first().memory_size() + identity.schema().memory_size();
    match identity {
        StoredIdentity::Stream { scope, item, stream, .. } => base + scope.memory_size() + item.memory_size() + stream.memory_size(),
        StoredIdentity::Object { key, .. } => base + key.memory_size(),
    }
}

/* === Tests === */
