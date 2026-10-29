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
    #[error(transparent)]
    Declaration(#[from] crate::inventory::DeclarationError),

    #[error(transparent)]
    Config(#[from] crate::ConfigError),

    #[error(transparent)]
    Registration(#[from] crate::RegistrationError),

    #[error(transparent)]
    Message(#[from] crate::MessageError),
}
