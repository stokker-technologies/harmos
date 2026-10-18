/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Bounded reads of regular input files

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    io::{self, Read},
    path::Path,
};

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn read(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let maximum = u64::try_from(limit)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "input file limit is too large"))?;
    if !std::fs::metadata(path)?.is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "input must be a regular file"));
    }
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "input must be a regular file"));
    }
    let mut bytes = Vec::new();
    file.take(maximum).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("input file exceeds {limit} bytes")));
    }
    Ok(bytes)
}

/* === Private Functions === */

/* === Tests === */
