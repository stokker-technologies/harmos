/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Re-Exports === */

pub use harmos_domain::streams::StreamSpec;

/* === Imports === */

use super::StreamDeclaration;
use harmos_domain::config::SchemaSettings;

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Stream(#[from] harmos_domain::streams::Error),
    #[error("stream `{id}` is declared more than once")]
    DuplicateStream { id: String },
}

/* === Public Functions === */

pub fn validate(stream: &StreamDeclaration) -> Result<(), Error> {
    validate_with_settings(stream, &SchemaSettings::default())
}

pub fn validate_with_settings(stream: &StreamDeclaration, settings: &SchemaSettings) -> Result<(), Error> {
    harmos_domain::streams::validate(&stream.metadata, &stream.row_metadata, &stream.id, stream.window, &stream.row, settings)?;
    Ok(())
}

/* === Tests === */
