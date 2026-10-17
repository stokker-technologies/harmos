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

// Local Imports
use crate::model::storage::codec::{Bytes, Code, Id, Index, Optional, Text, coded, product};
use crate::plugin::Grant;
use crate::{Identifier, MemorySize};

/* === Constants === */

/* === Macros === */
product!(Catalog {
    kind: Code<CatalogKind> => 1,
    schema: Optional<Id> => 2,
    after: Optional<Text> => 3,
    rows: Optional<Index> => 4,
    bytes: Optional<Index> => 5,
});

product!(CatalogPage { entries: Bytes => 1, next: Optional<Text> => 2 });

coded!(CatalogKind {
    Operations,
    Schemas,
    Streams,
    Objects
});

/* === Type Aliases === */

/* === Structs === */
/// One page of the catalog: live operation contracts, or the schemas, streams
/// or objects storage holds durably. `schema` narrows the stored kinds to one
/// schema id; `after` continues from the previous page's `next`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    pub kind: CatalogKind,
    pub schema: Option<Identifier>,
    pub after: Option<String>,
    pub rows: Option<usize>,
    pub bytes: Option<usize>,
}

/// `entries` is a UTF-8 JSON array of the kind's canonical entries:
/// `DiscoveredOperation`, stored schemas, or `StoredIdentity` values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CatalogPage {
    pub entries: Vec<u8>,
    pub next: Option<String>,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CatalogKind {
    #[default]
    Operations,
    Schemas,
    Streams,
    Objects,
}

/* === Traits === */

/* === Implementations === */
impl Catalog {
    // Stored Data Follows The Storage Read Grant Grammar; Operation Discovery
    // Keeps Its Per-Contract Conditional Access.
    pub fn required_grant(&self) -> Option<Grant> {
        let (family, all) = match self.kind {
            CatalogKind::Operations => return None,
            CatalogKind::Schemas => ("schemas", "schemas:*"),
            CatalogKind::Streams => ("streams", "streams:*"),
            CatalogKind::Objects => ("objects", "objects:*"),
        };
        Some(Grant::of(
            "host.catalog",
            self.schema.as_ref().map_or_else(
                || all.to_owned(),
                |schema| format!("{family}:schema:{schema}"),
            ),
        ))
    }
}

impl MemorySize for Catalog {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.schema
                    .as_ref()
                    .map_or(0, |schema| schema.as_str().len()),
            )
            .saturating_add(self.after.as_ref().map_or(0, String::capacity))
    }
}

impl MemorySize for CatalogPage {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.entries.capacity())
            .saturating_add(self.next.as_ref().map_or(0, String::capacity))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
