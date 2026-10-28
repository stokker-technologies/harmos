/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod voltage;

pub use voltage::NetVoltage;

#[harmos::streams]
pub enum Streams {
    NetVoltage(NetVoltage),
}
