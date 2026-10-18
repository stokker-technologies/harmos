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
use super::{Declares, Definition, OperationDeclaration, OperationPolicy, ReplaySafety, Visibility};
use crate::catalog::{self, Catalog, Member, Node, Projection, Visitor};
use crate::{Error, Identifier, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: &'static str,
    pub description: &'static str,
    pub policy: OperationPolicy,
    pub visibility: Visibility,
    pub replay: ReplaySafety,
}

pub struct Declarations;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Entry {
    pub const fn of<O: OperationDeclaration>() -> Self {
        Self { id: O::ID, description: O::DESCRIPTION, policy: O::POLICY, visibility: O::VISIBILITY, replay: O::REPLAY }
    }

    pub fn definition(&self) -> Result<Definition> {
        let id = Identifier::new(self.id)?;
        let definition = Definition {
            id,
            description: self.description.to_owned(),
            policy: self.policy,
            visibility: self.visibility,
            replay: self.replay,
        };
        definition.validate()?;
        Ok(definition)
    }
}

impl Visitor for Declarations {
    type Entry = Entry;
}

impl<M: Declares> Member<M> for Declarations {
    const ENTRY: Entry = Entry::of::<M::Declaration>();
}

impl Declarations {
    pub fn of<C: Catalog<Self>>() -> Result<Vec<Definition>> {
        Self::from_projection(&C::ENTRIES)
    }

    pub fn from_entries(entries: &[Entry]) -> Result<Vec<Definition>> {
        let mut names = BTreeSet::new();
        let mut definitions = Vec::with_capacity(entries.len());
        for declaration in entries {
            append(declaration, &mut names, &mut definitions)?;
        }
        Ok(definitions)
    }

    pub fn from_tree(entries: &[Node<Entry>]) -> Result<Vec<Definition>> {
        let mut names = BTreeSet::new();
        let mut definitions = Vec::with_capacity(catalog::count(entries));
        catalog::try_for_each(entries, |declaration| append(declaration, &mut names, &mut definitions))?;
        Ok(definitions)
    }

    pub fn from_projection<P: Projection<Entry>>(entries: &P) -> Result<Vec<Definition>> {
        let mut names = BTreeSet::new();
        let mut definitions = Vec::with_capacity(P::LEN);
        entries.try_for_each(|declaration| append(declaration, &mut names, &mut definitions))?;
        Ok(definitions)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn append(declaration: &Entry, names: &mut BTreeSet<Identifier>, definitions: &mut Vec<Definition>) -> Result<()> {
    let definition = declaration.definition()?;
    if !names.insert(definition.id.clone()) {
        return Err(Error::DuplicateOperation { id: declaration.id.to_owned() });
    }
    definitions.push(definition);
    Ok(())
}

/* === Tests === */
