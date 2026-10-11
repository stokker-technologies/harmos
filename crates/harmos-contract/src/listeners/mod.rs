/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod delivery;

/* === Re-Exports === */
pub use delivery::{Context, ContextError, Delivery, EncodingError};

/* === Imports === */
// Standard Library Imports
use std::borrow::Cow;
use std::collections::BTreeSet;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use crate::catalog::{Catalog, Projection, Visitor};
use crate::operations::{
    Contract, ContractError, ReplaySafety, RetryPolicy, Visibility, WireDeclaration,
};
use crate::schema::{self, Field};
use crate::{Identifier, IdentifierError};

/* === Constants === */
const MAX_LISTENERS: usize = 4096;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Entry {
    pub id: &'static str,
    pub source: Source,
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

/* === Enums === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Record {
        id: Cow<'static, str>,
        version: u32,
        fields: Cow<'static, [Field]>,
    },
    Stream {
        id: Cow<'static, str>,
        schema_ordinal: u32,
        fields: Cow<'static, [Field]>,
    },
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

/* === Traits === */

/* === Implementations === */
impl Source {
    pub fn id(&self) -> &str {
        match self {
            Self::Record { id, .. } | Self::Stream { id, .. } => id,
        }
    }

    pub fn fields(&self) -> &[Field] {
        match self {
            Self::Record { fields, .. } | Self::Stream { fields, .. } => fields,
        }
    }

    pub fn matches_context(&self, context: Context) -> bool {
        context.validate().is_ok()
            && matches!(
                (self, context),
                (Self::Record { .. }, Context::Record { .. })
                    | (Self::Stream { .. }, Context::Stream { .. })
            )
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
            Self::Stream {
                schema_ordinal: 0, ..
            } => {
                return Err(Error::Source {
                    reason: "stream schema ordinal must be positive",
                });
            }
            _ => {}
        }
        schema::validate(self.fields())?;
        Ok(())
    }
}

impl Entry {
    pub fn definition(&self) -> Result<Definition, Error> {
        let id = Identifier::new(self.id)?;
        self.source.validate()?;
        let callback = self.callback.contract()?;
        Definition::new(id, self.source.clone(), callback)
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
                reason: "callbacks must be Shared, Never retry, and Unknown replay safety",
            });
        }
        let request = delivery::fields(source.fields());
        if callback.request() != request || !callback.response().is_empty() {
            return Err(Error::Callback {
                reason: "callback request must be exactly Delivery<Row> for this source, with a unit response",
            });
        }
        Ok(Self {
            id,
            source,
            callback,
        })
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
        collect(expected, |visit| {
            crate::catalog::try_for_each(entries, visit)
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn collect(
    expected: usize,
    project: impl FnOnce(&mut dyn FnMut(&Entry) -> Result<(), Error>) -> Result<(), Error>,
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
            return Err(Error::Duplicate {
                id: definition.id.to_string(),
            });
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

/* === Tests === */
