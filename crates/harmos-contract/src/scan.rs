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

// Third Party Imports
use memchr::{memchr, memmem};
use serde::Deserialize;
use serde::de::DeserializeOwned;
// Local Imports

/* === Constants === */
const FAMILY: &[u8] = b"\0HARMOS:ARTIFACT:\0";
pub const PREFIX: &[u8] = b"\0HARMOS:ARTIFACT:V5:";

pub const FRAGMENT_LIMIT: usize = 64 * 1024;

pub const KINDS: [&str; 19] = [
    "capabilities",
    "contracts",
    "contributions",
    "egress",
    "guest",
    "invoke",
    "listeners",
    "observers",
    "operations",
    "plugin",
    "publications",
    "requires",
    "restart",
    "route",
    "routes",
    "sidecar",
    "stream",
    "subscribe",
    "survey",
];

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Deserialize)]
struct Kinded {
    kind: String,
}

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("the artifact uses retired Harmos metadata; rebuild it with the current SDK")]
    IncompatiblePayload,
    #[error("a Harmos metadata fragment exceeds its {FRAGMENT_LIMIT}-byte bound")]
    Oversized,
    #[error("a Harmos metadata fragment is invalid: {source}")]
    Malformed {
        #[source]
        source: serde_json::Error,
    },
    #[error("a Harmos metadata fragment declares the unknown kind `{kind}`")]
    UnknownKind { kind: String },
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn fragments<F: DeserializeOwned>(
    artifact: &[u8],
    owned: &[&str],
) -> Result<Vec<F>, ScanError> {
    // Keep Complete Search Markers Out Of Reader Binaries Themselves.
    let family = &FAMILY[..FAMILY.len() - 1];
    let retired = memmem::Finder::new(family);
    for position in retired.find_iter(artifact) {
        let version = &artifact[position + family.len()..];
        if version.starts_with(b"V1:")
            || version.starts_with(b"V2:")
            || version.starts_with(b"V3:")
            || version.starts_with(b"V4:")
        {
            return Err(ScanError::IncompatiblePayload);
        }
    }
    let mut prefix = [0; PREFIX.len()];
    prefix[..family.len()].copy_from_slice(std::hint::black_box(family));
    // A Terminator Separates The Family Literal From Adjacent Version Literals.
    prefix[family.len()] = std::hint::black_box(b'V');
    prefix[family.len() + 1] = std::hint::black_box(b'5');
    prefix[family.len() + 2] = std::hint::black_box(b':');
    let current = memmem::Finder::new(&prefix);
    let mut found = Vec::new();
    let mut cursor = 0;

    while let Some(relative) = current.find(&artifact[cursor..]) {
        let start = cursor + relative + PREFIX.len();
        let bounded = &artifact[start..artifact.len().min(start + FRAGMENT_LIMIT + 1)];
        let Some(length) = memchr(0, bounded) else {
            return Err(ScanError::Oversized);
        };
        let encoded = &bounded[..length];
        cursor = start + length + 1;

        let Kinded { kind } =
            serde_json::from_slice(encoded).map_err(|source| ScanError::Malformed { source })?;
        if !KINDS.contains(&kind.as_str()) {
            return Err(ScanError::UnknownKind { kind });
        }
        if !owned.contains(&kind.as_str()) {
            continue;
        }

        found.push(
            serde_json::from_slice(encoded).map_err(|source| ScanError::Malformed { source })?,
        );
    }

    Ok(found)
}

/* === Private Functions === */

/* === Tests === */
