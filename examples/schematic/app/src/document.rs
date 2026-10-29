/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app::{Origin, SchematicEditor};
use crate::error::DocumentError;
use crate::state::Schematic;
use harmos::state::Position;
use schematic_storage::{Jsonl, Sidecar};

#[derive(Debug, Deserialize, Serialize)]
struct Artifact {
    generation: u64,
    position: Position,
    schematic: Schematic,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Document {
    origin: Schematic,
    generation: u64,
    position: Position,
    journal: PathBuf,
}

impl Document {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            origin: Schematic::default(),
            generation: 1,
            position: Position::ORIGIN,
            journal: journal_of(path.as_ref()),
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
            generation: artifact.generation,
            position: artifact.position,
            journal: journal_of(path),
        })
    }

    pub fn save(path: &Path, snapshot: harmos::state::At<Schematic>, checkpoint: &Sidecar) -> Result<(), DocumentError> {
        checkpoint.save(|generation, position| {
            if position != snapshot.position {
                return Err(io::Error::other("snapshot and settled journal positions differ"));
            }
            let artifact = Artifact {
                generation,
                position,
                schematic: snapshot.value,
            };
            let parent = path.parent().unwrap_or_else(|| Path::new("."));
            let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
            serde_json::to_writer_pretty(&mut temporary, &artifact)?;
            temporary.flush()?;
            temporary.as_file().sync_all()?;
            temporary.persist(path).map_err(|error| error.error)?;
            Ok(())
        })?;
        Ok(())
    }

    pub fn journal(&self) -> &Path {
        &self.journal
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn into_origin(self) -> Origin {
        Origin::at(self.origin, self.position)
    }

    pub fn recovering(&self) -> Result<Jsonl<SchematicEditor>, io::Error> {
        Jsonl::sidecar(&self.journal, self.generation, self.position)
    }
}

fn journal_of(path: &Path) -> PathBuf {
    path.with_extension("jsonl")
}
