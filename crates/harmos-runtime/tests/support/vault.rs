/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// An Evidence Vault Over A Scratch Directory For Stories That Restart A Runtime.
// Drafts are plain files, so a foreign writer can be handed one and a reopened
// directory still holds what a stop or crash left behind.

#![allow(dead_code)]
/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos_runtime::{evidence::EvidenceError, evidence::Vault};

/* === Constants === */
const CEILING: usize = 1024 * 1024;

const CADENCE: Duration = Duration::from_millis(250);

// A Fixed FNV-1a Fold Keeps Digests Identical Across Runs.
const DIGEST_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

const DIGEST_PRIME: u64 = 0x0000_0100_0000_01b3;

const DRAFTS: &str = "drafts";

const KEPT: &str = "kept";

const QUARANTINE: &str = "quarantine";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone)]
pub struct Directory {
    root: PathBuf,
    ceiling: usize,
    cadence: Duration,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Directory {
    pub fn at(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        for place in [DRAFTS, KEPT, QUARANTINE] {
            fs::create_dir_all(root.join(place)).expect("the scratch directory is writable");
        }

        Self {
            root,
            ceiling: CEILING,
            cadence: CADENCE,
        }
    }

    pub fn reading(mut self, bytes: usize) -> Self {
        self.ceiling = bytes.max(1);
        self
    }

    pub fn watching(mut self, interval: Duration) -> Self {
        self.cadence = interval.max(Duration::from_millis(1));
        self
    }

    pub fn quarantined(&self) -> Vec<String> {
        let mut found: Vec<String> = fs::read_dir(self.root.join(QUARANTINE))
            .expect("the quarantine is readable")
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
            .collect();
        found.sort();

        found
    }

    fn staged(&self, name: &str) -> PathBuf {
        self.root.join(DRAFTS).join(name)
    }

    fn held(&self, name: &str) -> PathBuf {
        self.root.join(KEPT).join(name)
    }

    fn unused(&self, name: &str) -> PathBuf {
        let place = self.root.join(QUARANTINE);

        std::iter::once(place.join(name))
            .chain((1..).map(|again| place.join(format!("{name}.{again}"))))
            .find(|candidate| !candidate.exists())
            .expect("an unused quarantine name")
    }
}

impl Vault for Directory {
    fn ceiling(&self) -> usize {
        self.ceiling
    }

    fn cadence(&self) -> Duration {
        self.cadence
    }

    async fn open(&self, name: &str) -> Result<u64, EvidenceError> {
        if self.held(name).exists() {
            return Err(EvidenceError::unwritable(format!(
                "`{name}` is already kept, and this directory overwrites nothing"
            )));
        }
        // An unfinished draft is resumed rather than replaced.
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.staged(name))
            .map_err(EvidenceError::unwritable)?;

        length(&file)
    }

    async fn external(&self, name: &str) -> Result<Option<PathBuf>, EvidenceError> {
        Ok(Some(self.staged(name)))
    }

    async fn append(&self, name: &str, bytes: &[u8]) -> Result<u64, EvidenceError> {
        let mut file = OpenOptions::new()
            .append(true)
            .open(self.staged(name))
            .map_err(EvidenceError::unwritable)?;
        file.write_all(bytes).map_err(EvidenceError::unwritable)?;

        length(&file)
    }

    async fn length(&self, name: &str) -> Result<u64, EvidenceError> {
        fs::metadata(self.staged(name))
            .map(|held| held.len())
            .map_err(EvidenceError::unreadable)
    }

    async fn verify(&self, name: &str) -> Result<(u64, u64), EvidenceError> {
        let bytes = fs::read(self.staged(name)).map_err(EvidenceError::unreadable)?;
        let digest = bytes.iter().fold(DIGEST_OFFSET, |held, byte| {
            (held ^ u64::from(*byte)).wrapping_mul(DIGEST_PRIME)
        });

        Ok((digest, bytes.len() as u64))
    }

    async fn claim(&self, name: &str) -> Result<(), EvidenceError> {
        fs::rename(self.staged(name), self.held(name)).map_err(EvidenceError::unwritable)
    }

    async fn quarantine(&self, name: &str) -> Result<(), EvidenceError> {
        let staged = self.staged(name);
        if !staged.exists() {
            return Ok(());
        }

        fs::rename(&staged, self.unused(name)).map_err(EvidenceError::unwritable)
    }

    async fn read(
        &self,
        name: &str,
        offset: u64,
        limit: usize,
    ) -> Result<(Vec<u8>, u64), EvidenceError> {
        // A fact naming these bytes is durable before the claim runs, so bytes
        // still staged under a finalized name are bookkeeping catching up.
        let kept = self.held(name);
        if !kept.exists() && self.staged(name).exists() {
            fs::rename(self.staged(name), &kept).map_err(EvidenceError::unreadable)?;
        }

        window(&kept, offset, limit.min(self.ceiling))
    }

    async fn read_draft(
        &self,
        name: &str,
        offset: u64,
        limit: usize,
    ) -> Result<(Vec<u8>, u64), EvidenceError> {
        window(&self.staged(name), offset, limit.min(self.ceiling))
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn length(file: &File) -> Result<u64, EvidenceError> {
    file.metadata()
        .map(|held| held.len())
        .map_err(EvidenceError::unreadable)
}

fn window(path: &Path, offset: u64, limit: usize) -> Result<(Vec<u8>, u64), EvidenceError> {
    let mut file = File::open(path).map_err(EvidenceError::unreadable)?;
    let length = length(&file)?;
    if offset >= length {
        return Ok((Vec::new(), length));
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(EvidenceError::unreadable)?;
    let wanted = limit.min((length - offset) as usize);
    let mut bytes = vec![0; wanted];
    file.read_exact(&mut bytes)
        .map_err(EvidenceError::unreadable)?;

    Ok((bytes, length))
}

/* === Tests === */
