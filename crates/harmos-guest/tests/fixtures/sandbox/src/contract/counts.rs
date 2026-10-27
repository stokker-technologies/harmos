/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Counts Operation Contract And Execution.

/* === --- === Definition Module === --- === */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{ReadOperation, Value};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_guest::operation(harmos_path = ::harmos_guest,
    resources = [crate::Owner], id = "fixture.counts", response = Value, policy = ReadOperation::POLICY)]
#[derive(Clone, PartialEq)]

pub struct CountsOperation {
    pub value: String,
}

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

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl harmos_guest::Operation for CountsOperation {
        type Error = std::io::Error;

        async fn run(self, sdk: harmos_guest::Context<'_, Self>) -> Result<harmos_guest::operations::Response<Value>, std::io::Error> {
            let owner = sdk.resource::<crate::Owner>();
            Ok(Value {
                value: owner.ran.load(std::sync::atomic::Ordering::SeqCst).to_string(),
            }
            .into())
        }
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}
