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

#[harmos_sidecar::operation]
impl Validating {
    async fn initialize(_: Arc<crate::Resources>, _: &harmos_sidecar::Supervisor) -> Self {
        Self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn run(&mut self, _context: &harmos_sidecar::Supervisor, request: &mut ValidateSchematic) -> Result<Validation, Error> {
        let diagnostics = request.schematic.diagnostics();
        Ok(Validation { diagnostics })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
