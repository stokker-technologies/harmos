/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Stored Stream And Object Identities The Paged Catalog Lists.

use serde::{Deserialize, Serialize};

use super::{ObjectKey, SchemaKey, StorageCursor};
use crate::{Identifier, MemorySize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentitySelection {
    pub kind: IdentityKind,
    pub schema: Option<Identifier>,
}

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

impl MemorySize for IdentitySelection {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.schema.as_ref().map_or(0, |schema| schema.memory_size().saturating_sub(std::mem::size_of::<Identifier>())))
    }
}

impl MemorySize for StoredIdentity {
    fn memory_size(&self) -> usize {
        let base = std::mem::size_of::<Self>()
            .saturating_add(self.schema().memory_size().saturating_sub(std::mem::size_of::<SchemaKey>()))
            .saturating_add(self.first().memory_size().saturating_sub(std::mem::size_of::<StorageCursor>()));
        match self {
            Self::Stream { scope, item, stream, .. } => base
                .saturating_add(scope.as_ref().map_or(0, |scope| scope.memory_size().saturating_sub(std::mem::size_of::<Identifier>())))
                .saturating_add(item.memory_size().saturating_sub(std::mem::size_of::<Identifier>()))
                .saturating_add(stream.memory_size().saturating_sub(std::mem::size_of::<Identifier>())),
            Self::Object { key, .. } => base.saturating_add(key.memory_size().saturating_sub(std::mem::size_of::<ObjectKey>())),
        }
    }
}

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
