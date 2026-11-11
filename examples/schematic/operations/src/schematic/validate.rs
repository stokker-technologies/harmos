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

const POLICY: harmos_operations::operations::OperationPolicy = harmos_operations::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos_operations::operations::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_operations::operation(
    id = "schematic.validate",
    response = Validation,
    policy = POLICY,
    visibility = Public,
    replay = harmos_operations::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "Validate"),
)]
#[derive(Clone, Debug, PartialEq)]
pub struct ValidateSchematic {
    pub schematic: schematic_domain::model::Snapshot,
}

#[harmos_operations::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Validation {
    pub diagnostics: Vec<String>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos_operations::Operation for ValidateSchematic {
    type Error = crate::Error;

    async fn run(self, _: harmos_operations::Context<'_, Self>) -> Result<harmos_operations::Response<Validation>, Self::Error> {
        let diagnostics = self.schematic.diagnostics();
        Ok(Validation { diagnostics }.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
