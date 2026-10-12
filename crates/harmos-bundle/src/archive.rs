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
use std::ops::Range;
// Third Party Imports
use sha2::{Digest, Sha256};
// Local Imports
use super::{Error, Facility, Limits, Manifest, invalid};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Contents = BTreeMap<String, Range<usize>>;

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
pub(super) fn digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let hash = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(hash.len() * 2);
    for byte in hash {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

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
            || !component
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
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
    if path
        .split('/')
        .next()
        .is_some_and(|component| component.eq_ignore_ascii_case("manifest.json"))
    {
        return Err(invalid("manifest.json is reserved"));
    }
    Ok(())
}

pub(super) fn encode(
    manifest: &Manifest,
    contents: &BTreeMap<String, &[u8]>,
    limits: Limits,
) -> Result<Vec<u8>, Error> {
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
            file.executable
                .as_ref()
                .is_some_and(|value| value.facility == Facility::Sidecar),
        )?;
    }
    Ok(builder.into_inner()?.bytes)
}

pub(super) fn decode(bytes: &[u8], limits: Limits) -> Result<(Manifest, Contents), Error> {
    if bytes.len() > limits.archive_bytes {
        return Err(Error::Limit("archive bytes"));
    }
    let mut archive = tar::Archive::new(bytes);
    let mut entries = archive.entries()?.raw(true);
    let entry = entries
        .next()
        .ok_or_else(|| invalid("missing manifest"))??;
    if entry.path_bytes().as_ref() != b"manifest.json" {
        return Err(invalid("manifest must be first"));
    }
    let range = entry_range(&entry, bytes.len(), limits.manifest_bytes)?;
    super::metadata::preflight(&bytes[range.clone()], limits)?;
    let manifest: Manifest = serde_json::from_slice(&bytes[range])?;
    super::validate(&manifest, limits)?;
    let mut contents = BTreeMap::new();
    let mut total = 0usize;
    for entry in entries {
        let entry = entry?;
        if contents.len() >= limits.files {
            return Err(Error::Limit("file count"));
        }
        let path_bytes = entry.path_bytes();
        let name = std::str::from_utf8(&path_bytes).map_err(|_| invalid("non-UTF8 file path"))?;
        path(name)?;
        let range = entry_range(&entry, bytes.len(), limits.file_bytes)?;
        total = total
            .checked_add(range.len())
            .ok_or(Error::Limit("total file bytes"))?;
        if total > limits.total_file_bytes {
            return Err(Error::Limit("total file bytes"));
        }
        if contents.insert(name.to_owned(), range).is_some() {
            return Err(invalid("duplicate archive file"));
        }
    }
    if contents.len() != manifest.files.len() {
        return Err(invalid("archive file set differs from manifest"));
    }
    for file in &manifest.files {
        let range = contents
            .get(&file.path)
            .ok_or_else(|| invalid("missing declared file"))?;
        if range.len() as u64 != file.size || digest(&bytes[range.clone()]) != file.sha256 {
            return Err(invalid("file size or digest mismatch"));
        }
    }
    let data = contents
        .iter()
        .map(|(path, range)| (path.clone(), &bytes[range.clone()]))
        .collect();
    // Exact regeneration rejects alternate headers, unknown/duplicate JSON
    // keys, extensions, nonzero padding, truncation and trailing archives. The
    // tar reader alone accepts some of these and stops at the first zero block.
    if encode(&manifest, &data, limits)? != bytes {
        return Err(invalid("noncanonical archive"));
    }
    Ok((manifest, contents))
}

pub(super) fn permissions(file: &std::fs::File, executable: bool) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(if executable {
            0o755
        } else {
            0o644
        }))?;
    }
    #[cfg(not(unix))]
    let _ = (file, executable);
    Ok(())
}

pub(super) fn staging(parent: &std::path::Path) -> io::Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix("harmos-plugin-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    builder.tempdir_in(parent)
}

/* === Private Functions === */
fn append(
    builder: &mut tar::Builder<Bounded>,
    path: &str,
    bytes: &[u8],
    executable: bool,
) -> Result<(), Error> {
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

fn entry_range(
    entry: &tar::Entry<'_, &[u8]>,
    archive_len: usize,
    limit: usize,
) -> Result<Range<usize>, Error> {
    if entry.header().as_ustar().is_none() || entry.header().entry_type() != tar::EntryType::Regular
    {
        return Err(invalid("only regular USTAR files are allowed"));
    }
    let size = usize::try_from(entry.size()).map_err(|_| Error::Limit("file bytes"))?;
    if size > limit {
        return Err(Error::Limit("entry bytes"));
    }
    let start =
        usize::try_from(entry.raw_file_position()).map_err(|_| Error::Limit("archive bytes"))?;
    let end = start
        .checked_add(size)
        .filter(|end| *end <= archive_len)
        .ok_or_else(|| invalid("truncated file"))?;
    Ok(start..end)
}

/* === Tests === */
