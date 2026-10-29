/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::envelope::{DefinitionId, FactSource, SystemFact};
use crate::Identified;

pub type PreparedMembers = Vec<(Option<SystemFact>, Prepared)>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prepared {
    pub id: DefinitionId,
    pub version: u32,
    pub payload: serde_json::Value,
}

#[derive(Default)]
pub struct Records {
    members: Vec<(Option<SystemFact>, Prepared)>,
    bytes: usize,
    refusal: Option<PreparationError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PreparationError {
    #[error("required journal capture failed")]
    CaptureFailed,

    #[error("a commit member could not be encoded")]
    Encoding,

    #[error("the commit exceeds its record or payload limit")]
    Limit,

    #[error("an emitted record is not a current member of the frozen runtime catalog")]
    Membership,

    #[error("a record has an invalid or reserved source identity")]
    Identity,
}

pub trait Record: Identified + Serialize {}

pub trait RecordProjection<R> {
    fn project(&self) -> Option<&R>;
}

pub trait CatalogCodec {
    fn identify(&self) -> (DefinitionId, u32);
    fn encode<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>;
}

pub trait RecordCatalog: CatalogCodec + Sized {
    fn restore(id: &DefinitionId, version: u32, payload: &serde_json::Value) -> Result<Option<Self>, serde_json::Error>;
}

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
    fn restore(id: &DefinitionId, version: u32, payload: &serde_json::Value) -> Result<Option<Self>, serde_json::Error> {
        if (id, version) != (&T::definition().0, T::definition().1) {
            return Ok(None);
        }
        T::deserialize(payload).map(Some)
    }
}

impl RecordCatalog for () {
    fn restore(_: &DefinitionId, _: u32, _: &serde_json::Value) -> Result<Option<Self>, serde_json::Error> {
        Ok(None)
    }
}

impl Prepared {
    pub fn encode(value: &impl CatalogCodec) -> Result<Self, PreparationError> {
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
                if self.bytes.len().checked_add(bytes.len()).is_none_or(|n| n > Records::MAX_BYTES) {
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
        let mut writer = Bounded { bytes: Vec::new(), overflow: false };
        let result = value.encode(&mut serde_json::Serializer::new(&mut writer));
        if writer.overflow {
            return Err(PreparationError::Limit);
        }
        result.map_err(|_| PreparationError::Encoding)?;
        let payload = serde_json::from_slice(&writer.bytes).map_err(|_| PreparationError::Encoding)?;
        let prepared = Self { id, version, payload };
        if prepared.bytes()? > Records::MAX_BYTES {
            return Err(PreparationError::Limit);
        }
        Ok(prepared)
    }

    pub fn typed_record(value: &impl CatalogCodec) -> Result<Self, PreparationError> {
        let prepared = Self::encode(value)?;
        if ["harmos:work/", "harmos:guest/", "harmos:sidecar/"].iter().any(|prefix| prepared.id.as_str().starts_with(prefix)) {
            return Err(PreparationError::Identity);
        }
        Ok(prepared)
    }

    pub fn system(fact: &SystemFact) -> Result<Self, PreparationError> {
        let owner = match &fact.source {
            FactSource::Work(owner) | FactSource::Guest(owner) | FactSource::Sidecar(owner) => owner,
        };
        if owner.len().checked_add(fact.name.len()).and_then(|n| n.checked_add(16)).is_none_or(|n| n > Records::MAX_BYTES) {
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

    pub fn bytes(&self) -> Result<usize, PreparationError> {
        if self.id.as_str().len() > Records::MAX_BYTES {
            return Err(PreparationError::Limit);
        }
        let payload = serde_json::to_vec(&self.payload).map_err(|_| PreparationError::Encoding)?.len();
        let identity = serde_json::to_vec(&self.id).map_err(|_| PreparationError::Encoding)?.len();
        payload.checked_add(identity).and_then(|n| n.checked_add(4)).ok_or(PreparationError::Limit)
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
        self.bytes = self.bytes.checked_add(bytes).filter(|n| *n <= Self::MAX_BYTES).ok_or(PreparationError::Limit)?;
        Ok(())
    }

    pub fn collect<S, T: super::Transaction<S> + CatalogCodec>(
        transaction: &T, state: &S,
    ) -> Result<(Prepared, PreparedMembers), PreparationError> {
        let prepared = Prepared::encode(transaction)?;
        let mut out = Self::default();
        out.reserve(prepared.bytes()?)?;
        transaction.records(state, &mut out);
        if let Some(refusal) = out.refusal {
            return Err(refusal);
        }
        Ok((prepared, out.members))
    }
}
