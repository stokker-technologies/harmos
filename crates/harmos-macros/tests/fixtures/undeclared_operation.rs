/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Undeclared Child Operations Cannot Be Called Through An Authored Context.


#[path = "policy.rs"]
mod controls;


#[harmos::operation(id = "test.read", policy = controls::POLICY)]
struct Read;

#[harmos::operation(id = "test.other", policy = controls::POLICY)]
struct Other;


fn undeclared(ctx: harmos::Context<'_, Read>) {
    std::mem::drop(ctx.operation(Other));
}

fn main() {}
