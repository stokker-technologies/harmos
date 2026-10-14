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
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::Error;
use schematic_contract::operations::validate::{Request, Response};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct ValidateSchematic;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos_guest::operation(declaration = schematic_contract::operations::ValidateSchematic)]
impl ValidateSchematic {
    async fn initialize(_: Arc<()>, _: &harmos_guest::Context) -> Self {
        Self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn run(
        &mut self,
        _context: &harmos_guest::Context,
        request: &mut Request,
    ) -> Result<Response, Error> {
        let symbols: std::collections::BTreeSet<_> =
            request.schematic.symbols.iter().copied().collect();
        let mut diagnostics = Vec::new();
        if symbols.len() != request.schematic.symbols.len() {
            diagnostics.push("Duplicate symbol IDs".to_owned());
        }
        for connection in &request.schematic.connections {
            if !symbols.contains(&connection.from) || !symbols.contains(&connection.to) {
                diagnostics.push(format!(
                    "Connection {} to {} references a missing symbol",
                    connection.from, connection.to
                ));
            }
        }
        Ok(Response { diagnostics })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
