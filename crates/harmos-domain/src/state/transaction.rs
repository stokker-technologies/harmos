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
use std::convert::Infallible;
// Third Party Imports

// Local Imports
use crate::state::envelope::DefinitionId;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Never {}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("stored definition {id} is not declared; choose a fresh storage root")]
    UnknownDefinition { id: DefinitionId },

    #[error("stored definition {id} has incompatible shape revision {version}; choose a fresh storage root")]
    UndeclaredVersion { id: DefinitionId, version: u32 },

    #[error("the payload did not decode as {id} version {version}")]
    Malformed { id: DefinitionId, version: u32 },
}

/* === Traits === */

pub trait Transaction<S> {
    type Error;

    fn check(&self, state: &S) -> Result<(), Self::Error>;

    fn records(&self, _state: &S, _out: &mut super::record::Records) {}

    fn apply(&self, state: &mut S);
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` declares an inverse but never describes it",
    label = "declared without a body",
    note = "write the body beside the change: `impl harmos::state::Invertible<{S}> for {Self} {{ type \
            Inverse = …; fn inverse(&self, before: &{S}) -> Self::Inverse {{ … }} }}`"
)]
pub trait Invertible<S>: Transaction<S> {
    type Inverse: Transaction<S>;

    fn inverse(&self, before: &S) -> Self::Inverse;
}

/* === Implementations === */

impl super::record::CatalogCodec for Never {
    fn identify(&self) -> (DefinitionId, u32) {
        match *self {}
    }

    fn encode<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        match *self {}
    }
}

impl<S> Transaction<S> for Never {
    type Error = Infallible;

    fn check(&self, _state: &S) -> Result<(), Self::Error> {
        match *self {}
    }

    fn apply(&self, _state: &mut S) {
        match *self {}
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
