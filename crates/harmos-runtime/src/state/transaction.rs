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
pub use harmos_domain::state::{
    Catalogued, DecodeError, Invert, Mutual, Never, Transaction, TransactionCatalog, Undoable,
};

/* === Imports === */
use crate::Application;

/* === Type Aliases === */
pub(crate) type Inverting<A> = fn(
    &<A as Application>::Transaction,
    &<A as Application>::State,
) -> Option<<A as Application>::Transaction>;

pub(crate) type TransactionError<A> =
    <<A as Application>::Transaction as Transaction<<A as Application>::State>>::Error;
