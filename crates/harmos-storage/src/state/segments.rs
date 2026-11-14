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

use std::io::Result as IoResult;

// Standard Library Imports
use std::io::{self, Cursor, Read, Write};
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
    pub position: Position,
    pub count: usize,
    pub total_bytes: usize,
    pub segments: Vec<Segment>,
}

#[derive(Clone, Debug)]
pub struct SnapshotPayload {
    data: Arc<[u8]>,
    manifest: SnapshotManifest,
}

pub struct SnapshotWriter {
    data: Vec<u8>,
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
    pub fn new(limit: NonZeroUsize) -> IoResult<Self> {
        Ok(Self {
            data: Vec::new(),
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

    pub fn finish(mut self, position: Position) -> IoResult<SnapshotPayload> {
        self.flush()?;
        self.finish_segment();
        Ok(SnapshotPayload {
            data: self.data.into(),
            manifest: SnapshotManifest {
                position,
                count: self.segments.len(),
                total_bytes: self.total_bytes,
                segments: self.segments,
            },
        })
    }
}

impl Write for SnapshotWriter {
    fn write(&mut self, mut bytes: &[u8]) -> IoResult<usize> {
        let length = bytes.len();
        let total = self.total_bytes.checked_add(length).ok_or_else(|| io::Error::other("snapshot size overflow"))?;
        self.data.try_reserve(length).map_err(|_| io::Error::other("write snapshot: capacity exhausted"))?;
        while !bytes.is_empty() {
            let count = bytes.len().min(self.limit - self.bytes);
            self.data.extend_from_slice(&bytes[..count]);
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

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl SnapshotPayload {
    pub fn from_bytes(bytes: &[u8], limit: NonZeroUsize, position: Position) -> IoResult<Self> {
        let mut writer = SnapshotWriter::new(limit)?;
        writer.write_all(bytes)?;
        writer.finish(position)
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
        self.manifest.segments.capacity().saturating_mul(std::mem::size_of::<Segment>()).saturating_add(self.data.len())
    }

    pub fn reader(&self) -> IoResult<Cursor<Arc<[u8]>>> {
        Ok(Cursor::new(Arc::clone(&self.data)))
    }

    pub fn verify(&self, maximum: usize) -> IoResult<()> {
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
