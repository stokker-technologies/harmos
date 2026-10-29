/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


#[harmos::lanes]
enum Lanes {
    #[default]
    #[lane(serial)]
    Open,
}


fn main() {}
