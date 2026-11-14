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

use harmos_runtime::state::At;

// Standard Library Imports
use std::io;
use std::io::Write;
use std::path::Path;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use crate::app::Origin;
use crate::error::DocumentError;
use crate::state::Schematic;
use harmos_runtime::state::Position;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Debug, Deserialize, Serialize)]
struct Artifact {
    position: Position,
    schematic: Schematic,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Document {
    origin: Schematic,
    position: Position,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Document {
    pub fn new() -> Self {
        Self {
            origin: Schematic::default(),
            position: Position::ORIGIN,
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(path = %path.as_ref().display()))]
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DocumentError> {
        let path = path.as_ref();
        let file = std::fs::File::open(path)?;
        let reader = io::BufReader::new(file);
        let artifact: Artifact = serde_json::from_reader(reader)?;

        Ok(Self {
            origin: artifact.schematic,
            position: artifact.position,
        })
    }

    pub fn save(path: &Path, snapshot: At<Schematic>) -> Result<(), DocumentError> {
        let artifact = Artifact {
            position: snapshot.position,
            schematic: snapshot.value,
        };
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temporary, &artifact)?;
        temporary.flush()?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }

    pub fn into_origin(self) -> Origin {
        Origin::at(self.origin, self.position)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
