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
use std::sync::Arc;

// Third Party Imports
use serde::{Deserializer, Serializer};

// Local Imports
use crate::app::SchematicEditor;
use crate::records::Records;
use crate::transactions::Transactions;
use harmos::{DefinitionId, state::Kind, state::Recorded, storage::Payload};
use jsonl_adapter::Wire;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Wire for SchematicEditor {
    fn identify(value: &Recorded<Self>) -> (DefinitionId, u32) {
        match value {
            Recorded::Transaction(change) => change.identify(),
            Recorded::Record(fact) => fact.identify(),
            Recorded::System(_) => unreachable!("system facts are encoded by the adapter"),
        }
    }

    fn encode<S: Serializer>(value: &Recorded<Self>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Recorded::Transaction(change) => change.encode_with(serializer),
            Recorded::Record(fact) => fact.encode_with(serializer),
            Recorded::System(_) => unreachable!("system facts are encoded by the adapter"),
        }
    }

    fn decode<'de, D: Deserializer<'de> + Copy>(
        kind: Kind,
        id: &DefinitionId,
        version: u32,
        payload: D,
    ) -> Result<Option<Payload<Self>>, D::Error> {
        Ok(match kind {
            Kind::Transaction => {
                Transactions::restore(id, version, payload)?.map(|(change, apply)| Payload {
                    prepared: None,
                    id: id.clone(),
                    version,
                    value: Recorded::Transaction(Arc::new(change)),
                    apply,
                })
            }
            Kind::Record => Records::restore(id, version, payload)?.map(|fact| Payload {
                prepared: None,
                id: id.clone(),
                version,
                value: Recorded::Record(fact),
                apply: None,
            }),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
