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

// Local Imports
use super::{Direction, OperationDefinition, OperationPolicy, ReplaySafety, Visibility};
use crate::catalog::{self, Node, Visitor};
use crate::operations::{Error, Result};
use harmos_domain::Identifier;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub id: &'static str,
    pub description: &'static str,
    pub policy: OperationPolicy,
    pub visibility: Visibility,
    pub replay: ReplaySafety,
    pub fingerprint: fn(&str) -> Result<harmos_domain::Fingerprint>,
}

pub struct Declarations;

pub struct Callees;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.description == other.description
            && self.policy == other.policy
            && self.visibility == other.visibility
            && self.replay == other.replay
            && (self.fingerprint)(self.id).ok() == (other.fingerprint)(other.id).ok()
    }
}

impl Eq for Entry {}

impl Entry {
    pub const fn request<Q: super::RequestShape>(id: &'static str, policy: OperationPolicy) -> Self {
        Self {
            id,
            description: "",
            policy,
            visibility: Visibility::Private,
            replay: ReplaySafety::Unknown,
            fingerprint: request_fingerprint::<Q>,
        }
    }

    pub fn definition(&self) -> Result<OperationDefinition> {
        let definition = self.declared()?;
        definition.validate()?;
        Ok(definition)
    }

    pub(super) fn declared(&self) -> Result<OperationDefinition> {
        let id = Identifier::new(self.id)?;
        let definition = OperationDefinition {
            id,
            description: self.description.to_owned(),
            policy: self.policy,
            visibility: self.visibility,
            replay: self.replay,
            fingerprint: (self.fingerprint)(self.id)?,
        };
        Ok(definition)
    }
}

impl Visitor for Declarations {
    type Entry = Entry;
}

impl Declarations {
    pub fn from_tree(entries: &[Node<Entry>]) -> Result<Vec<OperationDefinition>> {
        let mut names = BTreeSet::new();
        let mut definitions = Vec::with_capacity(catalog::count(entries));
        catalog::try_for_each(entries, |declaration| append(declaration, &mut names, &mut definitions))?;
        Ok(definitions)
    }
}

impl Visitor for Callees {
    type Entry = &'static [super::WireDeclaration];
}

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn payload<T: harmos_domain::Portable>() -> Result<harmos_domain::Fingerprint> {
    let schema = harmos_domain::schema::graph::Schema::from_declaration(T::DEFINITION, &harmos_domain::config::SchemaSettings::default())
        .map_err(|error| harmos_domain::MessageError::new(error.to_string()))?;
    let root = schema.root().map_err(|error| harmos_domain::MessageError::new(error.to_string()))?;
    harmos_domain::schema::identity::fingerprint(root, super::Codec::Protobuf.id())
        .map_err(|error| harmos_domain::MessageError::new(error.to_string()).into())
}

pub(super) fn request_fingerprint<Q: super::RequestShape>(id: &str) -> Result<harmos_domain::Fingerprint> {
    let input = if Q::Input::STREAMED { Some(payload::<<Q::Input as super::Direction>::Item>()?) } else { None };
    let output = if Q::Output::STREAMED { Some(payload::<<Q::Output as super::Direction>::Item>()?) } else { None };
    Ok(harmos_domain::schema::identity::operation(id, payload::<Q::Request>()?, payload::<Q::Response>()?, input, output))
}

fn append(declaration: &Entry, names: &mut BTreeSet<Identifier>, definitions: &mut Vec<OperationDefinition>) -> Result<()> {
    let definition = declaration.definition()?;
    if !names.insert(definition.id.clone()) {
        return Err(Error::DuplicateOperation { id: declaration.id.to_owned() });
    }
    definitions.push(definition);
    Ok(())
}

/* === Tests === */
