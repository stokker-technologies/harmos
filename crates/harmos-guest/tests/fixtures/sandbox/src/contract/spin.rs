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
use super::{Duration, Echoed, OperationPolicy, RetryPolicy};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[cfg_attr(not(target_arch = "wasm32"), harmos_domain::operation(harmos_path = ::harmos_domain, id = "probe.spin", response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_secs(600),
        attempt_timeout: Duration::from_secs(600),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: RetryPolicy::Never,
    }))]
#[cfg_attr(target_arch = "wasm32", harmos::guest::operation(harmos_path = ::harmos::guest, id = "probe.spin", response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_secs(600),
        attempt_timeout: Duration::from_secs(600),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: RetryPolicy::Never,
    }))]
#[derive(Clone, PartialEq)]

pub struct SpinOperation {}

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
    use super::{Echoed, SpinOperation};
    use harmos::guest::OperationRefusal;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl harmos::guest::Operation for SpinOperation {
        type Error = OperationRefusal;

        async fn run(
            self, _sdk: harmos::guest::Context<'_, Self>,
        ) -> Result<harmos::guest::operations::Response<Echoed>, OperationRefusal> {
            let result: Result<Echoed, OperationRefusal> = async move {
                let mut counted = 0u64;
                loop {
                    counted = counted.wrapping_add(1);
                    std::hint::black_box(counted);
                }
            }
            .await;
            result.map(Into::into)
        }
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}
