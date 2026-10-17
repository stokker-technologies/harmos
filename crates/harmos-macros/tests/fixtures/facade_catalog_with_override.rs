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
#[facade::message(harmos_path = ::facade::__harmos)]
#[facade::operation(
    harmos_path = ::facade::__harmos, id = "facade.export",
    response = (),
    policy = facade::__harmos::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(30),
    attempt_timeout: std::time::Duration::from_secs(1),
    retries: facade::__harmos::operations::RetryPolicy::Never,
    restart: facade::__harmos::operations::RestartPolicy::Never,
},
    version = 1,
)]
struct Export { count: u64 }

/* === Enums === */
#[facade::catalog(harmos_path = ::facade::__harmos)]
enum Operations {
    Export(Export),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn main() {
    let entries = facade::__harmos::operations::Declarations::of::<Operations>().unwrap();
    assert_eq!(entries[0].id.as_str(), "facade.export");
}

/* === Tests === */
