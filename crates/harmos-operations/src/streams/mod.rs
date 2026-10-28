/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod publish;

pub use publish::{FollowingRow, Publish};

#[harmos::catalog(harmos_path = crate)]
pub enum Streams {
    Publish(Publish),
}
