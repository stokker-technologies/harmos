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
    #[lane(serial)]
    Device,
}


fn main() {}
