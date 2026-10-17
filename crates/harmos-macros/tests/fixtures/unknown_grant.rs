/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


/* === Modules === */
#[path = "observer_controls.rs"]
mod controls;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos::operations::OperationPolicy;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Program;

/* === Enums === */

/* === Traits === */
trait ProductPlugin {}

/* === Implementations === */
impl harmos::operations::lifecycle::Policy for Program {
    const INITIALIZATION_POLICY: OperationPolicy = controls::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = controls::POLICY;
}

#[harmos::plugin(id = "invalid-requirement", provides = (), requires(harmos::operations::lifecycle::Status: invoke("ledger.deposit")),
    context = harmos::sidecar::Supervisor, config = ())]
impl ProductPlugin for Program {}

/* === Public Functions === */

/* === Private Functions === */
fn main() {}

/* === Tests === */
