/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[harmos::operation(
    id = "schematic.validate",
    response = Validation,
    policy = crate::POLICY,
    visibility = Public,
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

#[cfg(feature = "runtime")]
impl harmos::Operation for ValidateSchematic {
    type Error = crate::Error;

    async fn run(self, _: harmos::Context<'_, Self>) -> Result<harmos::Response<Validation>, Self::Error> {
        let diagnostics = self.schematic.diagnostics();
        Ok(Validation { diagnostics }.into())
    }
}
