/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Release Dropped Guest Children

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::{Child, GuestChildAction, Message};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<R: Message> Drop for Child<R> {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        let context = self.context.clone();
        let instance = self.instance;
        wit_bindgen::spawn_local(async move {
            let _ = context.child_control(instance, GuestChildAction::Release).await;
        });
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
