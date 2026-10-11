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
use std::io;
use std::path::{Path, PathBuf};
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use crate::app::{Origin, SchematicEditor};
use crate::error::DocumentError;
use crate::state::Schematic;
use harmos::Position;
use jsonl_adapter::Jsonl;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Debug, Deserialize, Serialize)]
struct Artifact {
    generation: u64,
    position: Position,
    schematic: Schematic,
}

#[derive(Clone, Debug)]
pub struct Document {
    origin: Schematic,
    generation: u64,
    position: Position,
    sidecar: PathBuf,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Document {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            origin: Schematic::default(),
            generation: 1,
            position: Position::ORIGIN,
            sidecar: sidecar_of(path.as_ref()),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(path = %path.as_ref().display()))]
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DocumentError> {
        let path = path.as_ref();
        let artifact: Artifact =
            serde_json::from_reader(io::BufReader::new(std::fs::File::open(path)?))?;

        Ok(Self {
            origin: artifact.schematic,
            generation: artifact.generation,
            position: artifact.position,
            sidecar: sidecar_of(path),
        })
    }

    pub fn sidecar(&self) -> &Path {
        &self.sidecar
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn into_origin(self) -> Origin {
        Origin::at(self.origin, self.position)
    }

    pub fn recovering(&self) -> Result<Jsonl<SchematicEditor>, io::Error> {
        Jsonl::sidecar(&self.sidecar, self.generation, self.position)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn sidecar_of(path: &Path) -> PathBuf {
    path.with_extension("jsonl")
}

/* === Tests === */
