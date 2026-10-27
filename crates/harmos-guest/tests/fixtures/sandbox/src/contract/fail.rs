/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Fail Operation Contract And Execution.

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

#[harmos_guest::operation(harmos_path = ::harmos_guest, id = "probe.fail", response = Echoed, policy = PROBE)]
#[derive(Clone, PartialEq)]

pub struct FailOperation {}

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
    use harmos_guest::OperationRefusal;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl harmos_guest::Operation for FailOperation {
        type Error = OperationRefusal;

        async fn run(self, _sdk: harmos_guest::Context<'_, Self>) -> Result<harmos_guest::operations::Response<Echoed>, OperationRefusal> {
            let result: Result<Echoed, OperationRefusal> = async move { panic!("requested guest trap") }.await;
            result.map(Into::into)
        }
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}
