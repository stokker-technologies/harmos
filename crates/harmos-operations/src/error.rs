/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Dependency(#[from] harmos_sdk::DependencyError),
    #[error("operation instance `{0}` is not registered for this caller")]
    UnknownInstance(u64),
    #[error(transparent)]
    Stream(#[from] harmos_sdk::operations::StreamError),
}
