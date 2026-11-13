/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use schematic_app::error::{DocumentError, LaunchError, SchematicError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Result<T> = std::result::Result<T, Error>;

/* === Structs === */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Incompatible(#[from] harmos_runtime::runtime::Incompatible),

    #[error(transparent)]
    Document(#[from] DocumentError),

    #[error(transparent)]
    Launch(#[from] LaunchError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Guest(#[from] harmos_runtime::guest::Error),

    #[error(transparent)]
    Edit(#[from] harmos_runtime::Error<SchematicError>),

    #[error(transparent)]
    State(#[from] harmos_runtime::Error),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
