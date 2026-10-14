/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub mod analyze;
pub mod propose;
pub mod validate;

/* === Re-Exports === */
pub use analyze::AnalyzeSchematic;
pub use propose::ProposeRoute;
pub use validate::ValidateSchematic;

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */
pub const POLICY: harmos_contract::operations::OperationPolicy =
    harmos_contract::operations::OperationPolicy {
        timeout: std::time::Duration::from_secs(10),
        attempt_timeout: std::time::Duration::from_secs(10),
        retries: harmos_contract::operations::RetryPolicy::Never,
        restart: harmos_contract::operations::RestartPolicy::Never,
    };

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[harmos_contract::catalog]
pub enum Operations {
    Validate(ValidateSchematic),
    Route(ProposeRoute),
    Analyze(AnalyzeSchematic),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
