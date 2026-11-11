/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod cancel;
mod status;
mod watch;

/* === Re-Exports === */

pub use cancel::Cancel;
pub use harmos_domain::model::operations::lifecycle::*;

pub use status::{InstanceState, Status};
pub use watch::Watch;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

pub const POLICY: harmos_domain::OperationPolicy = harmos_domain::OperationPolicy {
    timeout: std::time::Duration::from_secs(30),
    attempt_timeout: std::time::Duration::from_secs(30),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos_domain::RetryPolicy::Never,
};
const WATCH_POLICY: harmos_domain::OperationPolicy = harmos_domain::OperationPolicy {
    timeout: harmos_domain::operations::UNBOUNDED,
    attempt_timeout: harmos_domain::operations::UNBOUNDED,
    cancellation_timeout: std::time::Duration::ZERO,
    ..POLICY
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[harmos::catalog(harmos_path = crate)]
pub enum Lifecycle {
    Status(Status),
    Cancel(Cancel),
    Watch(Watch),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
