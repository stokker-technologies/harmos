/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Host Clock Declarations and Narrow Clock Access.

/* === Modules === */

mod now;
mod sleep;

/* === Re-Exports === */

pub use now::Now;
pub use sleep::Sleep;

/* === Imports === */

use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

const POLICY: crate::OperationPolicy = crate::OperationPolicy {
    timeout: harmos_operations::operations::UNBOUNDED,
    attempt_timeout: harmos_operations::operations::UNBOUNDED,
    cancellation_timeout: Duration::ZERO,
    retries: crate::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct ClockAccess(pub Arc<dyn Clock>);

/* === Enums === */

/* === Traits === */

pub trait Clock: Send + Sync {
    fn now(&self) -> prost_types::Timestamp;
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
