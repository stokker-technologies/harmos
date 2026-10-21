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
use std::collections::BTreeSet;
use std::time::Duration;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::contracts::{self, Encoder};
use super::{StreamDeclaration, publications};
use crate::catalog::Node;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Descriptors;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DemandKey {
    namespace: &'static str,
    id: &'static str,
}

#[derive(Clone, Copy)]
pub struct Entry {
    pub id: &'static str,
    pub description: &'static str,
    pub interval: Duration,
    pub activation: Activation,
    pub streams: &'static [publications::Entry],
    pub demand_keys: &'static [DemandKey],
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    pub id: String,
    pub description: String,
    pub interval: Duration,
    pub activation: Activation,
    pub streams: Vec<String>,
    pub demand_keys: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub facility: String,
    pub owner: String,
    pub observers: Vec<Declaration>,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Activation {
    #[default]
    Plugin,

    Demand,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Contract(#[from] contracts::Error),

    #[error(transparent)]
    Schema(#[from] crate::schema::graph::Error),

    #[error("the artifact carries no complete {facility} observer inventory for `{owner}`")]
    Missing { facility: String, owner: String },

    #[error("the artifact carries multiple {facility} observer inventories for `{owner}`")]
    Duplicate { facility: String, owner: String },

    #[error("observer `{id}` is invalid: {reason}")]
    Invalid { id: String, reason: &'static str },
}

/* === Traits === */

pub trait Declared {
    const ENTRY: Entry;
}

/* === Implementations === */

impl crate::catalog::Visitor for Descriptors {
    type Entry = Entry;
}

impl<S: Declared> crate::catalog::Member<S> for Descriptors {
    const ENTRY: Entry = S::ENTRY;
}

impl Entry {
    pub fn declaration(&self) -> Declaration {
        Declaration {
            id: self.id.to_owned(),
            description: self.description.to_owned(),
            interval: self.interval,
            activation: self.activation,
            streams: self.streams.iter().map(|entry| entry.id.to_owned()).collect(),
            demand_keys: self.demand_keys.iter().map(DemandKey::name).collect(),
        }
    }
}

impl DemandKey {
    pub const fn new(namespace: &'static str, id: &'static str) -> Self {
        assert!(!namespace.is_empty() && !id.is_empty(), "demand keys require a namespace and identity");
        Self { namespace, id }
    }

    pub fn name(&self) -> String {
        format!("{}:{}", self.namespace, self.id)
    }
}

impl<const N: usize> Encoder<N> {
    const fn key(&mut self, key: DemandKey) {
        // Encode both components with the ordinary JSON escaping, sharing quotes.
        self.quoted(key.namespace);
        self.cursor -= 1;
        self.raw(b":");
        let start = self.cursor;
        self.quoted(key.id);
        if N > 0 {
            let mut index = start;
            while index + 1 < self.cursor {
                self.bytes[index] = self.bytes[index + 1];
                index += 1;
            }
        }
        self.cursor -= 1;
    }
}

/* === Public Functions === */

pub fn select(manifests: Vec<Manifest>, facility: &str, owner: &str, grants: &[StreamDeclaration]) -> Result<Vec<Declaration>, Error> {
    Ok(select_manifest(manifests, facility, owner, grants)?.observers)
}

pub fn select_manifest(manifests: Vec<Manifest>, facility: &str, owner: &str, grants: &[StreamDeclaration]) -> Result<Manifest, Error> {
    let mut matching = manifests.into_iter().filter(|manifest| manifest.facility == facility && manifest.owner == owner);
    let manifest = matching.next().ok_or_else(|| Error::Missing { facility: facility.into(), owner: owner.into() })?;
    if matching.next().is_some() {
        return Err(Error::Duplicate { facility: facility.into(), owner: owner.into() });
    }

    validate(&manifest.observers, grants)?;
    Ok(manifest)
}

pub fn validate(observers: &[Declaration], grants: &[StreamDeclaration]) -> Result<(), Error> {
    let mut ids = BTreeSet::new();
    for observer in observers {
        let invalid = |reason| Error::Invalid { id: observer.id.clone(), reason };
        if observer.id.trim().is_empty() || observer.id.chars().any(char::is_control) || !ids.insert(&observer.id) {
            return Err(invalid("identities must be nonempty, unique, and contain no control characters"));
        }
        if observer.interval.is_zero() {
            return Err(invalid("interval must be positive"));
        }
        let mut keys = BTreeSet::new();
        for stream in &observer.streams {
            if !keys.insert(stream) || !grants.iter().any(|grant| grant.id == *stream) {
                return Err(invalid("outputs must be unique members of the artifact publication grant"));
            }
        }
        for key in &observer.demand_keys {
            if key.trim().is_empty() || key.chars().any(char::is_control) || !keys.insert(key) {
                return Err(invalid("demand keys must be nonempty, unique, and distinct from outputs"));
            }
        }
        if observer.activation == Activation::Demand && keys.is_empty() {
            return Err(invalid("demand activation requires an output or demand key"));
        }
    }
    Ok(())
}

pub const fn encoded_len(facility: &str, owner: &str, entries: &[Entry]) -> usize {
    encoded::<0>(facility, owner, entries).cursor
}

pub const fn encode<const N: usize>(facility: &str, owner: &str, entries: &[Entry]) -> [u8; N] {
    assert!(N == encoded_len(facility, owner, entries));
    encoded::<N>(facility, owner, entries).bytes
}

pub const fn encoded_tree_len(facility: &str, owner: &str, entries: &[Node<Entry>]) -> usize {
    let length = encoded_tree::<0>(facility, owner, entries).cursor;
    assert!(length - crate::PREFIX.len() - 1 <= crate::FRAGMENT_LIMIT);
    length
}

pub const fn encode_tree<const N: usize>(facility: &str, owner: &str, entries: &[Node<Entry>]) -> [u8; N] {
    assert!(N == encoded_tree_len(facility, owner, entries));
    encoded_tree::<N>(facility, owner, entries).bytes
}

/* === Private Functions === */

const fn encoded<const N: usize>(facility: &str, owner: &str, entries: &[Entry]) -> Encoder<N> {
    let mut out = Encoder { bytes: [0; N], cursor: 0 };
    out.raw(b"\0HARMOS:ARTIFACT:V6:{\"kind\":\"observers\",\"facility\":");
    out.quoted(facility);
    out.raw(b",\"owner\":");
    out.quoted(owner);
    out.raw(b",\"observers\":[");
    let mut index = 0;
    while index < entries.len() {
        if index > 0 {
            out.raw(b",");
        }
        entry(&mut out, entries[index]);
        index += 1;
    }
    out.raw(b"]}\0");
    out
}

const fn entry<const N: usize>(out: &mut Encoder<N>, entry: Entry) {
    out.raw(b"{\"id\":");
    out.quoted(entry.id);
    out.raw(b",\"description\":");
    out.quoted(entry.description);
    out.raw(b",\"interval\":{\"secs\":");
    out.number(entry.interval.as_secs());
    out.raw(b",\"nanos\":");
    out.number(entry.interval.subsec_nanos() as u64);
    out.raw(b"},\"activation\":");
    out.quoted(match entry.activation {
        Activation::Plugin => "plugin",
        Activation::Demand => "demand",
    });
    out.raw(b",\"streams\":[");
    let mut item = 0;
    while item < entry.streams.len() {
        if item > 0 {
            out.raw(b",");
        }
        out.quoted(entry.streams[item].id);
        item += 1;
    }
    out.raw(b"],\"demand_keys\":[");
    item = 0;
    while item < entry.demand_keys.len() {
        if item > 0 {
            out.raw(b",");
        }
        out.key(entry.demand_keys[item]);
        item += 1;
    }
    out.raw(b"]}");
}

const fn encoded_tree<const N: usize>(facility: &str, owner: &str, entries: &[Node<Entry>]) -> Encoder<N> {
    let mut out = Encoder { bytes: [0; N], cursor: 0 };
    out.raw(b"\0HARMOS:ARTIFACT:V6:{\"kind\":\"observers\",\"facility\":");
    out.quoted(facility);
    out.raw(b",\"owner\":");
    out.quoted(owner);
    out.raw(b",\"observers\":[");
    let mut first = true;
    tree(&mut out, entries, &mut first, 0);
    out.raw(b"]");
    out.raw(b"}\0");
    out
}

const fn tree<const N: usize>(out: &mut Encoder<N>, entries: &[Node<Entry>], first: &mut bool, depth: usize) {
    assert!(depth <= 32);
    let mut index = 0;
    while index < entries.len() {
        match &entries[index] {
            Node::Leaf(value) => {
                if !*first {
                    out.raw(b",");
                }
                *first = false;
                entry(out, *value);
            }
            Node::Nested(entries) => tree(out, entries, first, depth + 1),
        }
        index += 1;
    }
}

/* === Tests === */
