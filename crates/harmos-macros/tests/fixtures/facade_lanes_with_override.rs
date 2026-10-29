/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


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
