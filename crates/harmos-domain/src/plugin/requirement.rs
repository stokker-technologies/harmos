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
use std::borrow::Cow;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use crate::catalog::{Member, Visitor};
use crate::operations::{Declares, OperationDeclaration, WireDeclaration};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
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

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Requirement {
    pub const fn of<O: OperationDeclaration>(grants: &'static [Grant]) -> Self {
        Self { declaration: WireDeclaration::of::<O>(), grants }
    }
}

impl Grant {
    pub fn of(operation: impl Into<Cow<'static, str>>, scope: impl Into<Cow<'static, str>>) -> Self {
        Self { operation: operation.into(), scope: Some(scope.into()) }
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
    M: Declares,
{
    const ENTRY: Requirement = Requirement::of::<M::Declaration>(&[]);
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
