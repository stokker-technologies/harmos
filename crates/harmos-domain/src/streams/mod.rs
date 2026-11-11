/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod revision;

/* === Re-Exports === */

pub use revision::revision;

/* === Imports === */

// Standard Library Imports
use std::any::TypeId;
use std::sync::Arc;
// Third Party Imports
use serde::Serialize;
use serde::de::DeserializeOwned;
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct StreamEntry<R> {
    id: &'static str,
    window: usize,
    presentation: Presentation,
    row: std::marker::PhantomData<fn() -> R>,
}

#[derive(Clone, Copy)]
pub struct Presentation {
    pub(crate) label: &'static str,
    pub(crate) description: Option<&'static str>,
    pub(crate) tags: &'static [&'static str],
    pub(crate) attributes_json: &'static str,
}

#[derive(Clone, Debug)]
// A declaration is a value an application *states*; only the host core reads
// what it states. In an authoring build there is no host core, so these fields
// are written and never read — which is the shape of the thing, not a loose
// end.

pub struct Definition {
    metadata: crate::Metadata,
    row_metadata: Vec<crate::StreamField>,
    id: Arc<str>,
    version: u32,
    window: usize,
    row: TypeId,
    owner: Option<Arc<str>>,
    schema: Arc<crate::schema::graph::Schema>,
}

/* === Enums === */

/* === Traits === */

pub trait StreamRow: crate::schema::Described + crate::RowMetadata + Clone + Send + Sync + Serialize + DeserializeOwned + 'static {
    const ENTRY: StreamEntry<Self>;
    const ID: &'static str = Self::ENTRY.id();
    const WINDOW: usize = Self::ENTRY.window();

    fn metadata() -> crate::Metadata {
        Self::ENTRY.metadata()
    }
    fn definition() -> Definition {
        Self::ENTRY.build().expect("valid authored stream value")
    }
}

/* === Implementations === */

impl<R> Copy for StreamEntry<R> {}

impl<R> Clone for StreamEntry<R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<R> StreamEntry<R> {
    pub const fn new(id: &'static str, window: usize) -> Self {
        Self {
            id,
            window,
            presentation: Presentation {
                label: id,
                description: None,
                tags: &[],
                attributes_json: "{}",
            },
            row: std::marker::PhantomData,
        }
    }

    pub const fn id(self) -> &'static str {
        self.id
    }
    pub const fn window(self) -> usize {
        self.window
    }
    pub const fn label(mut self, label: &'static str) -> Self {
        self.presentation.label = label;
        self
    }
    pub const fn description(mut self, description: &'static str) -> Self {
        self.presentation.description = Some(description);
        self
    }
    pub const fn tags(mut self, tags: &'static [&'static str]) -> Self {
        self.presentation.tags = tags;
        self
    }
    pub const fn attributes_json(mut self, attributes: &'static str) -> Self {
        self.presentation.attributes_json = attributes;
        self
    }
    pub fn metadata(self) -> crate::Metadata {
        self.presentation.metadata(self.id).expect("valid authored stream presentation")
    }
}

impl<R: crate::schema::Described + crate::RowMetadata + 'static> StreamEntry<R> {
    pub const fn declaration(self) -> crate::inventory::publications::Entry {
        crate::inventory::publications::Entry {
            id: self.id,
            presentation: self.presentation,
            row_metadata_json: R::ROW_METADATA_JSON,
            window: self.window,
            row: R::DEFINITION,
        }
    }

    pub fn build(self) -> Result<Definition, crate::inventory::publications::Error> {
        use crate::inventory::publications::Error;
        if !crate::stream_subset(self.id) {
            return Err(Error::InvalidId { id: self.id.to_owned() });
        }
        if self.window == 0 {
            return Err(Error::ZeroWindow { id: self.id.to_owned() });
        }
        let metadata = self.presentation.metadata(self.id)?;
        let rows = <R as crate::RowMetadata>::row_metadata();
        let schema =
            crate::schema::graph::Schema::from_declaration(R::DEFINITION, &crate::config::SchemaSettings::default()).map_err(|source| {
                Error::Fields {
                    id: self.id.to_owned(),
                    source: source.into(),
                }
            })?;
        let declaration = crate::inventory::StreamDeclaration {
            metadata: metadata.clone(),
            row_metadata: rows.clone(),
            id: self.id.to_owned(),
            window: self.window,
            row: schema.clone(),
        };
        crate::inventory::publications::validate(&declaration)?;
        let definition = Definition::declared::<R>(self.id, schema, self.window).map_err(|source| Error::Fields {
            id: self.id.to_owned(),
            source: source.into(),
        })?;
        Ok(definition.presentation(metadata, rows))
    }
}

impl Presentation {
    pub(crate) fn metadata(self, id: &str) -> Result<crate::Metadata, crate::inventory::publications::Error> {
        use crate::inventory::publications::Error;
        let identifier = crate::Identifier::new(id).map_err(|_| Error::InvalidId { id: id.to_owned() })?;
        let attributes = serde_json::from_str(self.attributes_json).map_err(|error| Error::Metadata {
            id: id.to_owned(),
            reason: error.to_string(),
        })?;
        let mut unique = std::collections::BTreeSet::new();
        if self.tags.iter().any(|tag| !unique.insert(tag)) {
            return Err(Error::Metadata {
                id: id.to_owned(),
                reason: "duplicate tag".to_owned(),
            });
        }
        let mut metadata = crate::Metadata::new(identifier, self.label, self.description.map(str::to_owned));
        metadata.tags = self.tags.iter().map(|tag| (*tag).to_owned()).collect();
        metadata.attributes = attributes;
        Ok(metadata)
    }
}

impl Definition {
    pub fn row(&self) -> TypeId {
        self.row
    }

    pub fn publication(mut self, owner: impl Into<Arc<str>>) -> Self {
        self.owner = Some(owner.into());
        self
    }

    pub fn native<S: crate::schema::Described>(id: impl Into<Arc<str>>, window: usize) -> Self {
        let schema = crate::schema::graph::Schema::from_declaration(S::DEFINITION, &crate::config::SchemaSettings::default())
            .expect("valid native stream schema");
        Self::declared::<S>(id, schema, window).expect("valid native stream revision")
    }

    pub fn declared<S: 'static>(
        id: impl Into<Arc<str>>, schema: crate::schema::graph::Schema, window: usize,
    ) -> Result<Self, crate::schema::graph::Error> {
        let version = revision(&schema, window)?;
        let id = id.into();
        Ok(Self {
            metadata: crate::Metadata::new(crate::Identifier::declared(id.as_ref(), "stream"), id.as_ref(), None),
            row_metadata: Vec::new(),
            id,
            version,
            window,
            row: TypeId::of::<S>(),
            owner: None,
            schema: Arc::new(schema),
        })
    }

    pub fn presentation(mut self, metadata: crate::Metadata, rows: Vec<crate::StreamField>) -> Self {
        self.metadata = metadata;
        self.row_metadata = rows;
        self
    }

    pub fn metadata(&self) -> &crate::Metadata {
        &self.metadata
    }

    pub fn row_metadata(&self) -> &[crate::StreamField] {
        &self.row_metadata
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }

    pub fn schema(&self) -> &crate::schema::graph::Schema {
        &self.schema
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn window(&self) -> usize {
        self.window
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
