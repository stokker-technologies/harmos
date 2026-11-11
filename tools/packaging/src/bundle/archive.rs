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
use std::collections::BTreeMap;
use std::io::{self, Write};
// Third Party Imports
// Local Imports
use super::{Error, Facility, Limits, Manifest, invalid};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Bounded {
    bytes: Vec<u8>,
    limit: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit - self.bytes.len() {
            return Err(io::Error::other("plugin byte limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/* === Public Functions === */

pub(super) fn path(path: &str) -> Result<(), Error> {
    // A deliberately small USTAR/Windows/Unix intersection: no extensions or
    // alternate separators are needed to interpret these names.
    if path.is_empty() || path.len() > 100 {
        return Err(invalid("file path must contain 1 to 100 ASCII bytes"));
    }
    for component in path.split('/') {
        let upper = component.to_ascii_uppercase();
        let stem = upper.split('.').next().unwrap_or_default();
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.ends_with('.')
            || !component.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            || matches!(
                stem,
                "CON"
                    | "PRN"
                    | "AUX"
                    | "NUL"
                    | "COM1"
                    | "COM2"
                    | "COM3"
                    | "COM4"
                    | "COM5"
                    | "COM6"
                    | "COM7"
                    | "COM8"
                    | "COM9"
                    | "LPT1"
                    | "LPT2"
                    | "LPT3"
                    | "LPT4"
                    | "LPT5"
                    | "LPT6"
                    | "LPT7"
                    | "LPT8"
                    | "LPT9"
            )
        {
            return Err(invalid("nonportable file path"));
        }
    }
    if path.split('/').next().is_some_and(|component| component.eq_ignore_ascii_case("manifest.json")) {
        return Err(invalid("manifest.json is reserved"));
    }
    Ok(())
}

pub(super) fn encode(manifest: &Manifest, contents: &BTreeMap<String, &[u8]>, limits: Limits) -> Result<Vec<u8>, Error> {
    let mut json = Bounded {
        bytes: Vec::new(),
        limit: limits.manifest_bytes,
    };
    serde_json::to_writer(&mut json, manifest)?;
    let output = Bounded {
        bytes: Vec::new(),
        limit: limits.archive_bytes,
    };
    let mut builder = tar::Builder::new(output);
    append(&mut builder, "manifest.json", &json.bytes, false)?;
    for file in &manifest.files {
        append(
            &mut builder,
            &file.path,
            contents[&file.path],
            file.executable.as_ref().is_some_and(|value| value.facility == Facility::Sidecar),
        )?;
    }
    Ok(builder.into_inner()?.bytes)
}

/* === Private Functions === */

fn append(builder: &mut tar::Builder<Bounded>, path: &str, bytes: &[u8], executable: bool) -> Result<(), Error> {
    let mut header = tar::Header::new_ustar();
    header.set_path(path)?;
    header.set_entry_type(tar::EntryType::Regular);
    header.set_size(bytes.len() as u64);
    header.set_mode(if executable { 0o755 } else { 0o644 });
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_cksum();
    builder.append(&header, bytes)?;
    Ok(())
}

/* === Tests === */
