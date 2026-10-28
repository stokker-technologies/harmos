/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use schematic_app::error::{DocumentError, LaunchError, SchematicError};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Incompatible(#[from] harmos::runtime::Incompatible),

    #[error(transparent)]
    Document(#[from] DocumentError),

    #[error(transparent)]
    Launch(#[from] LaunchError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Guest(#[from] harmos::guest::Error),

    #[error(transparent)]
    Edit(#[from] harmos::Error<SchematicError>),

    #[error(transparent)]
    State(#[from] harmos::Error),
}
