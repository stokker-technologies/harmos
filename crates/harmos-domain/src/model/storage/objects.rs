/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub mod codec;

/* === Re-Exports === */
pub use codec::Key;

/* === Imports === */

// Standard Library Imports
use std::borrow::Cow;
use std::num::{NonZeroU64, NonZeroUsize};

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use crate::Identifier;
use crate::model::storage::keys::invalid;
use crate::model::storage::{ObjectKey, ObjectValue, StorageCursor, StoreHistory};
use crate::schema::declaration::{Field, Kind};
use crate::schema::{Cardinality, Described};
use crate::{MemorySize, Message, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectEntry {
    pub position: StorageCursor,
    pub version: ObjectVersionToken,
    pub value: ObjectValue,
}

#[derive(Clone, Debug)]
pub struct ObjectRevision<T> {
    pub position: StorageCursor,
    pub version: ObjectVersionToken,
    pub value: T,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectPoint {
    pub schema: Identifier,
    pub item: ObjectKey,
    pub version: ObjectVersion,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectVersionToken {
    pub(crate) history: StoreHistory,
    pub(crate) revision: NonZeroU64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectReceipt {
    pub position: StorageCursor,
    pub version: ObjectVersionToken,
}

/* === Enums === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectVersion {
    Latest,
    Exact(ObjectVersionToken),
}

/* === Traits === */

/* === Implementations === */
impl MemorySize for ObjectEntry {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.position.memory_size().saturating_sub(std::mem::size_of::<StorageCursor>()))
            .saturating_add(self.version.memory_size().saturating_sub(std::mem::size_of::<ObjectVersionToken>()))
            .saturating_add(self.value.memory_size().saturating_sub(std::mem::size_of::<ObjectValue>()))
    }
}

impl<T: MemorySize> MemorySize for ObjectRevision<T> {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.position.memory_size().saturating_sub(std::mem::size_of::<StorageCursor>()))
            .saturating_add(self.version.memory_size().saturating_sub(std::mem::size_of::<ObjectVersionToken>()))
            .saturating_add(self.value.memory_size().saturating_sub(std::mem::size_of::<T>()))
    }
}

impl MemorySize for ObjectReceipt {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.position.memory_size() - std::mem::size_of::<StorageCursor>() + self.version.memory_size()
            - std::mem::size_of::<ObjectVersionToken>()
    }
}

impl MemorySize for ObjectVersionToken {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.history.memory_size().saturating_sub(std::mem::size_of::<StoreHistory>()))
    }
}

impl ObjectVersionToken {
    pub fn history(&self) -> &StoreHistory {
        &self.history
    }

    pub fn revision(&self) -> NonZeroU64 {
        self.revision
    }

    pub fn encode_bounded(&self, max_bytes: NonZeroUsize) -> Result<Vec<u8>> {
        if self.encoded_len() > max_bytes.get() {
            return Err(invalid("object version encoding exceeds allowance"));
        }
        Ok(self.encode_to_vec())
    }

    pub fn decode_bounded(bytes: &[u8], max_bytes: NonZeroUsize) -> Result<Self> {
        let required = Self::decoding_memory(bytes).map_err(|_| invalid("invalid object version encoding"))?;
        if required > max_bytes.get() {
            return Err(invalid("object version decoding exceeds allowance"));
        }
        let token = Self::decode(bytes).map_err(|_| invalid("invalid object version encoding"))?;
        if token.memory_size() > max_bytes.get() {
            return Err(invalid("object version decoding exceeds allowance"));
        }
        Ok(token)
    }

    pub fn new(position: StorageCursor) -> Self {
        Self { history: position.history, revision: NonZeroU64::new(position.sequence).expect("admission positions are nonzero") }
    }

    pub fn check_position(&self, position: &StorageCursor) -> Result<()> {
        self.history.check(&position.history)?;
        if self.revision.get() != position.sequence {
            return Err(invalid("object version does not match its admission identity"));
        }
        Ok(())
    }
}

crate::postcard_product!(ObjectVersionToken { history: StoreHistory, revision: NonZeroU64 });

impl Described for ObjectVersionToken {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(ObjectVersionToken));
    const FIELDS: &'static [Field] = &[
        Field { name: Cow::Borrowed("store"), tag: 1, kind: Kind::String, cardinality: Cardinality::Required },
        Field { name: Cow::Borrowed("epoch"), tag: 2, kind: Kind::String, cardinality: Cardinality::Required },
        Field { name: Cow::Borrowed("revision"), tag: 3, kind: Kind::U64, cardinality: Cardinality::Required },
    ];

    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
