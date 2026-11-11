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
use std::io;
use std::ops::Range;
// Third Party Imports
// Local Imports
use super::digest;
use super::{Error, Facility, Limits, Manifest, invalid};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Contents = BTreeMap<String, Range<usize>>;

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

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

pub(super) fn decode(bytes: &[u8], limits: Limits) -> Result<(Manifest, Contents), Error> {
    if bytes.len() > limits.archive_bytes {
        return Err(Error::Limit("archive bytes"));
    }
    let mut archive = tar::Archive::new(bytes);
    let mut entries = archive.entries()?.raw(true);
    let entry = entries.next().ok_or_else(|| invalid("missing manifest"))??;
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
        total = total.checked_add(range.len()).ok_or(Error::Limit("total file bytes"))?;
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
        let range = contents.get(&file.path).ok_or_else(|| invalid("missing declared file"))?;
        if range.len() as u64 != file.size || digest(&bytes[range.clone()]) != file.sha256 {
            return Err(invalid("file size or digest mismatch"));
        }
    }
    // Validate Canonical Metadata, Header Order, Padding and the Exact Trailer.
    let json = serde_json::to_vec(&manifest)?;
    let mut offset = 0usize;
    canonical_entry(bytes, &mut offset, "manifest.json", &json, false)?;
    for file in &manifest.files {
        let content = &bytes[contents[&file.path].clone()];
        let executable = file.executable.as_ref().is_some_and(|value| value.facility == Facility::Sidecar);
        canonical_entry(bytes, &mut offset, &file.path, content, executable)?;
    }
    if bytes.len() != offset + 1024 || bytes[offset..].iter().any(|byte| *byte != 0) {
        return Err(invalid("noncanonical archive trailer"));
    }
    Ok((manifest, contents))
}

pub(super) fn permissions(file: &std::fs::File, executable: bool) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(if executable { 0o755 } else { 0o644 }))?;
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

fn canonical_entry(bytes: &[u8], offset: &mut usize, path: &str, content: &[u8], executable: bool) -> Result<(), Error> {
    let mut header = tar::Header::new_ustar();
    header.set_path(path)?;
    header.set_entry_type(tar::EntryType::Regular);
    header.set_size(content.len() as u64);
    header.set_mode(if executable { 0o755 } else { 0o644 });
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_cksum();
    let data = *offset + 512;
    let end = data.checked_add(content.len()).ok_or(Error::Limit("archive bytes"))?;
    let padded = end.checked_add(511).ok_or(Error::Limit("archive bytes"))? / 512 * 512;
    if bytes.get(*offset..data) != Some(header.as_bytes().as_slice())
        || bytes.get(data..end) != Some(content)
        || bytes.get(end..padded).is_none_or(|padding| padding.iter().any(|byte| *byte != 0))
    {
        return Err(invalid("noncanonical archive entry"));
    }
    *offset = padded;
    Ok(())
}

fn entry_range(entry: &tar::Entry<'_, &[u8]>, archive_len: usize, limit: usize) -> Result<Range<usize>, Error> {
    if entry.header().as_ustar().is_none() || entry.header().entry_type() != tar::EntryType::Regular {
        return Err(invalid("only regular USTAR files are allowed"));
    }
    let size = usize::try_from(entry.size()).map_err(|_| Error::Limit("file bytes"))?;
    if size > limit {
        return Err(Error::Limit("entry bytes"));
    }
    let start = usize::try_from(entry.raw_file_position()).map_err(|_| Error::Limit("archive bytes"))?;
    let end = start.checked_add(size).filter(|end| *end <= archive_len).ok_or_else(|| invalid("truncated file"))?;
    Ok(start..end)
}

/* === Tests === */
