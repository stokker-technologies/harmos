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
use schematic_domain::operations::ValidateSchematic;
use schematic_domain::operations::schematic::validate::Validation;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct Validating;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos_guest::operation]
impl Validating {
    async fn initialize(_: Arc<()>, _: &harmos_guest::Context) -> Self {
        Self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn run(&mut self, _context: &harmos_guest::Context, request: &mut ValidateSchematic) -> Result<Validation, Error> {
        let diagnostics = request.schematic.diagnostics();
        Ok(Validation { diagnostics })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
