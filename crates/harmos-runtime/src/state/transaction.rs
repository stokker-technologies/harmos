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

pub use harmos_domain::state::{Catalogued, DecodeError, Invertible, Mutual, Never, Paired, Transaction, TransactionCatalog};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::Application;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub(crate) type Inverting<A> = harmos_domain::state::StateInverse<<A as Application>::State, <A as Application>::Transaction>;
pub(crate) type TransactionError<A> = <<A as Application>::Transaction as Transaction<<A as Application>::State>>::Error;

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

pub(crate) mod dispatch {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports
    use std::collections::BTreeMap;

    // Third Party Imports

    // Local Imports
    use crate::Application;
    use crate::state::envelope::{DefinitionId, Envelope, Kind};
    use crate::state::record::Upgrade;
    use crate::state::transaction::DecodeError;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    pub(crate) type Restore<A, W> = std::sync::Arc<dyn Fn(W, &mut <A as Application>::State) -> Result<(), DecodeError> + Send + Sync>;
    pub(crate) type Decode<A> = harmos_domain::state::StateDecoder<<A as Application>::Transaction>;

    /* === Structs === */

    pub(crate) struct Dispatch<A: Application, W> {
        transactions: BTreeMap<(DefinitionId, u32), Restore<A, W>>,
        decoders: BTreeMap<(DefinitionId, u32), Decode<A>>,
        records: BTreeMap<(DefinitionId, u32), Upgrade<A, W>>,
        pub(crate) states: harmos_domain::state::StateCatalog<A::State, A::Transaction>,
        pub(crate) record_registry: harmos_domain::state::RecordRegistry<A::Record>,
    }

    /* === Enums === */

    #[derive(Debug)]
    pub(crate) enum Restored<A: Application> {
        Applied,
        Recorded(A::Record),
    }

    /* === Traits === */

    /* === Implementations === */

    impl<A: Application, W> Dispatch<A, W> {
        pub(crate) fn definitions(&self) -> Vec<(Kind, DefinitionId, u32)> {
            self.transactions
                .keys()
                .map(|(id, version)| (Kind::Transaction, id.clone(), *version))
                .chain(self.records.keys().map(|(id, version)| (Kind::Record, id.clone(), *version)))
                .collect()
        }

        pub(crate) fn validate(&self, kind: Kind, id: &DefinitionId, version: u32) -> Result<(), DecodeError> {
            match kind {
                Kind::Transaction => declared_cloned(&self.transactions, id, version).map(|_| ()),
                Kind::Record => declared(&self.records, id, version).map(|_| ()),
            }
        }

        pub(crate) fn set_transaction(&mut self, id: DefinitionId, version: u32, restore: Restore<A, W>, decode: Decode<A>) {
            self.transactions.insert((id.clone(), version), restore);
            self.decoders.insert((id, version), decode);
        }

        pub(crate) fn set_record(&mut self, id: DefinitionId, version: u32, upgrade: Upgrade<A, W>) {
            self.records.insert((id, version), upgrade);
        }

        pub(crate) fn restore(&self, envelope: Envelope<W>, state: &mut A::State) -> Result<Restored<A>, DecodeError> {
            let Envelope { kind, id, version, payload, .. } = envelope;

            match kind {
                Kind::Transaction => {
                    let restore = declared_cloned(&self.transactions, &id, version)?;
                    restore(payload, state)?;
                    Ok(Restored::Applied)
                }
                Kind::Record => {
                    let upgrade = declared(&self.records, &id, version)?;
                    Ok(Restored::Recorded(upgrade(payload)?))
                }
            }
        }

        pub(crate) fn has_record(&self, id: &DefinitionId, version: u32) -> Result<bool, DecodeError> {
            match declared(&self.records, id, version) {
                Ok(_) => Ok(true),
                Err(DecodeError::UnknownDefinition { .. }) => Ok(false),
                Err(error) => Err(error),
            }
        }

        pub(crate) fn decode_transaction(&self, id: &DefinitionId, version: u32, bytes: &[u8]) -> Result<A::Transaction, DecodeError> {
            let decode = declared_cloned(&self.decoders, id, version)?;

            decode(id, version, bytes)?.ok_or_else(|| DecodeError::Malformed { id: id.clone(), version })
        }
    }

    impl<A: Application, W> Default for Dispatch<A, W> {
        fn default() -> Self {
            Self {
                transactions: BTreeMap::new(),
                decoders: BTreeMap::new(),
                records: BTreeMap::new(),
                states: harmos_domain::state::StateCatalog::new(),
                record_registry: harmos_domain::state::RecordRegistry::new(),
            }
        }
    }

    /* === Public Functions === */

    /* === Private Functions === */

    fn declared_cloned<F: Clone>(table: &BTreeMap<(DefinitionId, u32), F>, id: &DefinitionId, version: u32) -> Result<F, DecodeError> {
        if let Some(found) = table.get(&(id.clone(), version)) {
            return Ok(found.clone());
        }
        let known = table.range((id.clone(), u32::MIN)..=(id.clone(), u32::MAX)).next().is_some();
        Err(if known {
            DecodeError::UndeclaredVersion { id: id.clone(), version }
        } else {
            DecodeError::UnknownDefinition { id: id.clone() }
        })
    }

    fn declared<F: Copy>(table: &BTreeMap<(DefinitionId, u32), F>, id: &DefinitionId, version: u32) -> Result<F, DecodeError> {
        if let Some(found) = table.get(&(id.clone(), version)) {
            return Ok(*found);
        }
        let known = table.range((id.clone(), u32::MIN)..=(id.clone(), u32::MAX)).next().is_some();

        Err(if known {
            DecodeError::UndeclaredVersion { id: id.clone(), version }
        } else {
            DecodeError::UnknownDefinition { id: id.clone() }
        })
    }

    /* === Tests === */
}

/* === Tests === */
