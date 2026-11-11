/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub(crate) mod access;
mod initialize;
mod shutdown;
mod state;

/* === Re-Exports === */

pub use harmos_operations::model::operations::lifecycle::*;
pub use initialize::operation as initialize_operation;
pub use initialize::{ControlError, INITIALIZE_DESCRIPTION, INITIALIZE_OPERATION, Initialize};
pub use shutdown::operation as shutdown_operation;
pub use shutdown::{SHUTDOWN_DESCRIPTION, SHUTDOWN_OPERATION, Shutdown};
pub use state::InstanceState;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    #[test]
    fn lifecycle_descriptors_match_the_guest_carriers() {
        use super::{Action, Descriptor, INITIALIZE_OPERATION, POLICY, SHUTDOWN_OPERATION};
        for (action, definition) in [
            (Action::Initialize, INITIALIZE_OPERATION.declared().expect("valid operation")),
            (Action::Shutdown, SHUTDOWN_OPERATION.declared().expect("valid operation")),
        ] {
            let descriptor = Descriptor::new(action, POLICY).unwrap();
            assert_eq!(descriptor.contract().operation(), &definition);
        }
    }
}
