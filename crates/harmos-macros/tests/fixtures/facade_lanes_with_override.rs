/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// `lanes` used to take no arguments at all; proving the facade override
// reaches it too covers the highest-risk edge of this change.
#[facade::lanes(harmos_path = ::facade::__harmos)]
enum Lanes {
    #[default]
    Open,

    #[lane(serial)]
    Workflow,
}


fn main() {
    let _ = <Lanes as facade::__harmos::operations::Lane>::catalog();
}
