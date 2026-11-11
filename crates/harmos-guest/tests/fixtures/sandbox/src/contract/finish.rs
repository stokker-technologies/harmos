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

#[::harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(::harmos_guest::Request)]
#[harmos_path(::harmos_guest)]
#[request(response = Value)]
#[derive(Clone, PartialEq)]

pub struct FinishOperation {
    pub mode: u32,
}

pub static FINISH_OPERATION_DESCRIPTION: ::harmos_guest::operations::WireDeclaration =
    ::harmos_guest::operations::WireDeclaration::request::<FinishOperation>(
        "fixture.finish",
        OperationPolicy {
            timeout: Duration::from_secs(5),
            attempt_timeout: Duration::from_secs(5),
            cancellation_timeout: Duration::from_millis(100),
            retries: RetryPolicy::Never,
        },
    );

pub const FINISH_OPERATION_OPERATION: ::harmos_guest::operations::OperationSpec<FinishOperation> =
    ::harmos_guest::operations::OperationSpec::<FinishOperation>::new(&FINISH_OPERATION_DESCRIPTION);

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
    use crate::contract::FINISH_OPERATION_OPERATION;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    pub async fn finish_operation_handler(
        request: FinishOperation, sdk: harmos_guest::operations::FunctionContext<FinishOperation>,
    ) -> Result<harmos_guest::operations::Response<Value>, std::io::Error> {
        let owner = sdk.resource::<crate::Owner>().expect("declared resource is registered");
        owner.ran.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if request.mode == 2 {
            return std::future::pending().await;
        }
        sdk.cancelled().await;
        if request.mode == 1 {
            return std::future::pending().await;
        }
        Ok(Value { value: "42".into() }.into())
    }

    pub fn finish_operation_operation()
    -> ::core::result::Result<harmos_guest::operations::OperationFactory, harmos_guest::operations::ServingError> {
        harmos_guest::operations::OperationFactory::function(
            FINISH_OPERATION_OPERATION,
            const { &[harmos_guest::Resource::of::<crate::Owner>()] },
            finish_operation_handler,
            |_| harmos_guest::operations::FailureKind::Permanent,
        )
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

#[cfg(target_arch = "wasm32")]
pub use runtime::finish_operation_operation;
