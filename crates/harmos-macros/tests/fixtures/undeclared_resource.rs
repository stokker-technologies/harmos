/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Undeclared Resources Cannot Be Accessed Through An Authored Context.


#[path = "policy.rs"]
mod controls;


#[harmos::operation(id = "test.read", policy = controls::POLICY)]
struct Read;


fn undeclared(ctx: harmos::Context<'_, Read>) {
    let _ = ctx.resource::<u64>();
}

fn main() {}
