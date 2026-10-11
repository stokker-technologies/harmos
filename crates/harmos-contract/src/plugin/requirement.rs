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
use crate::operations::{Declares, OperationDeclaration, WireDeclaration, WireOperation};
use crate::schema::Described;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Requirement {
    pub declaration: WireDeclaration,
    pub grants: &'static [Grant],
}

pub struct Requirements;

/* === Enums === */
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Grant {
    Survey(Cow<'static, str>),
    Egress(Cow<'static, str>),
    Source(Cow<'static, str>),
}

/* === Traits === */

/* === Implementations === */
impl Requirement {
    pub const fn of<O>(grants: &'static [Grant]) -> Self
    where
        O: WireOperation,
        O::Request: Described,
        O::Response: Described,
    {
        Self {
            declaration: WireDeclaration::of::<O>(),
            grants,
        }
    }
}

impl Grant {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Survey(_) => "survey",
            Self::Egress(_) => "egress",
            Self::Source(_) => "source",
        }
    }

    pub fn selector(&self) -> &str {
        match self {
            Self::Survey(value) | Self::Egress(value) | Self::Source(value) => value,
        }
    }

    pub(crate) const fn borrowed(&self) -> &'static str {
        match self {
            Self::Survey(Cow::Borrowed(value))
            | Self::Egress(Cow::Borrowed(value))
            | Self::Source(Cow::Borrowed(value)) => value,
            _ => panic!("const grants must be borrowed"),
        }
    }
}

impl Visitor for Requirements {
    type Entry = Requirement;
}

impl<M> Member<M> for Requirements
where
    M: Declares,
    M::Declaration: WireOperation,
    <M::Declaration as OperationDeclaration>::Request: Described,
    <M::Declaration as OperationDeclaration>::Response: Described,
{
    const ENTRY: Requirement = Requirement::of::<M::Declaration>(&[]);
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
