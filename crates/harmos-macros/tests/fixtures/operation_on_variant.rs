/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


#[harmos::catalog]
enum Operations {
    #[operation(response = (), policy = ())]
    Read,
}


fn main() {}
