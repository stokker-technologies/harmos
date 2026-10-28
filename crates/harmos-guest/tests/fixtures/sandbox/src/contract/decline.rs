/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Decline Operation Contract And Execution.

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

#[cfg_attr(not(target_arch = "wasm32"), harmos_domain::operation(harmos_path = ::harmos_domain, id = "probe.decline", response = Echoed, policy = PROBE))]
#[cfg_attr(target_arch = "wasm32", harmos::guest::operation(harmos_path = ::harmos::guest, id = "probe.decline", response = Echoed, policy = PROBE))]
#[derive(Clone, PartialEq)]

pub struct DeclineOperation {}

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
    use harmos::guest::OperationRefusal;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl harmos::guest::Operation for DeclineOperation {
        type Error = OperationRefusal;

        async fn run(
            self, _sdk: harmos::guest::Context<'_, Self>,
        ) -> Result<harmos::guest::operations::Response<Echoed>, OperationRefusal> {
            let result: Result<Echoed, OperationRefusal> = async move { Err(OperationRefusal::new(7, "the probe declines")) }.await;
            result.map(Into::into)
        }
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}
