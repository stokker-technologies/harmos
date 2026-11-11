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

#[::harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(::harmos_guest::Request)]
#[harmos_path(::harmos_guest)]
#[request(response = Echoed)]
#[derive(Clone, PartialEq)]

pub struct DeclineOperation {}

pub static DECLINE_OPERATION_DESCRIPTION: ::harmos_guest::operations::WireDeclaration =
    ::harmos_guest::operations::WireDeclaration::request::<DeclineOperation>("probe.decline", PROBE);

pub const DECLINE_OPERATION_OPERATION: ::harmos_guest::operations::OperationSpec<DeclineOperation> =
    ::harmos_guest::operations::OperationSpec::<DeclineOperation>::new(&DECLINE_OPERATION_DESCRIPTION);

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
    use super::{DeclineOperation, Echoed};
    use crate::contract::DECLINE_OPERATION_OPERATION;
    use harmos_guest::OperationRefusal;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    pub async fn decline_operation_handler(
        _request: DeclineOperation, _sdk: harmos_guest::operations::FunctionContext<DeclineOperation>,
    ) -> Result<harmos_guest::operations::Response<Echoed>, OperationRefusal> {
        let result: Result<Echoed, OperationRefusal> = async move { Err(OperationRefusal::new(7, "the probe declines")) }.await;
        result.map(Into::into)
    }

    pub fn decline_operation_operation()
    -> ::core::result::Result<harmos_guest::operations::OperationFactory, harmos_guest::operations::ServingError> {
        harmos_guest::operations::OperationFactory::function(DECLINE_OPERATION_OPERATION, const { &[] }, decline_operation_handler, |_| {
            harmos_guest::operations::FailureKind::Permanent
        })
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

#[cfg(target_arch = "wasm32")]
pub use runtime::decline_operation_operation;
