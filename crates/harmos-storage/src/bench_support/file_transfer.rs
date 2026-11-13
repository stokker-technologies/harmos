/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Reusable file-transfer measurements for consumer-supplied client and plugin carriers.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{num::NonZeroUsize, time::Instant};

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use crate::{FileProducer, FileReference, Result, StorageFuture};

/* === Constants === */

pub const FILE_BYTES: usize = 30 * 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Range {
    pub data: Vec<u8>,
    pub total_length: u64,
    pub sha256: [u8; 32],
}

#[derive(Debug, serde::Serialize)]
pub struct Measurement {
    pub carrier: String,
    pub transfer: &'static str,
    pub direction: &'static str,
    pub bytes: usize,
    pub calls: usize,
    pub chunk_bytes: usize,
    pub seconds: f64,
    pub bytes_per_second: f64,
}

/* === Enums === */

/* === Traits === */

/// Supply real transport calls, using an isolated file identity for each upload.
/// Setup and
/// digest comparisons are outside the timed region; download hashing is inside; resource publication is inside.
pub trait Carrier {
    fn name(&self) -> &str;
    fn create(&self, length: u64, sha256: [u8; 32]) -> StorageFuture<'_, FileProducer>;
    fn append<'a>(&'a self, producer: &'a FileProducer, offset: u64, data: &'a [u8]) -> StorageFuture<'a, FileProducer>;
    fn publish<'a>(&'a self, producer: &'a FileProducer) -> StorageFuture<'a, FileReference>;
    fn get<'a>(&'a self, file: &'a FileReference, offset: u64, maximum: NonZeroUsize) -> StorageFuture<'a, Range>;
}

/* === Implementations === */

/* === Public Functions === */

/// Measure Unary Sessions and Ranges with the Consumer's Transfer Chunk Setting.
pub async fn run(carrier: &impl Carrier, chunk_bytes: NonZeroUsize) -> Result<[Measurement; 2]> {
    let data: Vec<u8> = (0..FILE_BYTES).map(|index| (index % 251) as u8).collect();
    let digest: [u8; 32] = Sha256::digest(&data).into();
    let started = Instant::now();
    let mut producer = carrier.create(FILE_BYTES as u64, digest).await?;
    for (index, chunk) in data.chunks(chunk_bytes.get()).enumerate() {
        producer = carrier.append(&producer, (index * chunk_bytes.get()) as u64, chunk).await?;
    }
    let file = carrier.publish(&producer).await?;
    let upload = measurement(carrier, "unary", "upload", 2 + FILE_BYTES.div_ceil(chunk_bytes.get()), chunk_bytes, started);
    assert_eq!(file.length, FILE_BYTES as u64);
    assert_eq!(file.sha256, digest);

    let started = Instant::now();
    let mut length = 0;
    let mut hash = Sha256::new();
    let mut calls = 0;
    while length < FILE_BYTES {
        let range = carrier.get(&file, length as u64, chunk_bytes).await?;
        assert_eq!(range.total_length, FILE_BYTES as u64);
        assert_eq!(range.sha256, digest);
        assert!(!range.data.is_empty() && range.data.len() <= chunk_bytes.get());
        length += range.data.len();
        hash.update(&range.data);
        calls += 1;
    }
    let download = measurement(carrier, "unary", "download", calls, chunk_bytes, started);
    assert_eq!(length, FILE_BYTES);
    assert_eq!(<[u8; 32]>::from(hash.finalize()), digest);
    Ok([upload, download])
}

/* === Private Functions === */

fn measurement(
    carrier: &impl Carrier, transfer: &'static str, direction: &'static str, calls: usize, chunk_bytes: NonZeroUsize, started: Instant,
) -> Measurement {
    let seconds = started.elapsed().as_secs_f64();
    Measurement {
        carrier: carrier.name().into(),
        transfer,
        direction,
        bytes: FILE_BYTES,
        calls,
        chunk_bytes: chunk_bytes.get(),
        seconds,
        bytes_per_second: FILE_BYTES as f64 / seconds,
    }
}

/* === Tests === */
