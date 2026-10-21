/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
// Third Party Imports
// Local Imports
use super::Permit;
use crate::state::recovery::Group;
use crate::storage::StorageManager;
use crate::storage::manager::encoded_size;
use crate::{Result, state::Kind};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C> StorageManager<C> {
    pub(crate) async fn reserve_group(
        &self, group: &Group<&serde_json::Value>, projection: Option<&crate::storage::capture::Projection>,
    ) -> Result<Permit> {
        let projection = projection.filter(|_| group.entries.iter().any(|e| e.kind == Kind::Record));
        let bytes = encoded_size(group)?;
        let mut permit = self.reserve_transition(bytes, projection.map(|p| p.limits)).await?;
        if let Some(projection) = projection {
            let mut records = crate::storage::ProjectedRecords::new(&mut permit, projection.limits);
            for entry in group.entries.iter().filter(|e| e.kind == Kind::Record) {
                projection.handler.project(entry, &mut records).await?;
            }
            records.check(self.settings().max_batch_bytes())?;
        }
        permit.encode(group)?;
        Ok(permit)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
