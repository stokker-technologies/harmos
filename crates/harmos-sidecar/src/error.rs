/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(feature = "kit")]
    #[error(transparent)]
    Executable(#[from] crate::ExecutableError),

    #[error(transparent)]
    Declaration(#[from] crate::inventory::DeclarationError),

    #[error(transparent)]
    Config(#[from] crate::ConfigError),

    #[error(transparent)]
    Registration(#[from] crate::RegistrationError),

    #[cfg(feature = "kit")]
    #[error(transparent)]
    Call(#[from] crate::CallError),

    #[cfg(feature = "kit")]
    #[error(transparent)]
    Publication(#[from] crate::PublicationError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
