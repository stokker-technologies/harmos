/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub(crate) mod dispatch {
    use std::collections::BTreeMap;

    use crate::Application;
    use crate::state::envelope::{DefinitionId, Envelope, Kind};
    use crate::state::record::Upgrade;
    use crate::state::transaction::DecodeError;

    pub(crate) type Restore<A, W> =
        fn(W, &mut <A as Application>::State) -> Result<(), DecodeError>;
    pub(crate) type Decode<A> = fn(
        &DefinitionId,
        u32,
        &[u8],
    )
        -> Result<Option<<A as Application>::Transaction>, DecodeError>;

    #[derive(Debug)]
    pub(crate) enum Restored<A: Application> {
        Applied,
        Recorded(A::Record),
        Skipped,
    }

    pub(crate) struct Dispatch<A: Application, W> {
        transactions: BTreeMap<(DefinitionId, u32), Restore<A, W>>,
        decoders: BTreeMap<(DefinitionId, u32), Decode<A>>,
        records: BTreeMap<(DefinitionId, u32), Upgrade<A, W>>,
    }

    impl<A: Application, W> Dispatch<A, W> {
        pub(crate) fn set_transaction(
            &mut self,
            id: DefinitionId,
            version: u32,
            restore: Restore<A, W>,
            decode: Decode<A>,
        ) {
            self.transactions.insert((id.clone(), version), restore);
            self.decoders.insert((id, version), decode);
        }

        pub(crate) fn set_record(
            &mut self,
            id: DefinitionId,
            version: u32,
            upgrade: Upgrade<A, W>,
        ) {
            self.records.insert((id, version), upgrade);
        }

        pub(crate) fn restore(
            &self,
            envelope: Envelope<W>,
            state: &mut A::State,
        ) -> Result<Restored<A>, DecodeError> {
            let Envelope {
                kind,
                id,
                version,
                payload,
                ..
            } = envelope;

            match kind {
                Kind::Transaction => {
                    let restore = declared(&self.transactions, &id, version)?;
                    restore(payload, state)?;
                    Ok(Restored::Applied)
                }
                Kind::Record => match declared(&self.records, &id, version) {
                    Ok(upgrade) => Ok(Restored::Recorded(upgrade(payload)?)),
                    Err(DecodeError::UnknownDefinition { .. }) => Ok(Restored::Skipped),
                    Err(loud) => Err(loud),
                },
            }
        }

        pub(crate) fn has_record(
            &self,
            id: &DefinitionId,
            version: u32,
        ) -> Result<bool, DecodeError> {
            match declared(&self.records, id, version) {
                Ok(_) => Ok(true),
                Err(DecodeError::UnknownDefinition { .. }) => Ok(false),
                Err(error) => Err(error),
            }
        }

        pub(crate) fn decode_transaction(
            &self,
            id: &DefinitionId,
            version: u32,
            bytes: &[u8],
        ) -> Result<A::Transaction, DecodeError> {
            let decode = declared(&self.decoders, id, version)?;

            decode(id, version, bytes)?.ok_or_else(|| DecodeError::Malformed {
                id: id.clone(),
                version,
            })
        }
    }

    impl<A: Application, W> Default for Dispatch<A, W> {
        fn default() -> Self {
            Self {
                transactions: BTreeMap::new(),
                decoders: BTreeMap::new(),
                records: BTreeMap::new(),
            }
        }
    }

    fn declared<F: Copy>(
        table: &BTreeMap<(DefinitionId, u32), F>,
        id: &DefinitionId,
        version: u32,
    ) -> Result<F, DecodeError> {
        if let Some(found) = table.get(&(id.clone(), version)) {
            return Ok(*found);
        }
        let known = table
            .range((id.clone(), u32::MIN)..=(id.clone(), u32::MAX))
            .next()
            .is_some();

        Err(if known {
            DecodeError::UndeclaredVersion {
                id: id.clone(),
                version,
            }
        } else {
            DecodeError::UnknownDefinition { id: id.clone() }
        })
    }
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::convert::Infallible;
// Third Party Imports

// Local Imports
use crate::Application;
use crate::state::envelope::DefinitionId;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub(crate) type Inverting<A> = fn(
    &<A as Application>::Transaction,
    &<A as Application>::State,
) -> Option<<A as Application>::Transaction>;

pub(crate) type TransactionError<A> =
    <<A as Application>::Transaction as Transaction<<A as Application>::State>>::Error;

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
    message = "`{Self}` declares no inverse",
    note = "a change answers the undo question where it is defined: `inverse = …` names the \
            change that takes it back, `inverse = Self` says it is its own, and `irreversible` \
            says it cannot be taken back — an irreversible change carries no invert body"
)]
pub trait Undoable {
    type Inverse;
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` declares an inverse but never describes it",
    label = "declared without a body",
    note = "write the body beside the change: `impl harmos::state::Invert<{S}> for {Self} {{ fn \
            invert(&self, before: &{S}) -> … }}`"
)]
pub trait Invert<S>: Undoable + Transaction<S> {
    fn invert(&self, before: &S) -> Self::Inverse;
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
    fn restore(
        _: &DefinitionId,
        _: u32,
        _: &serde_json::Value,
    ) -> Result<Option<Self>, serde_json::Error> {
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
