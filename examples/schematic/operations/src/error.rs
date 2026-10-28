/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid analysis input")]
    InvalidInput,

    #[error("analysis resources are unavailable")]
    Unavailable,

    #[error(transparent)]
    Resource(#[from] harmos::ResourceError),

    #[error(transparent)]
    Invocation(#[from] harmos::operations::InvocationError),

    #[error(transparent)]
    Output(#[from] harmos::Error),

    #[error(transparent)]
    Stream(#[from] harmos::operations::StreamError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("directory name is not UTF-8: {0:?}")]
    DirectoryName(std::ffi::OsString),

    #[error(transparent)]
    Host(Box<dyn std::error::Error + Send + Sync>),
}
