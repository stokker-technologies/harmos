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
use serde::Deserialize;
// Local Imports
use super::contracts::{self, Encoder};
use crate::catalog::Node;
use crate::listeners::{Definition, Entry, Source};
use crate::operations::Contract;

/* === Constants === */
const LIMIT: usize = 4096;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub facility: String,
    pub owner: String,
    pub listeners: Vec<Definition>,
}

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("multiple listener inventories for {facility} artifact `{owner}`")]
    Duplicate { facility: String, owner: String },
    #[error("listener inventory identity or count is invalid")]
    Inventory,
    #[error("duplicate listener or callback `{id}`")]
    Entry { id: String },
    #[error("listener callback `{id}` is absent or differs from its provided contract")]
    Callback { id: String },
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn select(
    manifests: Vec<Manifest>,
    facility: &str,
    owner: &str,
    provides: &[Contract],
) -> Result<Option<Vec<Definition>>, Error> {
    let mut selected = None;
    for manifest in manifests {
        if manifest.facility != facility || manifest.owner != owner {
            continue;
        }
        if selected.is_some() {
            return Err(Error::Duplicate {
                facility: facility.into(),
                owner: owner.into(),
            });
        }
        if !contracts::valid_identity(&manifest.facility, &manifest.owner)
            || manifest.listeners.len() > LIMIT
        {
            return Err(Error::Inventory);
        }
        validate(&manifest.listeners, provides)?;
        selected = Some(manifest.listeners);
    }
    Ok(selected)
}

pub fn validate(listeners: &[Definition], provides: &[Contract]) -> Result<(), Error> {
    if listeners.len() > LIMIT {
        return Err(Error::Inventory);
    }
    let mut ids = BTreeSet::new();
    let mut callbacks = BTreeSet::new();
    for listener in listeners {
        let id = listener.callback().operation().id.as_str();
        if !ids.insert(listener.id().as_str()) || !callbacks.insert(id) {
            return Err(Error::Entry { id: id.into() });
        }
        if !provides
            .iter()
            .any(|contract| contract == listener.callback())
        {
            return Err(Error::Callback { id: id.into() });
        }
    }
    Ok(())
}

pub const fn encoded_len(facility: &str, owner: &str, entries: &[Node<Entry>]) -> usize {
    encoded::<0>(facility, owner, entries).cursor
}

pub const fn encode<const N: usize>(
    facility: &str,
    owner: &str,
    entries: &[Node<Entry>],
) -> [u8; N] {
    let out = encoded::<N>(facility, owner, entries);
    assert!(
        out.cursor == N,
        "listener inventory length disagrees with encoder"
    );
    out.bytes
}

/* === Private Functions === */
const fn encoded<const N: usize>(
    facility: &str,
    owner: &str,
    entries: &[Node<Entry>],
) -> Encoder<N> {
    assert!(
        contracts::valid_identity(facility, owner),
        "invalid listener inventory identity"
    );
    assert!(
        crate::catalog::count(entries) <= LIMIT,
        "listener inventory exceeds leaf limit"
    );
    let mut out = Encoder {
        bytes: [0; N],
        cursor: 0,
    };
    out.raw(crate::PREFIX);
    out.raw(b"{\"kind\":\"listeners\",\"facility\":");
    out.quoted(facility);
    out.raw(b",\"owner\":");
    out.quoted(owner);
    out.raw(b",\"listeners\":[");
    tree(&mut out, entries, &mut true);
    out.raw(b"]}\0");
    assert!(
        out.cursor <= crate::FRAGMENT_LIMIT,
        "listener inventory exceeds fragment limit"
    );
    out
}

const fn tree<const N: usize>(out: &mut Encoder<N>, entries: &[Node<Entry>], first: &mut bool) {
    let mut index = 0;
    while index < entries.len() {
        match &entries[index] {
            Node::Nested(nested) => tree(out, nested, first),
            Node::Leaf(entry) => {
                if !*first {
                    out.raw(b",");
                }
                *first = false;
                out.raw(b"{\"id\":");
                out.quoted(entry.id);
                out.raw(b",\"source\":{\"kind\":");
                let (kind, id, version, fields) = match &entry.source {
                    Source::Record {
                        id,
                        version,
                        fields,
                    } => ("record", id, *version, fields),
                    Source::Stream {
                        id,
                        schema_ordinal,
                        fields,
                    } => ("stream", id, *schema_ordinal, fields),
                };
                out.quoted(kind);
                out.raw(b",\"id\":");
                let Cow::Borrowed(id) = id else {
                    panic!("listener source identity must be borrowed")
                };
                out.quoted(id);
                match &entry.source {
                    Source::Record { .. } => out.raw(b",\"version\":"),
                    Source::Stream { .. } => out.raw(b",\"schema_ordinal\":"),
                }
                out.number(version as u64);
                out.raw(b",\"fields\":");
                let Cow::Borrowed(fields) = fields else {
                    panic!("listener source fields must be borrowed")
                };
                contracts::fields(out, fields);
                out.raw(b"},\"callback\":");
                contracts::wire(out, &entry.callback);
                out.raw(b"}");
            }
        }
        index += 1;
    }
}

/* === Tests === */
