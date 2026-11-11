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
use super::{READ_OPERATION_OPERATION, Value};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[::harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(::harmos_guest::Request)]
#[harmos_path(::harmos_guest)]
#[request(response = Value)]
#[derive(Clone, PartialEq)]

pub struct CountsOperation {
    pub value: String,
}

pub static COUNTS_OPERATION_DESCRIPTION: ::harmos_guest::operations::WireDeclaration =
    ::harmos_guest::operations::WireDeclaration::request::<CountsOperation>("fixture.counts", READ_OPERATION_OPERATION.entry().policy);

pub const COUNTS_OPERATION_OPERATION: ::harmos_guest::operations::OperationSpec<CountsOperation> =
    ::harmos_guest::operations::OperationSpec::<CountsOperation>::new(&COUNTS_OPERATION_DESCRIPTION);

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
    use super::{CountsOperation, Value};
    use crate::contract::COUNTS_OPERATION_OPERATION;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    pub async fn counts_operation_handler(
        _request: CountsOperation, sdk: harmos_guest::operations::FunctionContext<CountsOperation>,
    ) -> Result<harmos_guest::operations::Response<Value>, std::io::Error> {
        let owner = sdk.resource::<crate::Owner>().expect("declared resource is registered");
        Ok(Value {
            value: owner.ran.load(std::sync::atomic::Ordering::SeqCst).to_string(),
        }
        .into())
    }

    pub fn counts_operation_operation()
    -> ::core::result::Result<harmos_guest::operations::OperationFactory, harmos_guest::operations::ServingError> {
        harmos_guest::operations::OperationFactory::function(
            COUNTS_OPERATION_OPERATION,
            const { &[harmos_guest::Resource::of::<crate::Owner>()] },
            counts_operation_handler,
            |_| harmos_guest::operations::FailureKind::Permanent,
        )
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

#[cfg(target_arch = "wasm32")]
pub use runtime::counts_operation_operation;
