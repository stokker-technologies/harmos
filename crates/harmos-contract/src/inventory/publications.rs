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
// Third Party Imports
use serde::Deserialize;
// Local Imports
use super::{StreamDeclaration, digits, quoted, quoted_len, write};
use crate::config::SchemaSettings;
use crate::schema::declaration::{Definition, Named};
use crate::schema::graph::Schema;

/* === Constants === */
const HEAD: &[u8] = b"\0HARMOS:ARTIFACT:V6:{\"kind\":\"publications\",\"facility\":";
const OWNER: &[u8] = b",\"owner\":";
const STREAMS: &[u8] = b",\"streams\":[";
const ID: &[u8] = b"{\"id\":";
const SCHEMA: &[u8] = b",\"schema\":";
const WINDOW: &[u8] = b",\"window\":";
const METADATA: &[u8] = b",\"metadata\":";
const ROW_METADATA: &[u8] = b",\"row_metadata\":";
const ROW: &[u8] = b",\"row\":";
const TAIL: &[u8] = b"]}\0";
const METADATA_ID: &[u8] = b"{\"identifier\":";
const METADATA_LABEL: &[u8] = b",\"label\":";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Copy)]
pub struct Entry {
    pub id: &'static str,
    pub metadata_json: &'static str,
    pub row_metadata_json: &'static str,
    pub schema: u32,
    pub window: usize,
    pub row: Definition,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub facility: String,
    pub owner: String,
    pub streams: Vec<StreamDeclaration>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedStream {
    pub metadata: crate::Metadata,
    #[serde(default)]
    pub row_metadata: Vec<crate::StreamField>,
    pub id: String,
    pub schema: u32,
    pub window: usize,
    pub row: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedManifest {
    pub facility: String,
    pub owner: String,
    pub streams: Vec<NamedStream>,
}

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("publication inventory differs from authored streams")]
    Mismatch,
    #[error("the artifact carries no complete {facility} publication inventory for `{owner}`")]
    Missing { facility: String, owner: String },
    #[error(
        "the artifact carries multiple complete {facility} publication inventories for `{owner}`"
    )]
    Duplicate { facility: String, owner: String },
    #[error("`{id}` is not a portable Harmos stream id")]
    InvalidId { id: String },
    #[error("the stream `{id}` has a zero schema ordinal")]
    ZeroSchema { id: String },
    #[error("the stream `{id}` has a zero window")]
    ZeroWindow { id: String },
    #[error(
        "the stream `{id}` is declared more than once in its publication inventory; each stream must appear exactly once in the artifact publication grant"
    )]
    DuplicateStream { id: String },
    #[error("stream `{id}` has invalid presentation metadata: {reason}")]
    Metadata { id: String, reason: String },
    #[error("stream `{id}` has an invalid field schema: {source}")]
    Fields {
        id: String,
        #[source]
        source: crate::schema::Error,
    },
}

/* === Traits === */
pub trait StreamSpec {
    type Row;
    const ID: &'static str;
    const VERSION: u32;
    const WINDOW: usize;
    const METADATA_JSON: &'static str = "";
    const ROW_METADATA_JSON: &'static str = "[]";

    fn metadata() -> crate::Metadata {
        if Self::METADATA_JSON.is_empty() {
            crate::Metadata::new(
                crate::Identifier::declared(Self::ID, "stream"),
                Self::ID,
                None,
            )
        } else {
            serde_json::from_str(Self::METADATA_JSON)
                .expect("macro-generated stream metadata is valid")
        }
    }

    fn row_metadata() -> Vec<crate::StreamField> {
        serde_json::from_str(Self::ROW_METADATA_JSON)
            .expect("macro-generated row metadata is valid")
    }
}

/* === Implementations === */
impl NamedManifest {
    pub fn resolve(
        self,
        declarations: &[Named],
        settings: &mut SchemaSettings,
    ) -> Result<Manifest, Error> {
        let streams = self
            .streams
            .into_iter()
            .map(|stream| {
                let row =
                    Schema::from_named(&stream.row, declarations, settings).map_err(|source| {
                        Error::Fields {
                            id: stream.id.clone(),
                            source: source.into(),
                        }
                    })?;
                let row = super::retain_schema(row, settings).map_err(|source| Error::Fields {
                    id: stream.id.clone(),
                    source: source.into(),
                })?;
                let stream = StreamDeclaration {
                    metadata: stream.metadata,
                    row_metadata: stream.row_metadata,
                    id: stream.id,
                    schema: stream.schema,
                    window: stream.window,
                    row,
                };
                validate(&stream)?;
                Ok(stream)
            })
            .collect::<Result<_, Error>>()?;
        Ok(Manifest {
            facility: self.facility,
            owner: self.owner,
            streams,
        })
    }
}

impl Entry {
    pub fn metadata(&self) -> crate::Metadata {
        if self.metadata_json.is_empty() {
            crate::Metadata::new(
                crate::Identifier::declared(self.id, "stream"),
                self.id,
                None,
            )
        } else {
            serde_json::from_str(self.metadata_json).expect("declared stream metadata is valid")
        }
    }

    pub fn row_metadata(&self) -> Vec<crate::StreamField> {
        serde_json::from_str(self.row_metadata_json).expect("declared row metadata is valid")
    }

    pub const fn of<S: StreamSpec>() -> Self
    where
        S::Row: crate::schema::Described,
    {
        Self {
            id: S::ID,
            metadata_json: S::METADATA_JSON,
            row_metadata_json: S::ROW_METADATA_JSON,
            schema: S::VERSION,
            window: S::WINDOW,
            row: <S::Row as crate::schema::Described>::DEFINITION,
        }
    }
}

/* === Public Functions === */
pub const fn merge<const N: usize>(named: &[Entry], sampled: &[Entry]) -> [Entry; N] {
    let mut merged = [Entry {
        id: "",
        metadata_json: "",
        row_metadata_json: "[]",
        schema: 0,
        window: 0,
        row: Definition::of::<()>(),
    }; N];
    let mut index = 0;
    while index < named.len() {
        merged[index] = named[index];
        index += 1;
    }
    let mut offset = 0;
    while offset < sampled.len() {
        merged[index + offset] = sampled[offset];
        offset += 1;
    }

    merged
}

pub const fn require_tree_subset(
    observers: &[crate::catalog::Node<super::observers::Entry>],
    grants: &[Entry],
) {
    let mut index = 0;
    while index < observers.len() {
        match &observers[index] {
            crate::catalog::Node::Leaf(entry) => require_subset(&[entry.streams], grants),
            crate::catalog::Node::Nested(nested) => require_tree_subset(nested, grants),
        }
        index += 1;
    }
}

pub const fn require_subset(outputs: &[&[Entry]], grants: &[Entry]) {
    let mut observer = 0;
    while observer < outputs.len() {
        let mut output = 0;
        while output < outputs[observer].len() {
            let required = &outputs[observer][output];
            let mut grant = 0;
            let mut found = false;
            while grant < grants.len() {
                let declared = &grants[grant];
                if same_bytes(required.id, declared.id)
                    && required.schema == declared.schema
                    && required.window == declared.window
                {
                    found = true;
                    break;
                }
                grant += 1;
            }
            assert!(
                found,
                "observer outputs must be a subset of the artifact publication grant"
            );
            output += 1;
        }
        observer += 1;
    }
}

pub fn select(
    manifests: Vec<Manifest>,
    facility: &str,
    owner: &str,
) -> Result<Vec<StreamDeclaration>, Error> {
    let mut matching = manifests
        .into_iter()
        .filter(|m| m.facility == facility && m.owner == owner);
    let manifest = matching.next().ok_or_else(|| Error::Missing {
        facility: facility.into(),
        owner: owner.into(),
    })?;
    if matching.next().is_some() {
        return Err(Error::Duplicate {
            facility: facility.into(),
            owner: owner.into(),
        });
    }
    let mut streams = BTreeMap::new();
    for stream in manifest.streams {
        validate(&stream)?;
        let id = stream.id.clone();
        if streams.insert(id.clone(), stream).is_some() {
            return Err(Error::DuplicateStream { id });
        }
    }
    Ok(streams.into_values().collect())
}

pub fn validate_match(expected: &[Entry], actual: &[StreamDeclaration]) -> Result<(), Error> {
    let mut authored = BTreeMap::new();
    for entry in expected {
        let metadata = if entry.metadata_json.is_empty() {
            let id = crate::Identifier::new(entry.id).map_err(|_| Error::InvalidId {
                id: entry.id.to_owned(),
            })?;
            crate::Metadata::new(id, entry.id, None)
        } else {
            serde_json::from_str(entry.metadata_json).map_err(|source| Error::Metadata {
                id: entry.id.to_owned(),
                reason: source.to_string(),
            })?
        };
        let row_metadata =
            serde_json::from_str(entry.row_metadata_json).map_err(|source| Error::Metadata {
                id: entry.id.to_owned(),
                reason: source.to_string(),
            })?;
        let stream = StreamDeclaration {
            metadata,
            row_metadata,
            id: entry.id.to_owned(),
            schema: entry.schema,
            window: entry.window,
            row: Schema::from_declaration(entry.row, &SchemaSettings::default()).map_err(
                |source| Error::Fields {
                    id: entry.id.to_owned(),
                    source: source.into(),
                },
            )?,
        };
        validate(&stream)?;
        if authored.insert(entry.id, stream).is_some() {
            return Err(Error::DuplicateStream {
                id: entry.id.to_owned(),
            });
        }
    }
    let mut admitted = BTreeMap::new();
    for stream in actual {
        validate(stream)?;
        if admitted.insert(stream.id.as_str(), stream).is_some() {
            return Err(Error::DuplicateStream {
                id: stream.id.clone(),
            });
        }
    }
    if authored.len() != admitted.len()
        || authored
            .iter()
            .any(|(id, stream)| admitted.get(id).is_none_or(|actual| *actual != stream))
    {
        return Err(Error::Mismatch);
    }
    Ok(())
}

pub fn validate(stream: &StreamDeclaration) -> Result<(), Error> {
    if !crate::stream_subset(&stream.id) {
        return Err(Error::InvalidId {
            id: stream.id.clone(),
        });
    }
    if stream.schema == 0 {
        return Err(Error::ZeroSchema {
            id: stream.id.clone(),
        });
    }
    if stream.window == 0 {
        return Err(Error::ZeroWindow {
            id: stream.id.clone(),
        });
    }
    if stream.metadata.identifier.as_str() != stream.id {
        return Err(Error::Metadata {
            id: stream.id.clone(),
            reason: "identifier differs from stream identity".into(),
        });
    }
    stream
        .row
        .prepare(&SchemaSettings::default())
        .map_err(|source| Error::Fields {
            id: stream.id.clone(),
            source: source.into(),
        })?;
    let mut described = std::collections::BTreeSet::new();
    for field in &stream.row_metadata {
        let name = field.metadata.identifier.as_str();
        if !stream.row.definitions[stream.row.root]
            .iter()
            .any(|wire| wire.name == name)
            || !described.insert(name)
        {
            return Err(Error::Metadata {
                id: stream.id.clone(),
                reason: format!("field `{name}` is unknown or described more than once"),
            });
        }
    }
    Ok(())
}

pub const fn encoded_len(facility: &str, owner: &str, streams: &[Entry]) -> usize {
    let mut len = HEAD.len()
        + quoted_len(facility)
        + OWNER.len()
        + quoted_len(owner)
        + STREAMS.len()
        + TAIL.len();
    let mut index = 0;
    while index < streams.len() {
        len += ID.len()
            + quoted_len(streams[index].id)
            + SCHEMA.len()
            + digits(streams[index].schema as u64)
            + WINDOW.len()
            + digits(streams[index].window as u64)
            + ROW.len()
            + quoted_len(streams[index].row.name)
            + METADATA.len()
            + metadata_len(&streams[index])
            + ROW_METADATA.len()
            + streams[index].row_metadata_json.len()
            + 1;
        if index > 0 {
            len += 1;
        }
        index += 1;
    }
    len
}

pub const fn encode<const N: usize>(facility: &str, owner: &str, streams: &[Entry]) -> [u8; N] {
    assert!(N == encoded_len(facility, owner, streams));
    let mut output = [0; N];
    let mut cursor = 0;
    write(&mut output, &mut cursor, HEAD);
    quoted(&mut output, &mut cursor, facility);
    write(&mut output, &mut cursor, OWNER);
    quoted(&mut output, &mut cursor, owner);
    write(&mut output, &mut cursor, STREAMS);
    let mut index = 0;
    while index < streams.len() {
        if index > 0 {
            write(&mut output, &mut cursor, b",");
        }
        write(&mut output, &mut cursor, ID);
        quoted(&mut output, &mut cursor, streams[index].id);
        write(&mut output, &mut cursor, SCHEMA);
        super::number(&mut output, &mut cursor, streams[index].schema as u64);
        write(&mut output, &mut cursor, WINDOW);
        super::number(&mut output, &mut cursor, streams[index].window as u64);
        write(&mut output, &mut cursor, ROW);
        quoted(&mut output, &mut cursor, streams[index].row.name);
        write(&mut output, &mut cursor, METADATA);
        metadata_into(&mut output, &mut cursor, &streams[index]);
        write(&mut output, &mut cursor, ROW_METADATA);
        write(
            &mut output,
            &mut cursor,
            streams[index].row_metadata_json.as_bytes(),
        );
        write(&mut output, &mut cursor, b"}");
        index += 1;
    }
    write(&mut output, &mut cursor, TAIL);
    output
}

/* === Private Functions === */
const fn same_bytes(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut i = 0;
    while i < left.len() {
        if left[i] != right[i] {
            return false;
        }
        i += 1;
    }
    true
}

const fn metadata_len(entry: &Entry) -> usize {
    if entry.metadata_json.is_empty() {
        METADATA_ID.len() + quoted_len(entry.id) + METADATA_LABEL.len() + quoted_len(entry.id) + 1
    } else {
        entry.metadata_json.len()
    }
}

const fn metadata_into<const N: usize>(output: &mut [u8; N], cursor: &mut usize, entry: &Entry) {
    if entry.metadata_json.is_empty() {
        write(output, cursor, METADATA_ID);
        quoted(output, cursor, entry.id);
        write(output, cursor, METADATA_LABEL);
        quoted(output, cursor, entry.id);
        write(output, cursor, b"}");
    } else {
        write(output, cursor, entry.metadata_json.as_bytes());
    }
}

/* === Tests === */
