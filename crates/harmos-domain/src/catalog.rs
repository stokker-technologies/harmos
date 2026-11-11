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

pub use crate::inventory::observers::Entry as ObserverEntry;
pub use crate::inventory::publications::Entry as PublicationEntry;
pub use crate::listeners::Entry as ListenerEntry;
pub use crate::operations::{Entry as OperationEntry, WireDeclaration as WireEntry};
pub use crate::plugin::Requirement as RequirementEntry;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

pub enum Node<E: 'static> {
    Leaf(E),
    Nested(&'static [Node<E>]),
}

/* === Traits === */

pub trait Visitor {
    type Entry: 'static;
}

pub trait Member<T>: Visitor {
    const ENTRY: Self::Entry;
}

pub trait InertCatalog<V: Visitor<Entry = E>, E: 'static> {
    const ENTRIES: &'static [Node<E>];
    const LEN: usize = count(Self::ENTRIES);
}

/* === Implementations === */

impl<V: Visitor<Entry = E>, E: 'static> InertCatalog<V, E> for () {
    const ENTRIES: &'static [Node<E>] = &[];
}

/* === Public Functions === */

pub const fn count<E>(nodes: &[Node<E>]) -> usize {
    let mut len: usize = 0;
    let mut index = 0;
    while index < nodes.len() {
        let added = match &nodes[index] {
            Node::Leaf(_) => 1,
            Node::Nested(nested) => count(nested),
        };
        len = match len.checked_add(added) {
            Some(len) => len,
            None => panic!("catalog leaf count overflows usize"),
        };
        index += 1;
    }
    len
}

pub fn try_for_each<E, F>(nodes: &[Node<E>], mut visit: impl FnMut(&E) -> Result<(), F>) -> Result<(), F> {
    visit_nodes(nodes, &mut visit)
}

/* === Private Functions === */

fn visit_nodes<E, F>(nodes: &[Node<E>], visit: &mut impl FnMut(&E) -> Result<(), F>) -> Result<(), F> {
    for node in nodes {
        match node {
            Node::Leaf(entry) => visit(entry)?,
            Node::Nested(nested) => visit_nodes(nested, visit)?,
        }
    }
    Ok(())
}

/* === Tests === */
