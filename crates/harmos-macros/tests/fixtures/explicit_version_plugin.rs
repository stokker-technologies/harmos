/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Executable versions come from Cargo; an authored second version is refused.
pub struct Settings;

pub struct Bridge;


#[harmos::plugin(description = "A bridge.", id = "bridge", requires = (), version = "1.0.0")]
impl harmos::Plugin for Bridge {}


fn main() {}
