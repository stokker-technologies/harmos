/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === --- === Definition Module === --- === */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{Duration, OperationPolicy, RetryPolicy, Value};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[cfg_attr(not(target_arch = "wasm32"), harmos_domain::operation(harmos_path = ::harmos_domain,
    resources = [crate::Owner], id = "fixture.finish", response = Value, policy = OperationPolicy {
        timeout: Duration::from_secs(5), attempt_timeout: Duration::from_secs(5),
        cancellation_timeout: Duration::from_millis(100), retries: RetryPolicy::Never,
    }))]
#[cfg_attr(target_arch = "wasm32", harmos::guest::operation(harmos_path = ::harmos::guest,
    resources = [crate::Owner], id = "fixture.finish", response = Value, policy = OperationPolicy {
        timeout: Duration::from_secs(5), attempt_timeout: Duration::from_secs(5),
        cancellation_timeout: Duration::from_millis(100), retries: RetryPolicy::Never,
    }))]
#[derive(Clone, PartialEq)]

pub struct FinishOperation {
    pub mode: u32,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

/* === --- === Runtime Module === --- === */

#[cfg(target_arch = "wasm32")]
mod runtime {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports

    // Local Imports
    use super::{FinishOperation, Value};

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl harmos::guest::Operation for FinishOperation {
        type Error = std::io::Error;

        async fn run(self, sdk: harmos::guest::Context<'_, Self>) -> Result<harmos::guest::operations::Response<Value>, std::io::Error> {
            let owner = sdk.resource::<crate::Owner>();
            owner.ran.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.mode == 2 {
                return std::future::pending().await;
            }
            sdk.cancelled().await;
            if self.mode == 1 {
                return std::future::pending().await;
            }
            Ok(Value { value: "42".into() }.into())
        }
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}
