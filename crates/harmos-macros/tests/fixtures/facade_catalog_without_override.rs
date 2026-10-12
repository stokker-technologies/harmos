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
struct Export;

/* === Enums === */
#[facade::catalog]
enum Operations {
    Export(Export),
}

/* === Traits === */

/* === Implementations === */
impl facade::__harmos::operations::OperationDeclaration for Export {
    type Request = usize;
    type Response = usize;
    const ID: &'static str = "facade.export";
    const POLICY: facade::__harmos::operations::OperationPolicy = facade::__harmos::operations::OperationPolicy {
        timeout: std::time::Duration::from_secs(30),
        attempt_timeout: std::time::Duration::from_secs(1),
        retries: facade::__harmos::operations::RetryPolicy::Never,
    };
}

/* === Public Functions === */

/* === Private Functions === */
fn main() {
    let entries = facade::__harmos::operations::Declarations::of::<Operations>().unwrap();
    assert_eq!(entries[0].id.as_str(), "facade.export");
}

/* === Tests === */
