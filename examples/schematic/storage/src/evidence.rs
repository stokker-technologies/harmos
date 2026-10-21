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
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos::{evidence::EvidenceError, evidence::Vault};

/* === Constants === */

const CEILING: usize = 1024 * 1024;

const CADENCE: Duration = Duration::from_millis(250);

const WINDOW: usize = 64 * 1024;

const DIGEST_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

const DIGEST_PRIME: u64 = 0x0000_0100_0000_01b3;

const DRAFTS: &str = "drafts";

const KEPT: &str = "kept";

const QUARANTINE: &str = "quarantine";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct EvidenceDirectory {
    root: PathBuf,
    ceiling: usize,
    cadence: Duration,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl EvidenceDirectory {
    pub fn at(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        for place in [DRAFTS, KEPT, QUARANTINE] {
            fs::create_dir_all(root.join(place))?;
        }
        sync(&root)?;

        Ok(Self { root, ceiling: CEILING, cadence: CADENCE })
    }

    pub fn reading(mut self, bytes: usize) -> Self {
        self.ceiling = bytes.max(1);
        self
    }

    pub fn watching(mut self, interval: Duration) -> Self {
        self.cadence = interval.max(Duration::from_millis(1));
        self
    }

    pub fn quarantined(&self) -> io::Result<Vec<String>> {
        let mut found: Vec<String> = fs::read_dir(self.root.join(QUARANTINE))?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
            .collect();
        found.sort();

        Ok(found)
    }

    fn staged(&self, name: &str) -> PathBuf {
        self.root.join(DRAFTS).join(name)
    }

    fn held(&self, name: &str) -> PathBuf {
        self.root.join(KEPT).join(name)
    }

    fn healed(&self, name: &str) -> io::Result<PathBuf> {
        let kept = self.held(name);
        if !kept.exists() && self.staged(name).exists() {
            self.rename(&self.staged(name), &kept)?;
        }

        Ok(kept)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)?;
        if let Some(parent) = from.parent() {
            sync(parent)?;
        }
        if let Some(parent) = to.parent() {
            sync(parent)?;
        }

        Ok(())
    }

    fn unused(&self, name: &str) -> io::Result<PathBuf> {
        let place = self.root.join(QUARANTINE);
        let bare = place.join(name);
        if !bare.exists() {
            return Ok(bare);
        }
        for again in 1..u32::MAX {
            let candidate = place.join(format!("{name}.{again}"));
            if !candidate.exists() {
                return Ok(candidate);
            }
        }

        Err(io::Error::new(io::ErrorKind::AlreadyExists, "the quarantine has no unused name left"))
    }
}

impl Vault for EvidenceDirectory {
    fn ceiling(&self) -> usize {
        self.ceiling
    }

    fn cadence(&self) -> Duration {
        self.cadence
    }

    async fn open(&self, name: &str) -> Result<u64, EvidenceError> {
        if self.held(name).exists() {
            return Err(EvidenceError::unwritable(format!("`{name}` is already kept, and this directory overwrites nothing")));
        }
        // Created here rather than on the first append, so a draft handed to a
        // foreign process is a file that exists before the process does, and
        // its directory entry is durable before anything is written into it.
        let staged = self.staged(name);
        let fresh = !staged.exists();
        let file = OpenOptions::new().create(true).append(true).open(&staged).map_err(EvidenceError::unwritable)?;
        if fresh {
            sync(&self.root.join(DRAFTS)).map_err(EvidenceError::unwritable)?;
        }

        file.metadata().map(|held| held.len()).map_err(EvidenceError::unreadable)
    }

    async fn external(&self, name: &str) -> Result<Option<PathBuf>, EvidenceError> {
        Ok(Some(self.staged(name)))
    }

    async fn append(&self, name: &str, bytes: &[u8]) -> Result<u64, EvidenceError> {
        let mut file = OpenOptions::new().append(true).open(self.staged(name)).map_err(EvidenceError::unwritable)?;
        file.write_all(bytes).map_err(EvidenceError::unwritable)?;

        file.metadata().map(|held| held.len()).map_err(EvidenceError::unreadable)
    }

    async fn length(&self, name: &str) -> Result<u64, EvidenceError> {
        fs::metadata(self.staged(name)).map(|held| held.len()).map_err(EvidenceError::unreadable)
    }

    async fn verify(&self, name: &str) -> Result<(u64, u64), EvidenceError> {
        // Writable, though nothing here writes: flushing a file's buffers is a
        // write operation on Windows, and `FlushFileBuffers` refuses a handle
        // opened for reading alone with `ERROR_ACCESS_DENIED`. One handle does
        // both halves, so the bytes folded below are the bytes just made
        // durable rather than whatever a second open found.
        let mut file = OpenOptions::new().read(true).write(true).open(self.staged(name)).map_err(EvidenceError::unreadable)?;
        // Durable first. A digest folded from bytes that have not reached the
        // disk describes a file that may not survive the fact about to name it.
        file.sync_data().map_err(EvidenceError::unwritable)?;

        let mut window = vec![0; WINDOW];
        let mut digest = DIGEST_OFFSET;
        let mut length = 0;
        loop {
            let read = file.read(&mut window).map_err(EvidenceError::unreadable)?;
            if read == 0 {
                break;
            }
            digest = fold(digest, &window[..read]);
            length += read as u64;
        }

        Ok((digest, length))
    }

    async fn claim(&self, name: &str) -> Result<(), EvidenceError> {
        self.rename(&self.staged(name), &self.held(name)).map_err(EvidenceError::unwritable)
    }

    async fn quarantine(&self, name: &str) -> Result<(), EvidenceError> {
        let staged = self.staged(name);
        if !staged.exists() {
            return Ok(());
        }
        let aside = self.unused(name).map_err(EvidenceError::unwritable)?;

        self.rename(&staged, &aside).map_err(EvidenceError::unwritable)
    }

    async fn read(&self, name: &str, offset: u64, limit: usize) -> Result<(Vec<u8>, u64), EvidenceError> {
        let kept = self.healed(name).map_err(EvidenceError::unreadable)?;

        window(&kept, offset, limit.min(self.ceiling))
    }

    async fn read_draft(&self, name: &str, offset: u64, limit: usize) -> Result<(Vec<u8>, u64), EvidenceError> {
        window(&self.staged(name), offset, limit.min(self.ceiling))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn window(path: &Path, offset: u64, limit: usize) -> Result<(Vec<u8>, u64), EvidenceError> {
    let mut file = File::open(path).map_err(EvidenceError::unreadable)?;
    let length = file.metadata().map(|held| held.len()).map_err(EvidenceError::unreadable)?;
    if offset >= length {
        return Ok((Vec::new(), length));
    }
    file.seek(SeekFrom::Start(offset)).map_err(EvidenceError::unreadable)?;
    let wanted = limit.min((length - offset) as usize);
    let mut bytes = vec![0; wanted];
    file.read_exact(&mut bytes).map_err(EvidenceError::unreadable)?;

    Ok((bytes, length))
}

fn fold(digest: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(digest, |held, byte| (held ^ u64::from(*byte)).wrapping_mul(DIGEST_PRIME))
}

fn sync(directory: &Path) -> io::Result<()> {
    if cfg!(unix) { File::open(directory)?.sync_all() } else { Ok(()) }
}

/* === Tests === */
