/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use harmos_operations::execution as harmos;
const POLICY: harmos::OperationPolicy = harmos_operations::model::operations::lifecycle::POLICY;
#[derive(harmos::Message, Debug, Default)]
#[harmos_path(harmos)]
#[derive(harmos::Request, Clone)]

struct Read;
static DESCRIPTION: harmos::operations::WireDeclaration = harmos::operations::WireDeclaration::request::<Read>("fixture.always", POLICY);
const SPEC: harmos::operations::OperationSpec<Read> = harmos::operations::OperationSpec::new(&DESCRIPTION);
async fn read(_: Read, _: harmos::operations::FunctionContext<Read>) -> Result<harmos::Response<()>, std::convert::Infallible> { Ok(().into()) }
fn main() {
 let first = harmos::operations::Operation::function(SPEC, &[], read, |_| harmos::operations::FailureKind::Permanent).unwrap();
 let batch = harmos::__private::Catalog::new().entry(first);
 #[cfg(feature = "optional_operation")]
 let batch = batch.entry(harmos::operations::Operation::function(optional::SPEC, &[], read, |_| harmos::operations::FailureKind::Permanent).unwrap());
 let expected = if cfg!(feature = "optional_operation") { 2 } else { 1 };
 assert_eq!(batch.operations.len(), expected);
 assert_eq!(batch.operations[0].definition.id.as_str(), "fixture.always");
 if cfg!(feature = "optional_operation") { assert_eq!(batch.operations[1].definition.id.as_str(), "fixture.optional"); }
}
#[cfg(feature = "optional_operation")]
mod optional {
 use super::*;
 static DESCRIPTION: harmos::operations::WireDeclaration = harmos::operations::WireDeclaration::request::<Read>("fixture.optional", POLICY);
 pub const SPEC: harmos::operations::OperationSpec<Read> = harmos::operations::OperationSpec::new(&DESCRIPTION);
}
