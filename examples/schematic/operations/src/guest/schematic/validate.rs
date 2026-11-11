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

#[derive(harmos_operations::Message)]
#[harmos_path(harmos_operations)]
#[derive(harmos_operations::Request)]
#[request(response = Validation)]
#[derive(Clone, Debug, PartialEq)]
pub struct ValidateSchematic {
    pub schematic: schematic_domain::model::Snapshot,
}

pub static VALIDATE_SCHEMATIC_DESCRIPTION: harmos_operations::operations::WireDeclaration =
    harmos_operations::operations::WireDeclaration::request::<ValidateSchematic>("schematic.validate", POLICY)
        .visibility(harmos_operations::operations::Visibility::Public)
        .replay(harmos_operations::operations::ReplaySafety::Idempotent)
        .grpc(harmos_operations::operations::Grpc::new("schematic.v1", "Schematic", "Validate"));

pub const VALIDATE_SCHEMATIC_OPERATION: harmos_operations::operations::OperationSpec<ValidateSchematic> =
    harmos_operations::operations::OperationSpec::<ValidateSchematic>::new(&VALIDATE_SCHEMATIC_DESCRIPTION);

#[derive(harmos_operations::Message, Default, Clone, Debug, PartialEq)]
pub struct Validation {
    pub diagnostics: Vec<String>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
mod execution {
    use super::*;
    pub async fn validate_schematic_handler(
        request: ValidateSchematic, _: harmos_operations::execution::operations::FunctionContext<ValidateSchematic>,
    ) -> Result<harmos_operations::Response<Validation>, crate::Error> {
        let diagnostics = request.schematic.diagnostics();
        Ok(Validation { diagnostics }.into())
    }

    pub fn validate_schematic_operation() -> ::core::result::Result<
        harmos_operations::execution::operations::OperationFactory,
        harmos_operations::execution::operations::ServingError,
    > {
        harmos_operations::execution::operations::OperationFactory::function(
            VALIDATE_SCHEMATIC_OPERATION,
            const { &[] },
            validate_schematic_handler,
            |_| harmos_operations::operations::FailureKind::Permanent,
        )
    }
}

#[cfg(feature = "runtime")]
pub use execution::validate_schematic_operation;

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
