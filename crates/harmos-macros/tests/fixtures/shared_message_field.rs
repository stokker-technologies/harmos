/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


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
#[harmos::message]
struct Leaf {
    value: u64,
}

// Shared ownership has no message wire meaning; only `Box<T>` is admitted.
#[harmos::message]
struct Holder {
    row: std::sync::Arc<Leaf>,
}

/* === Enums === */
#[harmos::message]
enum Shared {
    Owned(std::rc::Rc<Leaf>),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn main() {}

/* === Tests === */
