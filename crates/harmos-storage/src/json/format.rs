/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Bounded Checksummed JSON Framing And Durable Metadata Replacement.

use std::{
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
};

use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use crate::{BackendFailure, Error, Result};

const PREFIX: &[u8] = b"{\"value\":";
const TRAILER: &[u8] = b",\"sha256\":\"";
const END: &[u8] = b"\"}\n";
const HEX: &[u8; 16] = b"0123456789abcdef";

pub(super) struct Line {
    bytes: Vec<u8>,
    end: usize,
}

struct Sink<'a, W> {
    writer: &'a mut W,
    digest: Sha256,
    remaining: usize,
}

impl std::ops::Deref for Line {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes[PREFIX.len()..self.end]
    }
}

impl<W: Write> Write for Sink<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "JSON entry exceeds configured size"));
        }
        self.writer.write_all(bytes)?;
        self.digest.update(bytes);
        self.remaining -= bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

pub(super) fn write<W: Write>(writer: &mut W, maximum: usize, encode: impl FnOnce(&mut dyn Write) -> Result<()>) -> Result<()> {
    let overhead = PREFIX.len() + TRAILER.len() + 64 + END.len();
    let remaining = maximum.checked_sub(overhead).ok_or_else(|| invalid("entry limit is smaller than its integrity envelope"))?;
    writer.write_all(PREFIX).map_err(io)?;
    let mut sink = Sink { writer, digest: Sha256::new(), remaining };
    encode(&mut sink)?;
    let digest = checksum(sink.digest.finalize().into());
    writer.write_all(TRAILER).map_err(io)?;
    writer.write_all(&digest).map_err(io)?;
    writer.write_all(END).map_err(io)
}

pub(super) fn line(reader: &mut impl BufRead, maximum: usize) -> Result<Option<Line>> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().map_err(io)?;
        if available.is_empty() {
            return if line.is_empty() { Ok(None) } else { Err(corrupt("incomplete JSON entry")) };
        }
        let end = available.iter().position(|byte| *byte == b'\n');
        let count = end.map_or(available.len(), |index| index + 1);
        if count > maximum.saturating_sub(line.len()) {
            return Err(corrupt("JSON entry exceeds configured size"));
        }
        let required = line.len() + count;
        if required > line.capacity() {
            let capacity = required.max(line.capacity().saturating_mul(2)).min(maximum);
            line.try_reserve_exact(capacity - line.len()).map_err(|_| invalid("JSON framing allocation failed"))?;
        }
        line.extend_from_slice(&available[..count]);
        reader.consume(count);
        if end.is_some() {
            break;
        }
    }
    let overhead = PREFIX.len() + TRAILER.len() + 64 + END.len();
    if line.len() < overhead || !line.starts_with(PREFIX) || !line.ends_with(END) {
        return Err(corrupt("JSON entry envelope is invalid"));
    }
    let end = line.len() - TRAILER.len() - 64 - END.len();
    if &line[end..end + TRAILER.len()] != TRAILER {
        return Err(corrupt("JSON entry trailer is invalid"));
    }
    let digest = checksum(Sha256::digest(&line[PREFIX.len()..end]).into());
    if digest != line[end + TRAILER.len()..line.len() - END.len()] {
        return Err(corrupt("JSON entry checksum differs"));
    }
    Ok(Some(Line { bytes: line, end }))
}

pub(super) fn encode(value: &impl Serialize, writer: &mut dyn Write) -> Result<()> {
    serde_json::to_writer(writer, value).map_err(json)
}

pub(super) fn load<T: DeserializeOwned>(path: &Path, maximum: usize) -> Result<Option<T>> {
    regular(path)?;
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io(error)),
    };
    let mut reader = BufReader::new(file);
    let bytes = line(&mut reader, maximum)?.ok_or_else(|| corrupt("empty JSON metadata"))?;
    let value = serde_json::from_slice(&bytes).map_err(json)?;
    if !reader.fill_buf().map_err(io)?.is_empty() {
        return Err(corrupt("JSON metadata has trailing entries"));
    }
    Ok(Some(value))
}

pub(super) fn store(path: &Path, value: &impl Serialize, maximum: usize) -> Result<()> {
    regular(path)?;
    let pending = path.with_extension("pending");
    regular(&pending)?;
    let mut writer = BufWriter::new(File::create(&pending).map_err(io)?);
    write(&mut writer, maximum, |writer| encode(value, writer))?;
    writer.flush().map_err(io)?;
    writer.get_ref().sync_all().map_err(io)?;
    drop(writer);
    fs::rename(&pending, path).map_err(io)?;
    sync_directory(path.parent().expect("managed metadata parent"))
}

pub(super) fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path).map_err(io)?.sync_all().map_err(io)?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

pub(super) fn regular(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(corrupt("JSON storage contains a symlink")),
        Ok(metadata) if metadata.is_file() || metadata.is_dir() => Ok(true),
        Ok(_) => Err(corrupt("JSON storage contains a special file")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io(error)),
    }
}

pub(super) fn io(error: std::io::Error) -> Error {
    Error::backend("JSON storage I/O", BackendFailure::Transient, error)
}
pub(super) fn json(error: serde_json::Error) -> Error {
    Error::backend("JSON storage format", BackendFailure::Corrupt, error)
}
pub(super) fn corrupt(reason: &'static str) -> Error {
    Error::backend("JSON storage", BackendFailure::Corrupt, std::io::Error::other(reason))
}
pub(super) fn conflict(reason: &'static str) -> Error {
    Error::backend("JSON storage", BackendFailure::Conflict, std::io::Error::other(reason))
}
pub(super) fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "JSON storage", reason }
}
pub(super) fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 15) as usize] as char);
    }
    output
}

fn checksum(bytes: [u8; 32]) -> [u8; 64] {
    let mut output = [0; 64];
    for (byte, pair) in bytes.into_iter().zip(output.as_chunks_mut::<2>().0.iter_mut()) {
        pair[0] = HEX[(byte >> 4) as usize];
        pair[1] = HEX[(byte & 15) as usize];
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_framing() {
        let digest = Sha256::digest(b"abc");
        let expected = b"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(&checksum(digest.into()), expected);
        assert_eq!(hex(&[0, 15, 16, 127, 128, 255]), "000f107f80ff");

        let mut bytes = Vec::new();
        write(&mut bytes, 256, |writer| encode(&123u64, writer)).unwrap();
        let decoded = line(&mut bytes.as_slice(), 256).unwrap().unwrap();
        assert_eq!(&*decoded, b"123");
        bytes[PREFIX.len()] = b'9';
        assert!(line(&mut bytes.as_slice(), 256).is_err());
    }

    #[test]
    fn fragmented_framing() {
        let mut encoded = Vec::new();
        write(&mut encoded, 4096, |writer| encode(&"x".repeat(1024), writer)).unwrap();
        let mut reader = BufReader::with_capacity(31, encoded.as_slice());
        let decoded = line(&mut reader, 4096).unwrap().unwrap();
        assert_eq!(serde_json::from_slice::<String>(&decoded).unwrap(), "x".repeat(1024));
    }
}
