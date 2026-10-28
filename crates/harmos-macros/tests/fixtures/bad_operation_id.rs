/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


#[path = "policy.rs"]
mod controls;


#[harmos::operation(id = "bad.Segment", policy = controls::POLICY)]
struct Read;


fn main() {}
