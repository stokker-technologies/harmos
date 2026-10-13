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
use std::num::NonZeroU64;
// Third Party Imports

// Local Imports
use harmos::storage::{ObjectRetention, StoredObject};
use harmos::{DefinitionId, Identified, MemorySize, Message, MessageError, schema};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// Wire defaults are valid protobuf; the domain value requires validated construction.
#[harmos::message]
struct Wire {
    id: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Calibration {
    id: NonZeroU64,
}

/* === Enums === */
#[harmos::catalog]
pub enum Objects {
    Calibration(Calibration),
}

/* === Traits === */

/* === Implementations === */
impl Calibration {
    pub fn new(id: u64) -> Result<Self, MessageError> {
        let id = NonZeroU64::new(id)
            .ok_or_else(|| MessageError::new("calibration ID must be nonzero"))?;

        Ok(Self { id })
    }
}

impl Message for Calibration {
    fn encode_to_vec(&self) -> Vec<u8> {
        Message::encode_to_vec(&Wire { id: self.id.get() })
    }

    fn encoded_len(&self) -> usize {
        Message::encoded_len(&Wire { id: self.id.get() })
    }

    fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
        let wire = <Wire as Message>::decode(bytes)?;

        Self::new(wire.id)
    }
}

impl schema::Described for Calibration {
    const NAME: &'static str = concat!(module_path!(), "::Calibration");
    const MARKER: &'static [u8] = &schema::declaration::encode::<
        { schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
    const FIELDS: &'static [schema::Field] = <Wire as schema::Described>::FIELDS;
}

impl MemorySize for Calibration {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl Identified for Calibration {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("example.calibration"), 1)
    }
}

impl StoredObject for Calibration {
    type Key = NonZeroU64;
    const RETENTION: ObjectRetention = ObjectRetention::Latest;

    fn key(&self) -> &Self::Key {
        &self.id
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
