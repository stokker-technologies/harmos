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

#[derive(::harmos_guest::Message, Debug, Default)]
#[harmos_path(::harmos_guest)]
#[derive(::harmos_guest::Request)]
#[request(response = Echoed)]
#[derive(Clone, PartialEq)]

pub struct WaitOperation {}

pub static WAIT_OPERATION_DESCRIPTION: ::harmos_guest::operations::WireDeclaration =
    ::harmos_guest::operations::WireDeclaration::request::<WaitOperation>(
        "probe.wait",
        OperationPolicy {
            timeout: Duration::from_millis(5),
            attempt_timeout: Duration::from_millis(5),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    );

static WAIT_OPERATION_CALLEES: [::harmos_guest::operations::WireDeclaration; 1] =
    [super::provider::PROVIDER_ECHO_OPERATION_DESCRIPTION.copied()];
pub const WAIT_OPERATION_OPERATION: ::harmos_guest::operations::OperationSpec<WaitOperation> =
    ::harmos_guest::operations::OperationSpec::<WaitOperation>::new(&WAIT_OPERATION_DESCRIPTION).calls(&WAIT_OPERATION_CALLEES);

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
    use crate::contract::{PROVIDER_ECHO_OPERATION_OPERATION, WAIT_OPERATION_OPERATION};
    use harmos_guest::OperationRefusal;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    pub async fn wait_operation_handler(
        _request: WaitOperation, sdk: harmos_guest::operations::FunctionContext<WaitOperation>,
    ) -> Result<harmos_guest::operations::Response<Echoed>, OperationRefusal> {
        let result: Result<Echoed, OperationRefusal> = async move {
            let _ = sdk.operation(PROVIDER_ECHO_OPERATION_OPERATION, ProviderEchoOperation { say: "slow".into() }).await;
            // Resuming Reaches the Epoch Check a Suspended Guest Could Not
            for step in 0..4096u32 {
                std::hint::black_box(step);
            }
            Ok(Echoed { said: "waited".to_owned() })
        }
        .await;
        result.map(Into::into)
    }

    pub fn wait_operation_operation()
    -> ::core::result::Result<harmos_guest::operations::OperationFactory, harmos_guest::operations::ServingError> {
        harmos_guest::operations::OperationFactory::function(WAIT_OPERATION_OPERATION, const { &[] }, wait_operation_handler, |_| {
            harmos_guest::operations::FailureKind::Permanent
        })
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

#[cfg(target_arch = "wasm32")]
pub use runtime::wait_operation_operation;
