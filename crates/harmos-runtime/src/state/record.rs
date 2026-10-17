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
pub use crate::error::PreparationError;

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize, de::DeserializeOwned};
// Local Imports
use super::envelope::{DefinitionId, FactSource, Recorded, SystemFact};
use super::transaction::DecodeError;
use crate::Application;
use crate::Identified;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub(crate) type Upgrade<A, W> = fn(W) -> Result<<A as Application>::Record, DecodeError>;

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prepared {
    pub id: DefinitionId,
    pub version: u32,
    pub payload: serde_json::Value,
}

pub(crate) struct PreparedCommit<A: Application> {
    pub(crate) transaction: Prepared,
    pub(crate) records: Vec<(Recorded<A>, Prepared)>,
}

#[derive(Default)]
pub struct Records {
    members: Vec<(Option<SystemFact>, Prepared)>,
    bytes: usize,
    refusal: Option<PreparationError>,
}

/* === Enums === */

/* === Traits === */
pub trait Record: Identified + Serialize {}

pub trait RecordProjection<R> {
    fn project(&self) -> Option<&R>;
}

pub trait CatalogCodec {
    fn identify(&self) -> (DefinitionId, u32);
    fn encode<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>;
}

pub trait RecordCatalog: CatalogCodec + Sized {
    fn restore(
        id: &DefinitionId,
        version: u32,
        payload: &serde_json::Value,
    ) -> Result<Option<Self>, serde_json::Error>;
}

/* === Implementations === */
impl<R> RecordProjection<R> for R {
    fn project(&self) -> Option<&R> {
        Some(self)
    }
}

impl<T: Identified + Serialize> CatalogCodec for T {
    fn identify(&self) -> (DefinitionId, u32) {
        T::definition()
    }

    fn encode<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.serialize(serializer)
    }
}

impl<T: Record + DeserializeOwned> RecordCatalog for T {
    fn restore(
        id: &DefinitionId,
        version: u32,
        payload: &serde_json::Value,
    ) -> Result<Option<Self>, serde_json::Error> {
        if (id, version) != (&T::definition().0, T::definition().1) {
            return Ok(None);
        }
        T::deserialize(payload).map(Some)
    }
}

impl RecordCatalog for () {
    fn restore(
        _: &DefinitionId,
        _: u32,
        _: &serde_json::Value,
    ) -> Result<Option<Self>, serde_json::Error> {
        Ok(None)
    }
}

impl Prepared {
    pub(crate) fn encode(value: &impl CatalogCodec) -> Result<Self, PreparationError> {
        let (id, version) = value.identify();
        if id.as_str().len() > Records::MAX_BYTES {
            return Err(PreparationError::Limit);
        }
        struct Bounded {
            bytes: Vec<u8>,
            overflow: bool,
        }
        impl std::io::Write for Bounded {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self
                    .bytes
                    .len()
                    .checked_add(bytes.len())
                    .is_none_or(|n| n > Records::MAX_BYTES)
                {
                    self.overflow = true;
                    return Err(std::io::Error::other("commit payload limit"));
                }
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut writer = Bounded {
            bytes: Vec::new(),
            overflow: false,
        };
        let result = value.encode(&mut serde_json::Serializer::new(&mut writer));
        if writer.overflow {
            return Err(PreparationError::Limit);
        }
        result.map_err(|_| PreparationError::Encoding)?;
        let payload =
            serde_json::from_slice(&writer.bytes).map_err(|_| PreparationError::Encoding)?;
        let prepared = Self {
            id,
            version,
            payload,
        };
        if prepared.bytes()? > Records::MAX_BYTES {
            return Err(PreparationError::Limit);
        }
        Ok(prepared)
    }

    pub(crate) fn typed_record(value: &impl CatalogCodec) -> Result<Self, PreparationError> {
        let prepared = Self::encode(value)?;
        if ["harmos:work/", "harmos:guest/", "harmos:sidecar/"]
            .iter()
            .any(|prefix| prepared.id.as_str().starts_with(prefix))
        {
            return Err(PreparationError::Identity);
        }
        Ok(prepared)
    }

    pub(crate) fn system(fact: &SystemFact) -> Result<Self, PreparationError> {
        let owner = match &fact.source {
            FactSource::Work(owner) | FactSource::Guest(owner) | FactSource::Sidecar(owner) => {
                owner
            }
        };
        if owner
            .len()
            .checked_add(fact.name.len())
            .and_then(|n| n.checked_add(16))
            .is_none_or(|n| n > Records::MAX_BYTES)
        {
            return Err(PreparationError::Limit);
        }
        if !fact.validate_admission() {
            return Err(PreparationError::Identity);
        }
        if fact.payload.len() > Records::MAX_BYTES / 4 {
            return Err(PreparationError::Limit);
        }
        let (id, version) = fact.identify();
        let prepared = Self {
            id,
            version,
            payload: serde_json::json!(fact.payload),
        };
        if prepared.bytes()? > Records::MAX_BYTES {
            return Err(PreparationError::Limit);
        }
        Ok(prepared)
    }

    pub(crate) fn bytes(&self) -> Result<usize, PreparationError> {
        if self.id.as_str().len() > Records::MAX_BYTES {
            return Err(PreparationError::Limit);
        }
        let payload = serde_json::to_vec(&self.payload)
            .map_err(|_| PreparationError::Encoding)?
            .len();
        let identity = serde_json::to_vec(&self.id)
            .map_err(|_| PreparationError::Encoding)?
            .len();
        payload
            .checked_add(identity)
            .and_then(|n| n.checked_add(4))
            .ok_or(PreparationError::Limit)
    }
}

impl Records {
    pub const MAX_RECORDS: usize = 256;
    pub const MAX_BYTES: usize = 4 * 1024 * 1024;

    pub fn push<R: Record>(&mut self, record: R) -> Result<(), PreparationError> {
        if let Some(refusal) = self.refusal {
            return Err(refusal);
        }
        let result = (|| {
            if self.members.len() == Self::MAX_RECORDS {
                return Err(PreparationError::Limit);
            }
            let prepared = Prepared::typed_record(&record)?;
            self.reserve(prepared.bytes()?)?;
            self.members.push((None, prepared));
            Ok(())
        })();
        if let Err(refusal) = result {
            self.refusal = Some(refusal);
        }
        result
    }

    pub fn push_system(&mut self, fact: SystemFact) -> Result<(), PreparationError> {
        if let Some(refusal) = self.refusal {
            return Err(refusal);
        }
        let result = (|| {
            if self.members.len() == Self::MAX_RECORDS {
                return Err(PreparationError::Limit);
            }
            let prepared = Prepared::system(&fact)?;
            self.reserve(prepared.bytes()?)?;
            self.members.push((Some(fact), prepared));
            Ok(())
        })();
        if let Err(refusal) = result {
            self.refusal = Some(refusal);
        }
        result
    }

    fn reserve(&mut self, bytes: usize) -> Result<(), PreparationError> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= Self::MAX_BYTES)
            .ok_or(PreparationError::Limit)?;
        Ok(())
    }

    pub(crate) fn prepare<A: Application>(
        transaction: &A::Transaction,
        state: &A::State,
        catalog: &super::dispatch::Dispatch<A, crate::storage::Payload<A>>,
    ) -> Result<PreparedCommit<A>, PreparationError> {
        let prepared = Prepared::encode(transaction)?;
        let mut out = Self::default();
        out.reserve(prepared.bytes()?)?;
        crate::state::Transaction::records(transaction, state, &mut out);
        if let Some(refusal) = out.refusal {
            return Err(refusal);
        }
        let records = out
            .members
            .into_iter()
            .map(|(system, member)| {
                if let Some(fact) = system {
                    return Ok((Recorded::System(fact), member));
                }
                if !matches!(catalog.has_record(&member.id, member.version), Ok(true)) {
                    return Err(PreparationError::Membership);
                }
                let value = A::Record::restore(&member.id, member.version, &member.payload)
                    .map_err(|_| PreparationError::Membership)?
                    .ok_or(PreparationError::Membership)?;
                if value.identify() != (member.id.clone(), member.version) {
                    return Err(PreparationError::Membership);
                }
                Ok((Recorded::Record(value), member))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PreparedCommit {
            transaction: prepared,
            records,
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
