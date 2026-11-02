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
pub const PREFIX: &[u8] = b"\0HARMOS:ARTIFACT:V1:";

pub const FRAGMENT_LIMIT: usize = 64 * 1024;

pub const KINDS: [&str; 17] = [
    "capabilities",
    "contracts",
    "contributions",
    "guest",
    "listeners",
    "observers",
    "operations",
    "plugin",
    "publications",
    "requires",
    "restart",
    "route",
    "routes",
    "schema",
    "sidecar",
    "stream",
    "subscribe",
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
    #[error("schema preparation {budget} budget exceeded: used {used}, allowed {allowed}")]
    Budget { budget: &'static str, used: usize, allowed: usize },

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

pub fn fragments<F: DeserializeOwned>(artifact: &[u8], owned: &[&str]) -> Result<Vec<F>, ScanError> {
    fragments_with_settings(artifact, owned, &mut crate::config::SchemaSettings::default())
}

pub(crate) fn fragments_with_settings<F: DeserializeOwned>(
    artifact: &[u8], owned: &[&str], settings: &mut crate::config::SchemaSettings,
) -> Result<Vec<F>, ScanError> {
    // Keep Complete Search Markers Out Of Reader Binaries Themselves.
    let family = &FAMILY[..FAMILY.len() - 1];
    let mut prefix = [0; PREFIX.len()];
    prefix[..family.len()].copy_from_slice(std::hint::black_box(family));
    // A Terminator Separates The Family Literal From Adjacent Version Literals.
    prefix[family.len()] = std::hint::black_box(b'V');
    prefix[family.len() + 1] = std::hint::black_box(b'1');
    prefix[family.len() + 2] = std::hint::black_box(b':');
    let current = memmem::Finder::new(&prefix);
    let mut found = Vec::new();
    let mut cursor = 0;
    let mut remaining_bytes = settings.preparation_bytes.get();
    let mut remaining_work = settings.preparation_work.get();

    while let Some(relative) = current.find(&artifact[cursor..]) {
        let start = cursor + relative + PREFIX.len();
        let bounded = &artifact[start..artifact.len().min(start + FRAGMENT_LIMIT + 1)];
        let Some(length) = memchr(0, bounded) else {
            return Err(ScanError::Oversized);
        };
        let encoded = &bounded[..length];
        cursor = start + length + 1;

        let work = (settings.preparation_work.get() - remaining_work).saturating_add(encoded.len());
        remaining_work = remaining_work.checked_sub(encoded.len()).filter(|remaining| *remaining > 0).ok_or(ScanError::Budget {
            budget: "work",
            used: work,
            allowed: settings.preparation_work.get(),
        })?;
        let charge = encoded.len() * 16 + 4 * std::mem::size_of::<F>();
        let bytes = (settings.preparation_bytes.get() - remaining_bytes).saturating_add(charge);
        remaining_bytes = remaining_bytes.checked_sub(charge).filter(|remaining| *remaining > 0).ok_or(ScanError::Budget {
            budget: "memory bytes",
            used: bytes,
            allowed: settings.preparation_bytes.get(),
        })?;
        let Kinded { kind } = serde_json::from_slice(encoded).map_err(|source| ScanError::Malformed { source })?;
        if !KINDS.contains(&kind.as_str()) {
            return Err(ScanError::UnknownKind { kind });
        }
        if !owned.contains(&kind.as_str()) {
            continue;
        }

        found.push(serde_json::from_slice(encoded).map_err(|source| ScanError::Malformed { source })?);
    }

    settings.preparation_bytes = std::num::NonZeroUsize::new(remaining_bytes).expect("positive remaining memory");
    settings.preparation_work = std::num::NonZeroUsize::new(remaining_work).expect("positive remaining work");
    Ok(found)
}

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SchemaSettings;
    use std::num::NonZeroUsize;

    #[test]
    fn preparation_usage_reported() {
        let encoded = br#"{"kind":"schema","name":"fixture","fields":[]}"#;
        let mut artifact = Vec::new();
        for _ in 0..2 {
            artifact.extend_from_slice(PREFIX);
            artifact.extend_from_slice(encoded);
            artifact.push(0);
        }
        let bytes = 2 * (encoded.len() * 16 + 4 * std::mem::size_of::<serde_json::Value>());
        let work = 2 * encoded.len();
        for (budget, used) in [("memory bytes", bytes), ("work", work)] {
            let mut settings = SchemaSettings::default();
            if budget == "work" {
                settings.preparation_work = NonZeroUsize::new(used - 1).unwrap();
            } else {
                settings.preparation_bytes = NonZeroUsize::new(used - 1).unwrap();
            }
            let error = fragments_with_settings::<serde_json::Value>(&artifact, &["schema"], &mut settings).unwrap_err();
            assert!(matches!(error, ScanError::Budget { budget: actual, used: actual_used, allowed }
                if actual == budget && actual_used == used && allowed == used - 1));
        }
        let mut settings = SchemaSettings::default();
        assert_eq!(fragments_with_settings::<serde_json::Value>(&artifact, &["schema"], &mut settings).unwrap().len(), 2);
        assert_eq!(settings.preparation_bytes.get(), SchemaSettings::default().preparation_bytes.get() - bytes);
        assert_eq!(settings.preparation_work.get(), SchemaSettings::default().preparation_work.get() - work);
    }
}
