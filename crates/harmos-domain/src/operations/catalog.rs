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
use super::{Direction, OperationDeclaration, OperationDefinition, OperationPolicy, ReplaySafety, Visibility};
use crate::catalog::{self, Catalog, Member, Node, Projection, Visitor};
use crate::{Error, Identifier, Result};

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
    pub fingerprint: fn() -> Result<crate::Fingerprint>,
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
            && (self.fingerprint)().ok() == (other.fingerprint)().ok()
    }
}

impl Eq for Entry {}

impl Entry {
    pub const fn of<O: OperationDeclaration + ?Sized>() -> Self {
        Self {
            id: O::ID,
            description: O::DESCRIPTION,
            policy: O::POLICY,
            visibility: O::VISIBILITY,
            replay: O::REPLAY,
            fingerprint: fingerprint::<O>,
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
            fingerprint: (self.fingerprint)()?,
        };
        Ok(definition)
    }
}

impl Visitor for Declarations {
    type Entry = Entry;
}

impl<M: OperationDeclaration> Member<M> for Declarations {
    const ENTRY: Entry = Entry::of::<M>();
}

impl Declarations {
    pub fn of<C: Catalog<Self>>() -> Result<Vec<OperationDefinition>> {
        Self::from_projection(&C::ENTRIES)
    }

    pub fn from_tree(entries: &[Node<Entry>]) -> Result<Vec<OperationDefinition>> {
        let mut names = BTreeSet::new();
        let mut definitions = Vec::with_capacity(catalog::count(entries));
        catalog::try_for_each(entries, |declaration| append(declaration, &mut names, &mut definitions))?;
        Ok(definitions)
    }

    pub fn from_projection<P: Projection<Entry>>(entries: &P) -> Result<Vec<OperationDefinition>> {
        let mut names = BTreeSet::new();
        let mut definitions = Vec::with_capacity(P::LEN);
        entries.try_for_each(|declaration| append(declaration, &mut names, &mut definitions))?;
        Ok(definitions)
    }
}

impl Visitor for Callees {
    type Entry = &'static [super::WireDeclaration];
}

impl<M: OperationDeclaration> Member<M> for Callees {
    const ENTRY: Self::Entry = M::OPERATIONS;
}

/* === Public Functions === */

/* === Private Functions === */

fn payload<T: crate::Portable>() -> Result<crate::Fingerprint> {
    let schema = crate::schema::graph::Schema::from_declaration(T::DEFINITION, &crate::config::SchemaSettings::default())
        .map_err(|error| crate::MessageError::new(error.to_string()))?;
    let root = schema.root().map_err(|error| crate::MessageError::new(error.to_string()))?;
    crate::schema::identity::fingerprint(root, T::CODEC).map_err(|error| crate::MessageError::new(error.to_string()).into())
}

fn fingerprint<O: OperationDeclaration + ?Sized>() -> Result<crate::Fingerprint> {
    fingerprint_at::<O>(O::ID)
}

pub(super) fn fingerprint_at<O: OperationDeclaration + ?Sized>(id: &str) -> Result<crate::Fingerprint> {
    let input = if O::Input::STREAMED { Some(payload::<<O::Input as super::Direction>::Item>()?) } else { None };
    let output = if O::Output::STREAMED { Some(payload::<<O::Output as super::Direction>::Item>()?) } else { None };
    Ok(crate::schema::identity::operation(id, payload::<O::Request>()?, payload::<O::Response>()?, input, output))
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
