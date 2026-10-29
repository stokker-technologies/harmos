/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


struct Program;


#[harmos::plugin(id = "invalid_requirement", requires(harmos::builtins::lifecycle::Status: scope("")))]
impl harmos::Plugin for Program {}


fn main() {}
