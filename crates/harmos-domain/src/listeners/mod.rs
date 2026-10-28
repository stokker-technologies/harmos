/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod delivery;

pub use delivery::{Callback, Context, ContextError, Delivery, EncodingError, Listener, encode_borrowed, encoding_memory_bytes};

use std::borrow::Cow;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::catalog::{Catalog, Projection, Visitor};
use crate::config::SchemaSettings;
use crate::operations::{Contract, ContractError, ReplaySafety, RetryPolicy, Visibility, WireDeclaration};
use crate::schema::declaration::Definition as SchemaDefinition;
use crate::schema::graph::Schema;
use crate::schema::{self};
use crate::{Identifier, IdentifierError};

const MAX_LISTENERS: usize = 4096;

pub struct Entry {
    pub id: &'static str,
    pub source: SourceDeclaration,
    pub callback: WireDeclaration,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "DecodedDefinition")]
pub struct Definition {
    id: Identifier,
    source: Source,
    callback: Contract,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecodedDefinition {
    id: Identifier,
    source: Source,
    callback: Contract,
}

pub struct Declarations;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Record { id: Cow<'static, str>, version: u32, row: Schema },
    Stream { id: Cow<'static, str>, schema_ordinal: u32, row: Schema },
}

pub enum SourceDeclaration {
    Record { id: &'static str, version: u32, row: SchemaDefinition },
    Stream { id: &'static str, schema_ordinal: u32, row: SchemaDefinition },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Identity(#[from] IdentifierError),

    #[error("invalid listener source: {reason}")]
    Source { reason: &'static str },

    #[error(transparent)]
    Schema(#[from] schema::Error),

    #[error(transparent)]
    Contract(#[from] ContractError),

    #[error("invalid listener callback: {reason}")]
    Callback { reason: &'static str },

    #[error("duplicate listener or callback operation `{id}`")]
    Duplicate { id: String },

    #[error("selected listener catalog exceeds 4096 leaves")]
    Limit,

    #[error("listener projection count differs from its declared leaf count")]
    Count,
}

impl Source {
    pub fn id(&self) -> &str {
        match self {
            Self::Record { id, .. } | Self::Stream { id, .. } => id,
        }
    }

    pub fn schema(&self) -> &Schema {
        match self {
            Self::Record { row, .. } | Self::Stream { row, .. } => row,
        }
    }

    pub fn matches_context(&self, context: Context) -> bool {
        context.validate().is_ok()
            && matches!((self, context), (Self::Record { .. }, Context::Record { .. }) | (Self::Stream { .. }, Context::Stream { .. }))
    }

    fn validate(&self) -> Result<(), Error> {
        if self.id().trim().is_empty() || self.id().chars().any(char::is_control) {
            return Err(Error::Source {
                reason: "source identities must be nonempty and contain no control characters",
            });
        }
        match self {
            Self::Record { version: 0, .. } => {
                return Err(Error::Source {
                    reason: "record semantic version must be positive",
                });
            }
            Self::Stream { schema_ordinal: 0, .. } => {
                return Err(Error::Source {
                    reason: "stream schema ordinal must be positive",
                });
            }
            _ => {}
        }
        self.schema().prepare(&SchemaSettings::default()).map_err(schema::Error::from)?;
        Ok(())
    }
}

impl SourceDeclaration {
    pub fn row(&self) -> SchemaDefinition {
        match self {
            Self::Record { row, .. } | Self::Stream { row, .. } => *row,
        }
    }

    pub fn source(&self) -> Result<Source, Error> {
        let row = Schema::from_declaration(self.row(), &SchemaSettings::default()).map_err(schema::Error::from)?;
        Ok(match self {
            Self::Record { id, version, .. } => Source::Record {
                id: Cow::Borrowed(id),
                version: *version,
                row,
            },
            Self::Stream { id, schema_ordinal, .. } => Source::Stream {
                id: Cow::Borrowed(id),
                schema_ordinal: *schema_ordinal,
                row,
            },
        })
    }
}

impl Entry {
    pub fn definition(&self) -> Result<Definition, Error> {
        let id = Identifier::new(self.id)?;
        let source = self.source.source()?;
        let callback = self.callback.contract()?;
        Definition::new(id, source, callback)
    }
}

impl Definition {
    pub fn new(id: Identifier, source: Source, callback: Contract) -> Result<Self, Error> {
        source.validate()?;
        let operation = callback.operation();
        if operation.visibility != Visibility::Shared
            || operation.policy.retries != RetryPolicy::Never
            || operation.replay != ReplaySafety::Unknown
        {
            return Err(Error::Callback {
                reason: "callbacks must be Shared, Never retry or restart, and Unknown replay safety",
            });
        }
        let matches = delivery::matches(callback.request(), source.schema(), &SchemaSettings::default()).map_err(schema::Error::from)?;
        if !matches || !callback.response().fields().is_empty() {
            return Err(Error::Callback {
                reason: "callback request must be exactly Delivery<Row> for this source, with a unit response",
            });
        }
        Ok(Self { id, source, callback })
    }

    pub fn id(&self) -> &Identifier {
        &self.id
    }
    pub fn source(&self) -> &Source {
        &self.source
    }
    pub fn callback(&self) -> &Contract {
        &self.callback
    }
}

impl TryFrom<DecodedDefinition> for Definition {
    type Error = Error;
    fn try_from(decoded: DecodedDefinition) -> Result<Self, Self::Error> {
        Self::new(decoded.id, decoded.source, decoded.callback)
    }
}

impl Visitor for Declarations {
    type Entry = Entry;
}

impl Declarations {
    pub fn of<C: Catalog<Self>>() -> Result<Vec<Definition>, Error> {
        let expected = <C::Entries as Projection<Entry>>::LEN;
        collect(expected, |visit| C::ENTRIES.try_for_each(visit))
    }

    pub fn from_tree(entries: &[crate::catalog::Node<Entry>]) -> Result<Vec<Definition>, Error> {
        let expected = crate::catalog::count(entries);
        collect(expected, |visit| crate::catalog::try_for_each(entries, visit))
    }
}

fn collect(
    expected: usize, project: impl FnOnce(&mut dyn FnMut(&Entry) -> Result<(), Error>) -> Result<(), Error>,
) -> Result<Vec<Definition>, Error> {
    if expected > MAX_LISTENERS {
        return Err(Error::Limit);
    }
    let mut definitions = Vec::with_capacity(expected);
    let mut listeners = BTreeSet::new();
    let mut callbacks = BTreeSet::new();
    let mut visited = 0usize;
    project(&mut |entry| {
        visited = visited.checked_add(1).ok_or(Error::Limit)?;
        if visited > MAX_LISTENERS {
            return Err(Error::Limit);
        }
        if visited > expected {
            return Err(Error::Count);
        }
        let definition = entry.definition()?;
        if !listeners.insert(definition.id.clone()) {
            return Err(Error::Duplicate { id: definition.id.to_string() });
        }
        if !callbacks.insert(definition.callback.operation().id.clone()) {
            return Err(Error::Duplicate {
                id: definition.callback.operation().id.to_string(),
            });
        }
        definitions.push(definition);
        Ok(())
    })?;
    if visited != expected {
        return Err(Error::Count);
    }
    Ok(definitions)
}
