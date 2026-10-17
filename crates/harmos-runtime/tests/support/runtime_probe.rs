/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![allow(dead_code)]
/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_runtime::state::{Invert, Transaction, Undoable};
use harmos_runtime::{Application, DefinitionId, state::Kind};

/* === Constants === */
pub(crate) const RENAME: (Kind, &str, u32) = (Kind::Transaction, "sheet.rename", 1);

pub(crate) const EXPORTED: (Kind, &str, u32) = (Kind::Record, "sheet.exported", 1);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Sheet {
    pub(crate) title: String,
    pub(crate) sealed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Rename {
    pub(crate) to: String,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Editor;

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Refusal {
    #[error("the title must not be empty")]
    Untitled,
    #[error("the sheet is sealed")]
    Sealed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    Rename(Rename),
    Seal,
    Explode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Note {
    Exported,
}

/* === Traits === */

/* === Implementations === */
impl Transaction<Sheet> for Rename {
    type Error = Refusal;

    fn check(&self, sheet: &Sheet) -> Result<(), Self::Error> {
        if sheet.sealed {
            return Err(Refusal::Sealed);
        }
        (!self.to.is_empty()).then_some(()).ok_or(Refusal::Untitled)
    }

    fn apply(&self, sheet: &mut Sheet) {
        sheet.title.clone_from(&self.to);
    }
}

impl Undoable for Rename {
    type Inverse = Self;
}

impl Invert<Sheet> for Rename {
    fn invert(&self, before: &Sheet) -> Self {
        Self {
            to: before.title.clone(),
        }
    }
}

impl Transaction<Sheet> for Change {
    type Error = Refusal;

    fn check(&self, sheet: &Sheet) -> Result<(), Self::Error> {
        match self {
            Self::Rename(change) => change.check(sheet),
            _ => Ok(()),
        }
    }

    fn apply(&self, sheet: &mut Sheet) {
        match self {
            Self::Rename(change) => change.apply(sheet),
            Self::Seal => sheet.sealed = true,
            Self::Explode => panic!("apply exploded"),
        }
    }
}

impl From<Rename> for Change {
    fn from(change: Rename) -> Self {
        Self::Rename(change)
    }
}

impl Application for Editor {
    type State = Sheet;
    type Transaction = Change;
    type Record = Note;
    type Streams = ();
    type Resources = ();
}

impl harmos_runtime::state::TransactionCatalog<Sheet> for Change {}

impl harmos_runtime::state::CatalogCodec for Change {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        (
            harmos_runtime::DefinitionId::new(match self {
                Self::Rename(_) => "sheet.rename",
                Self::Seal => "sheet.seal",
                Self::Explode => "sheet.explode",
            }),
            1,
        )
    }
    fn encode<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Rename(v) => serde::Serialize::serialize(v, serializer),
            Self::Seal => serializer.serialize_unit(),
            Self::Explode => serializer.serialize_unit(),
        }
    }
}

impl harmos_runtime::state::CatalogCodec for Note {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        (harmos_runtime::DefinitionId::new("sheet.exported"), 1)
    }
    fn encode<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_unit()
    }
}

impl harmos_runtime::state::RecordCatalog for Note {
    fn restore(
        id: &harmos_runtime::DefinitionId,
        version: u32,
        _payload: &serde_json::Value,
    ) -> Result<Option<Self>, serde_json::Error> {
        if id.as_str() != "sheet.exported" || version != 1 {
            return Ok(None);
        }
        Ok(Some(Self::Exported))
    }
}

/* === Public Functions === */
pub(crate) fn rename(title: &str) -> Change {
    Change::Rename(Rename {
        to: title.to_owned(),
    })
}

pub(crate) fn named((kind, id, version): (Kind, &str, u32)) -> (Kind, DefinitionId, u32) {
    (kind, DefinitionId::new(id), version)
}

pub(crate) fn inverses(change: &Change, sheet: &Sheet) -> Option<Change> {
    match change {
        Change::Rename(change) => Some(change.invert(sheet).into()),
        Change::Seal | Change::Explode => None,
    }
}

/* === Private Functions === */

/* === Tests === */
