/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[harmos::lanes]
pub enum Lanes {
    #[default]
    Open,

    #[lane(serial)]
    Analysis,
}
