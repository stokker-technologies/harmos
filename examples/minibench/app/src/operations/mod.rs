/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub(crate) mod report;

/* === Re-Exports === */
pub use minibench_contract::probe::{ReadSample, Sample};
pub use report::{Corrected, ReportError, ReportRequest};

/* === Imports === */
// Standard Library Imports
use std::time::Duration;

// Third Party Imports

// Local Imports
use harmos::operations::{OperationPolicy, ReplaySafety, RetryPolicy};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[harmos::catalog]
pub enum Operations {
    #[operation(
        request = ReportRequest,
        response = Corrected,
        id = "minibench.report",
        policy = OperationPolicy {
            timeout: Duration::from_secs(30),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::Never,
        },
        replay = ReplaySafety::Idempotent,
    )]
    Report,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
