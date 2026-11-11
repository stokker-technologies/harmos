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
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

// Local Imports
use harmos_domain::state::Position;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub bytes: usize,
    pub digest: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotManifest {
    pub position: Option<Position>,
    pub count: usize,
    pub total_bytes: usize,
    pub segments: Vec<Segment>,
}

#[derive(Clone, Debug)]
pub struct SnapshotPayload {
    file: Arc<NamedTempFile>,
    manifest: SnapshotManifest,
}

pub struct SnapshotWriter {
    file: BufWriter<NamedTempFile>,
    limit: usize,
    bytes: usize,
    total_bytes: usize,
    digest: Sha256,
    segments: Vec<Segment>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl SnapshotWriter {
    pub fn new(limit: NonZeroUsize) -> io::Result<Self> {
        Ok(Self {
            file: BufWriter::new(NamedTempFile::new()?),
            limit: limit.get(),
            bytes: 0,
            total_bytes: 0,
            digest: Sha256::new(),
            segments: Vec::new(),
        })
    }

    pub fn finish_segment(&mut self) {
        if self.bytes != 0 {
            self.segments.push(Segment {
                bytes: self.bytes,
                digest: self.digest.finalize_reset().into(),
            });
            self.bytes = 0;
        }
    }

    pub fn finish(mut self, position: Position) -> io::Result<SnapshotPayload> {
        self.flush()?;
        self.finish_segment();
        let file = self.file.into_inner().map_err(|error| error.into_error())?;
        Ok(SnapshotPayload {
            file: Arc::new(file),
            manifest: SnapshotManifest {
                position: Some(position),
                count: self.segments.len(),
                total_bytes: self.total_bytes,
                segments: self.segments,
            },
        })
    }
}

impl Write for SnapshotWriter {
    fn write(&mut self, mut bytes: &[u8]) -> io::Result<usize> {
        let length = bytes.len();
        let total = self.total_bytes.checked_add(length).ok_or_else(|| io::Error::other("snapshot size overflow"))?;
        while !bytes.is_empty() {
            let count = bytes.len().min(self.limit - self.bytes);
            self.file.write_all(&bytes[..count])?;
            self.digest.update(&bytes[..count]);
            self.bytes += count;
            bytes = &bytes[count..];
            if self.bytes == self.limit {
                self.finish_segment();
            }
        }
        self.total_bytes = total;
        Ok(length)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl SnapshotPayload {
    pub fn from_bytes(bytes: &[u8], limit: NonZeroUsize, position: Position) -> io::Result<Self> {
        let mut writer = SnapshotWriter::new(limit)?;
        writer.write_all(bytes)?;
        writer.finish(position)
    }

    pub(crate) fn legacy(bytes: &[u8], limit: NonZeroUsize) -> io::Result<Self> {
        let mut payload = Self::from_bytes(bytes, limit, Position::ORIGIN)?;
        payload.manifest.position = None;
        Ok(payload)
    }

    pub fn manifest(&self) -> &SnapshotManifest {
        &self.manifest
    }

    pub fn manifest_digest(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        for segment in &self.manifest.segments {
            digest.update((segment.bytes as u64).to_le_bytes());
            digest.update(segment.digest);
        }
        digest.finalize().into()
    }

    pub fn len(&self) -> usize {
        self.manifest.total_bytes
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn segment_bytes(&self) -> usize {
        self.manifest.segments.iter().map(|segment| segment.bytes).max().unwrap_or(0)
    }

    pub fn retained_bytes(&self) -> usize {
        self.manifest.segments.capacity().saturating_mul(std::mem::size_of::<Segment>())
    }

    pub fn reader(&self) -> io::Result<File> {
        OpenOptions::new().read(true).open(self.file.path())
    }

    pub fn verify(&self, maximum: usize) -> io::Result<()> {
        let invalid = || io::Error::new(io::ErrorKind::InvalidData, "invalid snapshot manifest or segment");
        if self.manifest.count != self.manifest.segments.len() {
            return Err(invalid());
        }
        let mut reader = self.reader()?;
        let mut buffer = [0u8; 8192];
        let mut total = 0usize;
        for segment in &self.manifest.segments {
            if segment.bytes == 0 || segment.bytes > maximum {
                return Err(invalid());
            }
            total = total.checked_add(segment.bytes).ok_or_else(invalid)?;
            let mut remaining = segment.bytes;
            let mut digest = Sha256::new();
            while remaining != 0 {
                let count = remaining.min(buffer.len());
                reader.read_exact(&mut buffer[..count])?;
                digest.update(&buffer[..count]);
                remaining -= count;
            }
            if <[u8; 32]>::from(digest.finalize()) != segment.digest {
                return Err(invalid());
            }
        }
        if total != self.len() || reader.read(&mut buffer[..1])? != 0 {
            return Err(invalid());
        }
        Ok(())
    }
}

impl PartialEq for SnapshotPayload {
    fn eq(&self, other: &Self) -> bool {
        self.manifest == other.manifest
    }
}

impl Eq for SnapshotPayload {}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
