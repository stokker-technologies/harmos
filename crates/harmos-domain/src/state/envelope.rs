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

pub use crate::DefinitionId;

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::entry::{Metadata, Position};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemFact {
    pub source: FactSource,
    pub name: String,
    pub version: u32,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope<P> {
    pub position: Position,
    pub kind: Kind,
    pub id: DefinitionId,
    pub version: u32,
    pub metadata: Metadata,
    pub payload: P,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Kind {
    Transaction,
    Record,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FactSource {
    Work(String),
    Guest(String),
    Sidecar(String),
}

/* === Traits === */

/* === Implementations === */

impl SystemFact {
    pub fn identify(&self) -> (DefinitionId, u32) {
        let id = match &self.source {
            FactSource::Work(service) => format!("harmos:work/{service}/{}", self.name),
            FactSource::Guest(guest) => format!("harmos:guest/{guest}/{}", self.name),
            FactSource::Sidecar(owner) => format!("harmos:sidecar/{owner}/{}", self.name),
        };

        (DefinitionId::new(id), self.version)
    }

    pub(crate) fn validate_admission(&self) -> bool {
        if !matches!(self.source, FactSource::Sidecar(_)) {
            return true;
        }
        system_identity(&self.identify().0).is_ok_and(|value| value == Some((self.source.clone(), self.name.clone())))
    }

    pub fn restore<'de, D>(id: &DefinitionId, version: u32, payload: D) -> Result<Option<Self>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let Some((source, name)) = system_identity(id).map_err(serde::de::Error::custom)? else {
            return Ok(None);
        };
        let payload = Vec::<u8>::deserialize(payload)?;

        Ok(Some(Self { source, name, version, payload }))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn system_identity(id: &DefinitionId) -> Result<Option<(FactSource, String)>, &'static str> {
    if let Some(namespaced) = id.as_str().strip_prefix("harmos:work/") {
        let (service, name) = namespaced.split_once('/').ok_or("a work fact id must contain its local fact name")?;
        if service.is_empty() || name.is_empty() {
            return Err("a work fact id must name both service and fact");
        }
        return Ok(Some((FactSource::Work(service.to_owned()), name.to_owned())));
    }

    if let Some(namespaced) = id.as_str().strip_prefix("harmos:guest/") {
        let (guest, name) = namespaced.split_once('/').ok_or("a guest fact id must contain its local fact name")?;
        if guest.is_empty() || name.is_empty() {
            return Err("a guest fact id must name both guest and fact");
        }
        return Ok(Some((FactSource::Guest(guest.to_owned()), name.to_owned())));
    }

    if let Some(namespaced) = id.as_str().strip_prefix("harmos:sidecar/") {
        let (owner, name) = namespaced.split_once('/').ok_or("a sidecar fact id must contain its local fact name")?;
        if !crate::identity::segment(owner) || name.is_empty() {
            return Err("a sidecar fact must name a portable artifact owner and nonempty fact");
        }
        return Ok(Some((FactSource::Sidecar(owner.to_owned()), name.to_owned())));
    }
    Ok(None)
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_owner_parity() {
        let cases: Vec<(String, bool)> = serde_json::from_str(include_str!("../../tests/fixtures/owner-identities.json")).unwrap();
        for (owner, valid) in cases {
            let fact = SystemFact {
                source: FactSource::Sidecar(owner.clone()),
                name: "result/sample".to_owned(),
                version: 1,
                payload: vec![0, 255, 17],
            };
            assert_eq!(crate::state::Prepared::system(&fact).is_ok(), valid, "admission {owner:?}");
            let payload = serde_json::json!(fact.payload);
            let restored = SystemFact::restore(&fact.identify().0, 1, &payload);
            // A Slash Separates The Owner From The Fact Name; Only Exact Round Trips Preserve The Source.
            assert_eq!(restored.is_ok_and(|value| value == Some(fact)), valid, "restore {owner:?}");
        }
    }
}
