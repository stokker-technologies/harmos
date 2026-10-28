/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// The regression half of facade_lanes_with_override.rs.
#[facade::lanes]
enum Lanes {
    #[default]
    Open,

    #[lane(serial)]
    Workflow,
}


fn main() {
    let _ = <Lanes as facade::__harmos::operations::Lane>::catalog();
}
