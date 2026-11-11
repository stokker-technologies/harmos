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

pub use harmos_domain::model::operations::lifecycle::*;
pub use initialize::{ControlError, Initialize};
pub use shutdown::Shutdown;
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
        use super::{Action, Descriptor, Initialize, POLICY, Shutdown};
        for (action, definition) in [(Action::Initialize, Initialize::definition()), (Action::Shutdown, Shutdown::definition())] {
            let descriptor = Descriptor::new(action, POLICY).unwrap();
            assert_eq!(descriptor.contract().operation(), &definition);
        }
    }
}
