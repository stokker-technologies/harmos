/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Stored Stream And Object Identities The Paged Catalog Lists.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::{ObjectKey, SchemaKey, StorageCursor};
use crate::Identifier;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentitySelection {
    pub kind: IdentityKind,
    pub schema: Option<Identifier>,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityKind {
    Streams,
    Objects,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StoredIdentity {
    Stream { schema: SchemaKey, scope: Option<Identifier>, item: Identifier, stream: Identifier, first: StorageCursor },
    Object { schema: SchemaKey, key: ObjectKey, first: StorageCursor },
}

/* === Traits === */

/* === Implementations === */

impl StoredIdentity {
    pub fn first(&self) -> &StorageCursor {
        match self {
            Self::Stream { first, .. } | Self::Object { first, .. } => first,
        }
    }

    pub fn schema(&self) -> &SchemaKey {
        match self {
            Self::Stream { schema, .. } | Self::Object { schema, .. } => schema,
        }
    }

    pub fn kind(&self) -> IdentityKind {
        match self {
            Self::Stream { .. } => IdentityKind::Streams,
            Self::Object { .. } => IdentityKind::Objects,
        }
    }
}

impl IdentitySelection {
    pub fn matches(&self, identity: &StoredIdentity) -> bool {
        identity.kind() == self.kind && self.schema.as_ref().is_none_or(|schema| schema == &identity.schema().id)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
