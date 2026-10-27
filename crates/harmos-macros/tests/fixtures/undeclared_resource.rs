/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Undeclared Resources Cannot Be Accessed Through An Authored Context.

/* === Modules === */

#[path = "policy.rs"]
mod controls;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(id = "test.read", policy = controls::POLICY)]
struct Read;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn undeclared(ctx: harmos::Context<'_, Read>) {
    let _ = ctx.resource::<u64>();
}

fn main() {}

/* === Tests === */
