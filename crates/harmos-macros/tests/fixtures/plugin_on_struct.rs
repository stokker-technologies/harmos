/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Identity Is Authored On The Unified Implementation, Where Its Contract Is Known
#[harmos::plugin(description = "A bridge.", id = "bridge", requires = ())]
pub struct Bridge;


impl harmos::Plugin for Bridge {}


fn main() {}
