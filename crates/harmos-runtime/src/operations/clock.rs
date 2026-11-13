/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Supply Runtime Clock Resources to Host Operations.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Clock(pub(crate) crate::clock::Clock);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_operations::clock::Clock for Clock {
    fn now(&self) -> prost_types::Timestamp {
        self.0.date().into()
    }
    fn sleep(&self, duration: std::time::Duration) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(crate::clock::waiting(self.0.sleep(duration)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
