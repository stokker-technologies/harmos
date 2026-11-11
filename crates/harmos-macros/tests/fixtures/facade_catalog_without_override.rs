/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[facade::message(harmos_path = ::harmos)]
#[derive(facade::Request, Clone)]
#[harmos_path(::harmos)]
struct Export {}
static DESCRIPTION: facade::__harmos::operations::WireDeclaration = facade::__harmos::operations::WireDeclaration::request::<Export>("test.export", facade::__harmos::model::operations::lifecycle::POLICY);
const SPEC: facade::__harmos::operations::OperationSpec<Export> = facade::__harmos::operations::OperationSpec::new(&DESCRIPTION);
#[cfg(not(feature = "mismatched_payload"))]
async fn export(_: Export, _: facade::__harmos::operations::FunctionContext<Export>) -> Result<facade::__harmos::Response<()>, std::convert::Infallible> { Ok(().into()) }
#[cfg(feature = "mismatched_payload")]
async fn export(_: Export, _: facade::__harmos::operations::FunctionContext<Export>) -> Result<facade::__harmos::Response<u64>, std::convert::Infallible> { Ok(0.into()) }
fn main() {
 let entry = facade::__harmos::operations::Operation::function(SPEC, &[], export, |_| facade::__harmos::operations::FailureKind::Permanent).unwrap();
 let nested = facade::__harmos::__private::Catalog::new().entry(entry);
 assert_eq!(nested.operations.len(), 1);
 assert_eq!(nested.operations[0].definition.id.as_str(), "test.export");
}
