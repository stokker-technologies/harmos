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
use crate::state::{NetId, SymbolId};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type LaunchError = Error;

pub type Result<T> = std::result::Result<T, Error>;

/* === Structs === */

/* === Enums === */

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SchematicError {
    #[error("no symbol is placed under #{}", .0.0)]
    UnknownSymbol(SymbolId),

    #[error("a symbol is already placed under #{}", .0.0)]
    SymbolTaken(SymbolId),

    #[error("no net exists under #{}", .0.0)]
    UnknownNet(NetId),

    #[error("another net is already called {0}")]
    NameTaken(String),

    #[error("a connection joins at least two pins")]
    TooFewPins,

    #[error("a route must contain orthogonal segments, or be empty to clear it")]
    InvalidRoute,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("{variable} is not set")]
    Missing { variable: &'static str },

    #[error("{variable} is set to an empty value")]
    Empty { variable: &'static str },

    #[error("{variable} is not valid unicode")]
    NotUnicode { variable: &'static str },
}

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Decode(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Incompatible(#[from] harmos::runtime::Incompatible),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
