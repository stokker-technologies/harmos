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

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(
    id = "schematic_validate",
    response = Validation,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "Validate"),
)]
#[derive(Clone, Debug, PartialEq)]
pub struct ValidateSchematic {
    pub schematic: schematic_domain::model::Snapshot,
}

#[harmos::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Validation {
    pub diagnostics: Vec<String>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos::Operation for ValidateSchematic {
    type Error = crate::Error;

    async fn run(
        mut self, _: harmos::Context<'_, Self>,
    ) -> Result<harmos::Response<<Self as harmos::OperationDeclaration>::Response>, Self::Error> {
        let request = &mut self;

        let diagnostics = request.schematic.diagnostics();
        Ok(Validation { diagnostics }.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
