/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Wait Operation Contract And Execution.

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

#[cfg_attr(not(target_arch = "wasm32"), harmos_domain::operation(harmos_path = ::harmos_domain,
    operations = [super::ProviderEchoOperation], id = "probe.wait", response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_millis(5),
        attempt_timeout: Duration::from_millis(5),
        retries: RetryPolicy::Never,
    }))]
#[cfg_attr(target_arch = "wasm32", harmos::guest::operation(harmos_path = ::harmos::guest,
    operations = [super::ProviderEchoOperation], id = "probe.wait", response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_millis(5),
        attempt_timeout: Duration::from_millis(5),
        retries: RetryPolicy::Never,
    }))]
#[derive(Clone, PartialEq)]

pub struct WaitOperation {}

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
    use super::super::ProviderEchoOperation;
    use super::{Echoed, WaitOperation};
    use harmos::guest::OperationRefusal;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl harmos::guest::Operation for WaitOperation {
        type Error = OperationRefusal;

        // The Provider Answers Slowly; Nothing Here Burns Fuel While It Waits
        async fn run(self, sdk: harmos::guest::Context<'_, Self>) -> Result<harmos::guest::operations::Response<Echoed>, OperationRefusal> {
            let result: Result<Echoed, OperationRefusal> = async move {
                let _ = sdk.operation(ProviderEchoOperation { say: "slow".into() }).await;
                // Resuming Reaches The Epoch Check A Suspended Guest Could Not
                for step in 0..4096u32 {
                    std::hint::black_box(step);
                }
                Ok(Echoed { said: "waited".to_owned() })
            }
            .await;
            result.map(Into::into)
        }
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}
