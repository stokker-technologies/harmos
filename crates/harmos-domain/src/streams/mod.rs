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
use std::any::{Any, TypeId};
use std::hash::Hash;
use std::sync::Arc;
// Third Party Imports
use serde::Serialize;
use serde::de::DeserializeOwned;
// Local Imports

/* === Constants === */

/* === Macros === */

macro_rules! prefix {
    (($($whole:ident),+) selects ($($taken:ident),+) at ($($at:tt),+)) => {
        impl<$($whole: Track),+> Prefix<($($whole,)+)> for ($($taken,)+) {
            fn labels(&self) -> Vec<String> {
                vec![$(self.$at.label()),+]
            }

            fn selects(&self, whole: &($($whole,)+)) -> bool {
                $(self.$at == whole.$at)&&+
            }
        }
    };
}

prefix!((A) selects (A) at (0));

prefix!((A, B) selects (A) at (0));

prefix!((A, B) selects (A, B) at (0, 1));

prefix!((A, B, C) selects (A) at (0));

prefix!((A, B, C) selects (A, B) at (0, 1));

prefix!((A, B, C) selects (A, B, C) at (0, 1, 2));

prefix!((A, B, C, D) selects (A) at (0));

prefix!((A, B, C, D) selects (A, B) at (0, 1));

prefix!((A, B, C, D) selects (A, B, C) at (0, 1, 2));

prefix!((A, B, C, D) selects (A, B, C, D) at (0, 1, 2, 3));

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
    batch: Option<BatchDefinition>,
    owner: Option<Arc<str>>,
    schema: Option<Arc<crate::schema::graph::Schema>>,
}

#[derive(Clone, Copy, Debug)]

pub struct BatchDefinition {
    samples: u64,
    fields: &'static [&'static str],
    measure: fn(&dyn Any, &mut dyn FnMut(f64)),
}

/* === Enums === */

/* === Traits === */

pub trait StreamRow: Clone + Send + Sync + Serialize + DeserializeOwned + 'static {
    const ID: &'static str;
    const VERSION: u32;
    const WINDOW: usize;
    fn metadata() -> crate::Metadata {
        crate::Metadata::new(crate::Identifier::declared(Self::ID, "stream"), Self::ID, None)
    }
    fn row_metadata() -> Vec<crate::StreamField> {
        Vec::new()
    }
    fn batch_definition() -> Option<BatchDefinition> {
        None
    }
}

pub trait StreamCatalog<S: StreamRow> {}

pub trait Track: Clone + Eq + Hash + Send + Sync + 'static {
    fn label(&self) -> String;
}

pub trait Prefix<T>: Clone + Send + Sync + 'static {
    fn labels(&self) -> Vec<String>;

    fn selects(&self, whole: &T) -> bool;
}

pub trait Tracked {
    const TRACK: &'static [&'static str];

    type Track: Prefix<Self::Track>;

    fn track(&self) -> Self::Track;
}

pub trait Axial {
    const AXIS: &'static str;

    fn axis(&self) -> u64;
}

pub trait Batched: 'static {
    const SAMPLES: u64;
    const MEASURES: &'static [&'static str];

    fn measure(&self, visit: &mut dyn FnMut(f64));
}

/* === Implementations === */

impl Definition {
    pub fn row(&self) -> TypeId {
        self.row
    }

    pub fn batch(&self) -> Option<BatchDefinition> {
        self.batch
    }

    pub fn publication(mut self, owner: impl Into<Arc<str>>, schema: impl Into<Arc<crate::schema::graph::Schema>>) -> Self {
        self.owner = Some(owner.into());
        self.schema = Some(schema.into());
        self
    }

    pub fn declared<S: 'static>(id: impl Into<Arc<str>>, version: u32, window: usize) -> Self {
        let id = id.into();
        Self {
            metadata: crate::Metadata::new(crate::Identifier::declared(id.as_ref(), "stream"), id.as_ref(), None),
            row_metadata: Vec::new(),
            id,
            version,
            window,
            row: TypeId::of::<S>(),
            batch: None,
            owner: None,
            schema: None,
        }
    }

    pub fn batching(mut self, batch: Option<BatchDefinition>) -> Self {
        self.batch = batch;
        self
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

    pub fn schema(&self) -> Option<&crate::schema::graph::Schema> {
        self.schema.as_deref()
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn window(&self) -> usize {
        self.window
    }
}

impl BatchDefinition {
    pub fn samples(&self) -> u64 {
        self.samples
    }

    pub fn fields(&self) -> &'static [&'static str] {
        self.fields
    }

    pub fn measure(&self, row: &dyn Any, visit: &mut dyn FnMut(f64)) {
        (self.measure)(row, visit);
    }

    pub fn of<S: Batched>() -> Self {
        assert!(S::SAMPLES > 0 && !S::MEASURES.is_empty(), "a batch needs samples and measured fields");
        Self {
            samples: S::SAMPLES,
            fields: S::MEASURES,
            measure: |row, visit| row.downcast_ref::<S>().expect("declared stream row").measure(visit),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
