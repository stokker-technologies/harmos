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
/// Stream methods measure the retained baseline before its removal. Setup and
/// digest comparisons are outside the timed region; download hashing is inside; resource publication is inside.
pub trait Carrier {
    fn name(&self) -> &str;
    fn create(&self, length: u64, sha256: [u8; 32]) -> StorageFuture<'_, FileProducer>;
    fn append<'a>(&'a self, producer: &'a FileProducer, offset: u64, data: &'a [u8]) -> StorageFuture<'a, FileProducer>;
    fn publish<'a>(&'a self, producer: &'a FileProducer) -> StorageFuture<'a, FileReference>;
    fn get<'a>(&'a self, file: &'a FileReference, offset: u64, maximum: NonZeroUsize) -> StorageFuture<'a, Range>;
    fn upload_stream(&self, data: std::sync::Arc<[u8]>, chunk_bytes: NonZeroUsize, sha256: [u8; 32]) -> StorageFuture<'_, FileReference>;
    fn download_stream<'a>(&'a self, file: &'a FileReference) -> StorageFuture<'a, crate::FileHash>;
}

/* === Implementations === */

/* === Public Functions === */

/// Measure the same deterministic large file through either existing streams or
/// unary session actions. The consumer supplies its configured chunk/range bound.
pub async fn run(carrier: &impl Carrier, chunk_bytes: NonZeroUsize, streaming: bool) -> Result<[Measurement; 2]> {
    let data: std::sync::Arc<[u8]> = (0..FILE_BYTES).map(|index| (index % 251) as u8).collect::<Vec<_>>().into();
    let digest: [u8; 32] = Sha256::digest(&data).into();
    let transfer = if streaming { "streaming" } else { "unary" };
    let started = Instant::now();
    let (file, calls) = if streaming {
        (carrier.upload_stream(data.clone(), chunk_bytes, digest).await?, 1)
    } else {
        let mut producer = carrier.create(FILE_BYTES as u64, digest).await?;
        for (index, chunk) in data.chunks(chunk_bytes.get()).enumerate() {
            producer = carrier.append(&producer, (index * chunk_bytes.get()) as u64, chunk).await?;
        }
        let file = carrier.publish(&producer).await?;
        (file, 2 + FILE_BYTES.div_ceil(chunk_bytes.get()))
    };
    let upload = measurement(carrier, transfer, "upload", calls, chunk_bytes, started);
    assert_eq!(file.length, FILE_BYTES as u64);
    assert_eq!(file.sha256, digest);

    let started = Instant::now();
    let (downloaded, calls) = if streaming {
        (carrier.download_stream(&file).await?, 1)
    } else {
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
        (
            crate::FileHash {
                bytes_read: length as u64,
                sha256: hash.finalize().into(),
            },
            calls,
        )
    };
    let download = measurement(carrier, transfer, "download", calls, chunk_bytes, started);
    assert_eq!(downloaded.bytes_read, FILE_BYTES as u64);
    assert_eq!(downloaded.sha256, digest);
    Ok([upload, download])
}

pub async fn streaming_download(carrier: &impl Carrier, chunk_bytes: NonZeroUsize) -> Result<Measurement> {
    let data: Vec<u8> = (0..FILE_BYTES).map(|index| (index % 251) as u8).collect();
    let digest: [u8; 32] = Sha256::digest(&data).into();
    let mut producer = carrier.create(FILE_BYTES as u64, digest).await?;
    for (index, chunk) in data.chunks(chunk_bytes.get()).enumerate() {
        producer = carrier.append(&producer, (index * chunk_bytes.get()) as u64, chunk).await?;
    }
    let file = carrier.publish(&producer).await?;
    let started = Instant::now();
    let hash = carrier.download_stream(&file).await?;
    let measurement = measurement(carrier, "streaming", "download", 1, chunk_bytes, started);
    assert_eq!(hash.bytes_read, FILE_BYTES as u64);
    assert_eq!(hash.sha256, digest);
    Ok(measurement)
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
