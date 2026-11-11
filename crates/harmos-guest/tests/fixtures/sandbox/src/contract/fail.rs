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
use super::{Echoed, PROBE};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(::harmos_guest::Message, Debug, Default)]
#[harmos_path(::harmos_guest)]
#[derive(::harmos_guest::Request)]
#[request(response = Echoed)]
#[derive(Clone, PartialEq)]

pub struct FailOperation {}

pub static FAIL_OPERATION_DESCRIPTION: ::harmos_guest::operations::WireDeclaration =
    ::harmos_guest::operations::WireDeclaration::request::<FailOperation>("probe.fail", PROBE);

pub const FAIL_OPERATION_OPERATION: ::harmos_guest::operations::OperationSpec<FailOperation> =
    ::harmos_guest::operations::OperationSpec::<FailOperation>::new(&FAIL_OPERATION_DESCRIPTION);

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
    use super::{Echoed, FailOperation};
    use crate::contract::FAIL_OPERATION_OPERATION;
    use harmos_guest::OperationRefusal;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    pub async fn fail_operation_handler(
        _request: FailOperation, _sdk: harmos_guest::operations::FunctionContext<FailOperation>,
    ) -> Result<harmos_guest::operations::Response<Echoed>, OperationRefusal> {
        let result: Result<Echoed, OperationRefusal> = async move { panic!("requested guest trap") }.await;
        result.map(Into::into)
    }

    pub fn fail_operation_operation()
    -> ::core::result::Result<harmos_guest::operations::OperationFactory, harmos_guest::operations::ServingError> {
        harmos_guest::operations::OperationFactory::function(FAIL_OPERATION_OPERATION, const { &[] }, fail_operation_handler, |_| {
            harmos_guest::operations::FailureKind::Permanent
        })
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

#[cfg(target_arch = "wasm32")]
pub use runtime::fail_operation_operation;
