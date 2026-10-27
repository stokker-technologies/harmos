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
    #[error("no definition is declared for the id {id}")]
    UnknownDefinition { id: DefinitionId },

    #[error("{id} declares no implementation for version {version}")]
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

pub trait TransactionCatalog<S>: Transaction<S> + super::record::CatalogCodec {
    fn replay(&self, prepared: &super::record::Prepared, state: &mut S) -> Result<(), DecodeError> {
        if self.identify() != (prepared.id.clone(), prepared.version) {
            return Err(DecodeError::Malformed {
                id: prepared.id.clone(),
                version: prepared.version,
            });
        }
        self.apply(state);
        Ok(())
    }
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

// The Attribute's `inverse = …` Declaration, Readable Without A State Type So
// The Pair Check Runs Where The Change Is Declared Rather Than Where It Is Used.
#[diagnostic::on_unimplemented(
    message = "`{Self}` declares no inverse",
    note = "a change answers the undo question where it is defined: `inverse = …` names the \
            change that takes it back, `inverse = Self` says it is its own, and `irreversible` \
            says it cannot be taken back"
)]
pub trait Paired {
    type Inverse;
}

#[diagnostic::on_unimplemented(
    message = "the declared inverse of `{T}` does not name `{T}` back",
    note = "inverses come in pairs: when `{T}` declares `inverse = X`, `X` declares `inverse = \
            {T}` — or `inverse = Self`, which is its own pair"
)]
pub trait Mutual<T> {}

#[diagnostic::on_unimplemented(
    message = "the declared inverse `{Self}` is not a member of `{C}`",
    note = "add `{Self}` to `{C}` as its own variant — a catalog never invents one"
)]
pub trait Catalogued<C>: Sized {
    fn catalogued(self) -> C;
}

/* === Implementations === */

impl<S> TransactionCatalog<S> for Never {}

impl super::record::CatalogCodec for Never {
    fn identify(&self) -> (DefinitionId, u32) {
        match *self {}
    }

    fn encode<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        match *self {}
    }
}

impl super::record::RecordCatalog for Never {
    fn restore(_: &DefinitionId, _: u32, _: &serde_json::Value) -> Result<Option<Self>, serde_json::Error> {
        Ok(None)
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

impl<T> Mutual<T> for T {}

// The membership question is `Catalogued`, not the `Into` it is spelled with:
// pointing an author at a missing `From` impl would send them to write one by
// hand instead of to the catalog line that is actually missing a variant.
#[diagnostic::do_not_recommend]
impl<C, T: Into<C>> Catalogued<C> for T {
    fn catalogued(self) -> C {
        self.into()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
