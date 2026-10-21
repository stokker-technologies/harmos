/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Checksummed JSON Lines For State Persistence

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::io::{BufRead, Write};
// Third Party Imports
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
// Local Imports
use crate::{BackendFailure, Error, Result};

/* === Constants === */

const PREFIX: &[u8] = b"{\"value\":";
const TRAILER: &[u8] = b",\"sha256\":\"";
const END: &[u8] = b"\"}\n";
// Envelope Bytes Around One Value, Newline Included.
pub(super) const OVERHEAD: usize = PREFIX.len() + TRAILER.len() + 64 + END.len();
/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

pub(super) enum Read<T> {
    End,
    Incomplete,
    Value(T),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn encode<T: Serialize>(value: &T, maximum: usize) -> Result<Vec<u8>> {
    let json = serde_json::to_vec(value).map_err(json)?;
    let length = json.len().checked_add(OVERHEAD).ok_or_else(|| invalid("state size overflow"))?;
    if length > maximum {
        return Err(invalid("state record exceeds configured storage size"));
    }

    let digest = Sha256::digest(&json);
    let mut line = Vec::with_capacity(length);
    line.extend_from_slice(PREFIX);
    line.extend_from_slice(&json);
    line.extend_from_slice(TRAILER);
    for byte in digest {
        write!(line, "{byte:02x}").map_err(io)?;
    }
    line.extend_from_slice(END);
    Ok(line)
}

pub(super) fn write<T: Serialize>(writer: &mut impl Write, value: &T, maximum: usize) -> Result<()> {
    let line = encode(value, maximum)?;
    writer.write_all(&line).map_err(io)
}

pub(super) fn read<T: DeserializeOwned>(reader: &mut impl BufRead, maximum: usize) -> Result<Read<T>> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().map_err(io)?;
        if available.is_empty() {
            return Ok(if line.is_empty() { Read::End } else { Read::Incomplete });
        }
        let complete = available.iter().position(|byte| *byte == b'\n');
        let count = complete.map_or(available.len(), |index| index + 1);
        if count > maximum.saturating_sub(line.len()) {
            return Err(invalid("state record exceeds configured storage size"));
        }
        line.extend_from_slice(&available[..count]);
        reader.consume(count);
        if complete.is_some() {
            break;
        }
    }
    if line.len() < OVERHEAD || !line.starts_with(PREFIX) || !line.ends_with(END) {
        return Err(corrupt("state record has no integrity envelope"));
    }
    let payload_end = line.len() - TRAILER.len() - 64 - END.len();
    if &line[payload_end..payload_end + TRAILER.len()] != TRAILER {
        return Err(corrupt("state record has no integrity trailer"));
    }
    let digest = Sha256::digest(&line[PREFIX.len()..payload_end]);
    let actual = &line[payload_end + TRAILER.len()..line.len() - END.len()];
    for (index, byte) in digest.iter().enumerate() {
        let high = hex(actual[index * 2]);
        let low = hex(actual[index * 2 + 1]);
        if high.zip(low).map(|(high, low)| high * 16 + low) != Some(*byte) {
            return Err(corrupt("state record checksum differs"));
        }
    }
    serde_json::from_slice(&line[PREFIX.len()..payload_end])
        .map(Read::Value)
        .map_err(|error| Error::backend("decode state record", BackendFailure::Corrupt, error))
}

/* === Private Functions === */

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

pub(super) fn io(error: std::io::Error) -> Error {
    Error::backend("state file I/O", BackendFailure::Transient, error)
}

fn json(error: serde_json::Error) -> Error {
    Error::backend("encode state record", BackendFailure::Permanent, error)
}

pub(super) fn corrupt(reason: &'static str) -> Error {
    Error::backend("read state file", BackendFailure::Corrupt, std::io::Error::new(std::io::ErrorKind::InvalidData, reason))
}

pub(super) fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "state file", reason }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_corruption_refused() {
        let mut bytes = Vec::new();
        write(&mut bytes, &vec![1u8, 2, 3], 128).unwrap();
        let mut input = bytes.as_slice();
        assert!(matches!(read::<Vec<u8>>(&mut input, 128).unwrap(), Read::Value(value) if value == [1, 2, 3]));
        assert!(matches!(read::<Vec<u8>>(&mut input, 128).unwrap(), Read::End));
        let mut truncated = &bytes[..bytes.len() - 1];
        assert!(matches!(read::<Vec<u8>>(&mut truncated, 128).unwrap(), Read::Incomplete));
        bytes[1] = b'9';
        assert!(read::<Vec<u8>>(&mut bytes.as_slice(), 128).is_err());
    }

    #[test]
    fn record_size_validated() {
        let line = encode(&"abc", usize::MAX).unwrap();
        assert_eq!(encode(&"abc", line.len()).unwrap(), line);

        let mut bytes = Vec::new();
        assert!(write(&mut bytes, &"abc", line.len() - 1).is_err());
        assert!(bytes.is_empty());
        assert!(read::<String>(&mut line.as_slice(), line.len() - 1).is_err());
    }
}
