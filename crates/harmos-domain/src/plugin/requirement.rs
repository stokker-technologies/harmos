/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::catalog::{Member, Visitor};
use crate::operations::{OperationDeclaration, WireDeclaration};

// Permission to call one operation. An operation whose request names a channel,
// stream or source resolves it to `scope`; any other operation is granted by
// its id alone, which holding its requirement already confers.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct Grant {
    pub operation: Cow<'static, str>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Cow<'static, str>>,
}

pub struct Requirement {
    pub declaration: WireDeclaration,
    pub grants: &'static [Grant],
}

pub struct Requirements;

impl Requirement {
    pub const fn of<O: OperationDeclaration>(grants: &'static [Grant]) -> Self {
        Self {
            declaration: WireDeclaration::of::<O>(),
            grants,
        }
    }
}

impl Grant {
    pub fn of(operation: impl Into<Cow<'static, str>>, scope: impl Into<Cow<'static, str>>) -> Self {
        Self {
            operation: operation.into(),
            scope: Some(scope.into()),
        }
    }

    pub fn operation(&self) -> &str {
        &self.operation
    }

    pub fn scope(&self) -> Option<&str> {
        self.scope.as_deref()
    }

    pub(crate) const fn borrowed(&self) -> (&'static str, Option<&'static str>) {
        let operation = match &self.operation {
            Cow::Borrowed(value) => *value,
            Cow::Owned(_) => panic!("const grants must be borrowed"),
        };
        let scope = match &self.scope {
            None => None,
            Some(Cow::Borrowed(value)) => Some(*value),
            Some(Cow::Owned(_)) => panic!("const grants must be borrowed"),
        };
        (operation, scope)
    }
}

impl Visitor for Requirements {
    type Entry = Requirement;
}

impl<M> Member<M> for Requirements
where
    M: OperationDeclaration,
{
    const ENTRY: Requirement = Requirement::of::<M>(&[]);
}

// Upper Bound Includes Duplicates; Unused Nodes Are Empty Nested Projections.
pub const fn callee_count(nodes: &[crate::catalog::Node<&'static [WireDeclaration]>]) -> usize {
    let mut count = 0;
    let mut index = 0;
    while index < nodes.len() {
        count += match &nodes[index] {
            crate::catalog::Node::Leaf(entries) => entries.len(),
            crate::catalog::Node::Nested(entries) => callee_count(entries),
        };
        index += 1;
    }
    count
}

pub const fn aggregate<const N: usize>(
    explicit: &'static [crate::catalog::Node<Requirement>], provided: &[crate::catalog::Node<WireDeclaration>],
    callees: &[crate::catalog::Node<&'static [WireDeclaration]>],
) -> [crate::catalog::Node<Requirement>; N] {
    let mut output = [const { crate::catalog::Node::Nested(&[]) }; N];
    std::mem::forget(std::mem::replace(&mut output[0], crate::catalog::Node::Nested(explicit)));
    let mut cursor = 1;
    append_callees(callees, provided, &mut output, &mut cursor);
    output
}

const fn same(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

const fn provided(nodes: &[crate::catalog::Node<WireDeclaration>], id: &str) -> bool {
    let mut index = 0;
    while index < nodes.len() {
        let found = match &nodes[index] {
            crate::catalog::Node::Leaf(entry) => same(entry.operation.id, id),
            crate::catalog::Node::Nested(entries) => provided(entries, id),
        };
        if found {
            return true;
        }
        index += 1;
    }
    false
}

const fn required(nodes: &[crate::catalog::Node<Requirement>], id: &str) -> bool {
    let mut index = 0;
    while index < nodes.len() {
        let found = match &nodes[index] {
            crate::catalog::Node::Leaf(entry) => same(entry.declaration.operation.id, id),
            crate::catalog::Node::Nested(entries) => required(entries, id),
        };
        if found {
            return true;
        }
        index += 1;
    }
    false
}

const fn append_callees(
    nodes: &[crate::catalog::Node<&'static [WireDeclaration]>], providers: &[crate::catalog::Node<WireDeclaration>],
    output: &mut [crate::catalog::Node<Requirement>], cursor: &mut usize,
) {
    let mut index = 0;
    while index < nodes.len() {
        match &nodes[index] {
            crate::catalog::Node::Nested(entries) => append_callees(entries, providers, output, cursor),
            crate::catalog::Node::Leaf(entries) => {
                let mut member = 0;
                while member < entries.len() {
                    let declaration = &entries[member];
                    let id = declaration.operation.id;
                    if !provided(providers, id) && !required(output, id) {
                        std::mem::forget(std::mem::replace(
                            &mut output[*cursor],
                            crate::catalog::Node::Leaf(Requirement {
                                declaration: declaration.copied(),
                                grants: &[],
                            }),
                        ));
                        *cursor += 1;
                    }
                    member += 1;
                }
            }
        }
        index += 1;
    }
}
