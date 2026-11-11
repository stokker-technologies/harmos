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

pub struct SpinOperation {}

pub static SPIN_OPERATION_DESCRIPTION: ::harmos_guest::operations::WireDeclaration =
    ::harmos_guest::operations::WireDeclaration::request::<SpinOperation>(
        "probe.spin",
        OperationPolicy {
            timeout: Duration::from_secs(600),
            attempt_timeout: Duration::from_secs(600),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    );

pub const SPIN_OPERATION_OPERATION: ::harmos_guest::operations::OperationSpec<SpinOperation> =
    ::harmos_guest::operations::OperationSpec::<SpinOperation>::new(&SPIN_OPERATION_DESCRIPTION);

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
    use crate::contract::SPIN_OPERATION_OPERATION;
    use harmos_guest::OperationRefusal;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    pub async fn spin_operation_handler(
        _request: SpinOperation, _sdk: harmos_guest::operations::FunctionContext<SpinOperation>,
    ) -> Result<harmos_guest::operations::Response<Echoed>, OperationRefusal> {
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

    pub fn spin_operation_operation()
    -> ::core::result::Result<harmos_guest::operations::OperationFactory, harmos_guest::operations::ServingError> {
        harmos_guest::operations::OperationFactory::function(SPIN_OPERATION_OPERATION, const { &[] }, spin_operation_handler, |_| {
            harmos_guest::operations::FailureKind::Permanent
        })
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

#[cfg(target_arch = "wasm32")]
pub use runtime::spin_operation_operation;
