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
use std::borrow::Cow;
use std::collections::BTreeSet;
// Third Party Imports
use serde::de::{DeserializeOwned, IgnoredAny};
use serde::{Deserialize, Serialize};
// Local Imports
use super::contracts::{Encoder, valid_identity};
use crate::PREFIX;

/* === Constants === */

const ENTRY_LIMIT: usize = 256;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub namespace: Cow<'static, str>,
    pub payload: Cow<'static, str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Decoded")]
pub struct Manifest {
    pub facility: String,
    pub owner: String,
    pub sections: Vec<Section>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decoded {
    #[serde(default)]
    kind: Option<String>,

    facility: String,
    owner: String,
    sections: Vec<Section>,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("multiple contribution inventories for `{owner}`")]
    Duplicate { owner: String },

    #[error("contribution inventory requires a sidecar facility and a valid owner")]
    Identity,

    #[error("contribution inventory exceeds its section budget")]
    Budget,

    #[error("contribution namespace `{namespace}` is invalid or repeated")]
    Namespace { namespace: String },

    #[error("contribution section `{namespace}` is not JSON: {source}")]
    Payload {
        namespace: String,

        #[source]
        source: serde_json::Error,
    },

    #[error("invalid contribution inventory JSON: {0}")]
    Decode(#[from] serde_json::Error),
}

/* === Traits === */

/* === Implementations === */

impl Section {
    pub const fn new(namespace: &'static str, payload: &'static str) -> Self {
        Self {
            namespace: Cow::Borrowed(namespace),
            payload: Cow::Borrowed(payload),
        }
    }

    pub fn decode<T: DeserializeOwned>(&self) -> serde_json::Result<T> {
        serde_json::from_str(&self.payload)
    }

    const fn borrowed(&self) -> (&'static str, &'static str) {
        match (&self.namespace, &self.payload) {
            (Cow::Borrowed(namespace), Cow::Borrowed(payload)) => (namespace, payload),
            _ => panic!("const contribution sections must be borrowed"),
        }
    }
}

impl TryFrom<Decoded> for Manifest {
    type Error = Error;

    fn try_from(value: Decoded) -> Result<Self, Error> {
        if value.kind.as_deref().is_some_and(|kind| kind != "contributions") {
            return Err(Error::Identity);
        }
        let manifest = Self {
            facility: value.facility,
            owner: value.owner,
            sections: value.sections,
        };
        validate(&manifest)?;
        Ok(manifest)
    }
}

/* === Public Functions === */

pub fn select(manifests: Vec<Manifest>, facility: &str, owner: &str) -> Result<Vec<Section>, Error> {
    let mut matching = manifests.into_iter().filter(|manifest| manifest.facility == facility && manifest.owner == owner);
    let Some(manifest) = matching.next() else {
        return Ok(Vec::new());
    };
    if matching.next().is_some() {
        return Err(Error::Duplicate { owner: owner.to_owned() });
    }
    Ok(manifest.sections)
}

pub fn validate(manifest: &Manifest) -> Result<(), Error> {
    if !valid_identity(&manifest.facility, &manifest.owner) {
        return Err(Error::Identity);
    }
    if manifest.sections.len() > ENTRY_LIMIT {
        return Err(Error::Budget);
    }
    let mut namespaces = BTreeSet::new();
    for section in &manifest.sections {
        if !crate::artifact_subset(&section.namespace) || !namespaces.insert(&section.namespace) {
            return Err(Error::Namespace { namespace: section.namespace.to_string() });
        }
        serde_json::from_str::<IgnoredAny>(&section.payload).map_err(|source| Error::Payload {
            namespace: section.namespace.to_string(),
            source,
        })?;
    }
    Ok(())
}

pub const fn namespaced(sections: &[Section], namespaces: &[&str]) -> bool {
    if sections.len() != namespaces.len() {
        return false;
    }
    let mut index = 0;
    while index < sections.len() {
        let section = sections[index].borrowed().0.as_bytes();
        let expected = namespaces[index].as_bytes();
        if section.len() != expected.len() {
            return false;
        }
        let mut byte = 0;
        while byte < section.len() {
            if section[byte] != expected[byte] {
                return false;
            }
            byte += 1;
        }
        index += 1;
    }
    true
}

pub const fn encoded_len(facility: &str, owner: &str, sections: &[Section]) -> usize {
    let size = encoded::<0>(facility, owner, sections).cursor;
    assert!(size - PREFIX.len() - 1 <= crate::FRAGMENT_LIMIT);
    size
}

pub const fn encode<const N: usize>(facility: &str, owner: &str, sections: &[Section]) -> [u8; N] {
    assert!(N == encoded_len(facility, owner, sections));
    encoded::<N>(facility, owner, sections).bytes
}

/* === Private Functions === */

const fn encoded<const N: usize>(facility: &str, owner: &str, sections: &[Section]) -> Encoder<N> {
    assert!(valid_identity(facility, owner));
    assert!(sections.len() <= ENTRY_LIMIT, "contribution sections exceed their budget");
    let mut out = Encoder { bytes: [0; N], cursor: 0 };
    out.raw(PREFIX);
    out.raw(b"{\"kind\":\"contributions\",\"facility\":");
    out.quoted(facility);
    out.raw(b",\"owner\":");
    out.quoted(owner);
    out.raw(b",\"sections\":[");
    let mut index = 0;
    while index < sections.len() {
        if index > 0 {
            out.raw(b",");
        }
        let (namespace, payload) = sections[index].borrowed();
        assert!(crate::artifact_subset(namespace), "contribution namespaces use the portable artifact subset");
        out.raw(b"{\"namespace\":");
        out.quoted(namespace);
        out.raw(b",\"payload\":");
        out.quoted(payload);
        out.raw(b"}");
        index += 1;
    }
    out.raw(b"]}\0");
    out
}

/* === Tests === */
