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

pub use cancel::{Cancel, CancelRequest};
pub use harmos_domain::operations::lifecycle::*;
pub use status::{InstanceState, Status, StatusRequest};
pub use watch::{Watch, WatchRequest};

/* === Constants === */

pub const POLICY: harmos_domain::OperationPolicy = harmos_domain::OperationPolicy {
    timeout: std::time::Duration::from_secs(30),
    attempt_timeout: std::time::Duration::from_secs(30),
    retries: harmos_domain::RetryPolicy::Never,
};
const WATCH_POLICY: harmos_domain::OperationPolicy = harmos_domain::OperationPolicy {
    timeout: harmos_domain::operations::UNBOUNDED,
    attempt_timeout: harmos_domain::operations::UNBOUNDED,
    ..POLICY
};

/* === Enums === */

#[harmos::catalog(harmos_path = crate)]
pub enum Lifecycle {
    Status(Status),
    Cancel(Cancel),
    Watch(Watch),
}
