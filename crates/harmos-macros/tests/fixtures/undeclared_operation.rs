/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Undeclared Child Operations Cannot Be Called Through An Authored Context.

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

#[harmos::operation(id = "test.other", policy = controls::POLICY)]
struct Other;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn undeclared(ctx: harmos::Context<'_, Read>) {
    std::mem::drop(ctx.operation(Other));
}

fn main() {}

/* === Tests === */
