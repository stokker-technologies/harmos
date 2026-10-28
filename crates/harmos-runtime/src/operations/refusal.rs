/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("the operation deadline elapsed")]
    TimedOut,

    #[error("the operation was cancelled")]
    Cancelled,

    #[error("the operation runtime became unavailable")]
    Unavailable,
}
