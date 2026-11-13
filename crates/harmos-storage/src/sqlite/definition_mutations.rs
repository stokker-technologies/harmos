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

use super::*;
use rusqlite::{Connection, OptionalExtension, params};

/* === Public Functions === */

pub(super) fn replay(
    connection: &Connection, mutation: Option<&crate::MutationHeader>, kind: crate::DefinitionKind, name: &crate::Identifier,
) -> Result<Option<crate::DefinitionOutcome>> {
    let Some(mutation) = mutation else { return Ok(None) };
    let prior: Option<Vec<u8>> = connection
        .query_row("SELECT data FROM storage_mutations WHERE id=?1", [text(&mutation.mutation)?], |row| row.get(0))
        .optional()
        .map_err(db)?;
    prior.map(|data| mutation.replay_definition(&decode(&data)?, kind, name)).transpose()
}
pub(super) fn write(
    connection: &Connection, mutation: Option<&crate::MutationHeader>, kind: crate::DefinitionKind, name: &crate::Identifier,
    outcome: &crate::DefinitionOutcome,
) -> Result<()> {
    if let Some(mutation) = mutation {
        let receipt = mutation.definition(kind, name.clone(), outcome.clone());
        connection
            .execute("INSERT INTO storage_mutations VALUES(?1,?2)", params![text(&mutation.mutation)?, encode(&receipt)?])
            .map_err(db)?;
    }
    Ok(())
}
