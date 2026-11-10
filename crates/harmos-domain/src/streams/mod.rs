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

pub trait StreamRow: crate::schema::Described + Clone + Send + Sync + Serialize + DeserializeOwned + 'static {
    const ID: &'static str;
    const WINDOW: usize;
    fn metadata() -> crate::Metadata {
        crate::Metadata::new(crate::Identifier::declared(Self::ID, "stream"), Self::ID, None)
    }
    fn row_metadata() -> Vec<crate::StreamField> {
        Vec::new()
    }
}

pub trait StreamCatalog<S: StreamRow> {}

/* === Implementations === */

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
