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

#[harmos::message]
#[derive(Clone, Debug, PartialEq)]
pub struct ValidateSchematicRequest {
    pub schematic: schematic_domain::model::Snapshot,
}

#[harmos::operation(
    id = "schematic_validate",
    request = ValidateSchematicRequest, infer_request, response = Validation,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "Validate"),
)]
pub struct ValidateSchematic {}

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
    type Request = ValidateSchematicRequest;
    type Response = Validation;
    type Error = crate::Error;
    type Endpoints = ();

    async fn initialize(_: harmos::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self {})
    }

    async fn run(
        &mut self, _: harmos::OperationContext, request: &mut Self::Request, _: &mut Self::Endpoints,
    ) -> Result<harmos::Response<Self::Response>, Self::Error> {
        let diagnostics = request.schematic.diagnostics();
        Ok(Validation { diagnostics }.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
