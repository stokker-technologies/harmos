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
// Third Party Imports
use serde::Deserialize;
// Local Imports
use super::contracts::{self, Encoder};
use crate::catalog::Node;
use crate::config::SchemaSettings;
use crate::listeners::{Definition, Entry, Source, SourceDeclaration};
use crate::operations::Contract;
use crate::schema::declaration::Named;
use crate::schema::graph::Schema;

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

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NamedSource {
    Record {
        id: String,
        version: u32,
        row: String,
    },
    Stream {
        id: String,
        schema_ordinal: u32,
        row: String,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedListener {
    pub id: crate::Identifier,
    pub source: NamedSource,
    pub callback: contracts::NamedContract,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedManifest {
    pub facility: String,
    pub owner: String,
    pub listeners: Vec<NamedListener>,
}

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Schema(#[from] crate::schema::graph::Error),
    #[error(transparent)]
    Contract(#[from] contracts::Error),
    #[error(transparent)]
    Listener(#[from] crate::listeners::Error),
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

impl NamedManifest {
    pub fn resolve(
        self,
        declarations: &[Named],
        settings: &mut SchemaSettings,
    ) -> Result<Manifest, Error> {
        if self.listeners.len() > LIMIT {
            return Err(Error::Inventory);
        }
        let listeners = self
            .listeners
            .into_iter()
            .map(|entry| {
                let source = match entry.source {
                    NamedSource::Record { id, version, row } => Source::Record {
                        id: id.into(),
                        version,
                        row: super::retain_schema(
                            Schema::from_named(&row, declarations, settings)?,
                            settings,
                        )?,
                    },
                    NamedSource::Stream {
                        id,
                        schema_ordinal,
                        row,
                    } => Source::Stream {
                        id: id.into(),
                        schema_ordinal,
                        row: super::retain_schema(
                            Schema::from_named(&row, declarations, settings)?,
                            settings,
                        )?,
                    },
                };
                let callback = entry.callback.resolve(declarations, settings)?;
                Ok(Definition::new(entry.id, source, callback)?)
            })
            .collect::<Result<_, Error>>()?;
        Ok(Manifest {
            facility: self.facility,
            owner: self.owner,
            listeners,
        })
    }
}

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
                let (kind, id, version, row) = match &entry.source {
                    SourceDeclaration::Record { id, version, row } => ("record", id, *version, row),
                    SourceDeclaration::Stream {
                        id,
                        schema_ordinal,
                        row,
                    } => ("stream", id, *schema_ordinal, row),
                };
                out.quoted(kind);
                out.raw(b",\"id\":");
                out.quoted(id);
                match &entry.source {
                    SourceDeclaration::Record { .. } => out.raw(b",\"version\":"),
                    SourceDeclaration::Stream { .. } => out.raw(b",\"schema_ordinal\":"),
                }
                out.number(version as u64);
                out.raw(b",\"row\":");
                out.quoted(row.name);
                out.raw(b"},\"callback\":");
                contracts::wire(out, &entry.callback);
                out.raw(b"}");
            }
        }
        index += 1;
    }
}

/* === Tests === */
