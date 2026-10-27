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
    log::{Log, add, reserve},
};
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
        &self, snapshot: &BackendSnapshot, selection: &crate::IdentitySelection, page: ScanPage,
    ) -> Result<Vec<crate::StoredIdentity>> {
        self.pin(snapshot)?;
        let mut ordered = Vec::new();
        for identity in self.identities.values() {
            let sequence = identity.first().sequence;
            if sequence <= page.after || sequence > snapshot.at.sequence || !selection.matches(identity) {
                continue;
            }
            let insertion = ordered.partition_point(|prior: &&crate::StoredIdentity| prior.first().sequence < sequence);
            if insertion >= page.rows.get() {
                continue;
            }
            if ordered.len() == page.rows.get() {
                ordered.pop();
            }
            reserve(&mut ordered, page.rows.get())?;
            ordered.insert(insertion, identity);
        }
        let mut output = Vec::new();
        let mut bytes = 0usize;
        for identity in ordered {
            let size = crate::manager::encoded_size(identity)?;
            if !output.is_empty() && size > page.bytes.get().saturating_sub(bytes) {
                break;
            }
            reserve(&mut output, page.rows.get())?;
            bytes = add(bytes, size)?;
            output.push(identity.clone());
            if output.len() == page.rows.get() || bytes >= page.bytes.get() {
                break;
            }
        }
        Ok(output)
    }

    pub fn objects(&self, snapshot: &BackendSnapshot, selection: &ObjectSelection, page: ScanPage) -> Result<Vec<ObjectEntry>> {
        let generation = self.pin(snapshot)?;
        let reclaimed = self.pruned.iter().any(|(position, pruned)| *pruned <= generation && *position > page.after);
        if selection.versions == ObjectVersions::Revisions && reclaimed {
            return Err(crate::Error::Gap {
                requested: page.after + 1,
                available: self.pruned_through + 1,
            });
        }
        self.scan_read(snapshot.at.sequence, generation, page, |row: &ObjectEntry| {
            if !crate::selection::Entry::matches(row, selection) {
                return Ok(false);
            }
            if selection.versions == ObjectVersions::Revisions {
                return Ok(true);
            }
            let latest = self.latest(snapshot.at.sequence, generation, &row.value.schema.id, &row.value.item)?;
            Ok(latest.as_ref().is_some_and(|latest| latest.position == row.position))
        })
    }

    pub fn object(&self, snapshot: &BackendSnapshot, point: &ObjectPoint) -> Result<Option<ObjectEntry>> {
        let generation = self.pin(snapshot)?;
        match &point.version {
            ObjectVersion::Latest => self.latest(snapshot.at.sequence, generation, &point.schema, &point.item),
            ObjectVersion::Exact(version) => {
                self.history.check(version.history())?;
                let sequence = version.revision().get();
                if sequence > snapshot.at.sequence || self.pruned.get(&sequence).is_some_and(|pruned| *pruned <= generation) {
                    return Ok(None);
                }
                Ok(self
                    .one::<ObjectEntry>(sequence)?
                    .filter(|row| row.value.schema.id == point.schema && row.value.item == point.item && row.version == *version))
            }
        }
    }

    fn latest(&self, at: u64, generation: u64, schema: &crate::Identifier, item: &ObjectKey) -> Result<Option<ObjectEntry>> {
        let mut latest = None;
        self.visit_sized::<ObjectEntry>(at, 0, |row, _| {
            if row.value.schema.id == *schema
                && row.value.item == *item
                && !self.pruned.get(&row.position.sequence).is_some_and(|pruned| *pruned <= generation)
            {
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

        let mut remove = Vec::with_capacity(maximum.get());
        let mut complete = true;
        self.visit_sized::<ObjectEntry>(through.sequence, 0, |row, _| {
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
            self.visit_sized::<ObjectEntry>(through.sequence, row.position.sequence, |other, _| {
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
        let pruning = Pruning {
            through: self.pruned_through.max(through.sequence),
            positions,
        };
        format::store(&self.root.join("pruned.json"), &pruning, self.maximum)?;
        self.pruned = pruning.positions;
        self.pruned_through = pruning.through;
        self.generation = generation;
        Ok(complete)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
