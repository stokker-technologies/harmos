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
    version: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    schema: Option<crate::schema::graph::Schema>,

    pub payload: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginFact {
    schema: crate::schema::graph::Schema,
    payload: Vec<u8>,
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
    pub fn new(source: FactSource, name: String, payload: Vec<u8>) -> Self {
        let schema = crate::schema::graph::Schema::from_declaration(
            <Vec<u8> as crate::schema::Described>::DEFINITION,
            &crate::config::SchemaSettings::default(),
        )
        .expect("opaque fact schema");
        let version = crate::schema::identity::revision(&schema).expect("opaque fact revision");
        let schema = matches!(source, FactSource::Guest(_) | FactSource::Sidecar(_)).then_some(schema);
        Self { source, name, version, schema, payload }
    }

    pub fn encode<T: crate::schema::Described + serde::Serialize>(
        source: FactSource, name: String, value: &T,
    ) -> Result<Self, serde_json::Error> {
        let schema = crate::schema::graph::Schema::from_declaration(T::DEFINITION, &crate::config::SchemaSettings::default())
            .expect("valid system fact schema");
        let version = crate::schema::identity::revision(&schema).expect("valid system fact revision");
        let payload = serde_json::to_vec(value)?;
        let schema = matches!(source, FactSource::Guest(_) | FactSource::Sidecar(_)).then_some(schema);
        Ok(Self { source, name, version, schema, payload })
    }

    /// The Written Plugin Definition, Independent Of The Currently Installed Plugin.
    pub fn schema(&self) -> Option<&crate::schema::graph::Schema> {
        self.schema.as_ref()
    }

    pub fn identify(&self) -> (DefinitionId, u32) {
        let id = match &self.source {
            FactSource::Work(service) => format!("harmos:work/{service}/{}", self.name),
            FactSource::Guest(guest) => format!("harmos:guest/{guest}/{}", self.name),
            FactSource::Sidecar(owner) => format!("harmos:sidecar/{owner}/{}", self.name),
        };

        (DefinitionId::new(id), self.version)
    }

    pub(crate) fn validate_admission(&self) -> bool {
        let plugin = matches!(self.source, FactSource::Guest(_) | FactSource::Sidecar(_));
        if plugin != self.schema.is_some() {
            return false;
        }
        if let Some(schema) = &self.schema
            && !crate::schema::identity::revision(schema).is_ok_and(|revision| revision == self.version)
        {
            return false;
        }
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
        let (schema, payload) = if matches!(source, FactSource::Guest(_) | FactSource::Sidecar(_)) {
            let fact = PluginFact::deserialize(payload).map_err(|error| {
                serde::de::Error::custom(format!(
                    "plugin fact `{id}` has no readable stored definition: {error}; retain history and select a fresh storage root"
                ))
            })?;
            let revision = crate::schema::identity::revision(&fact.schema).map_err(serde::de::Error::custom)?;
            if revision != version {
                return Err(serde::de::Error::custom("plugin fact definition differs from its stored revision"));
            }
            (Some(fact.schema), fact.payload)
        } else {
            (None, Vec::<u8>::deserialize(payload)?)
        };

        Ok(Some(Self { source, name, version, schema, payload }))
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
    fn plugin_fact_definitions_survive_without_current_decoders() {
        for source in [FactSource::Guest("probe".into()), FactSource::Sidecar("probe".into())] {
            let old = SystemFact::encode(source.clone(), "reading".into(), &41u64).unwrap();
            let current = SystemFact::encode(source, "reading".into(), &String::from("changed")).unwrap();
            assert_ne!(old.identify().1, current.identify().1);
            let prepared = crate::state::Prepared::system(&old).unwrap();
            let restored = SystemFact::restore(&prepared.id, prepared.version, &prepared.payload).unwrap().unwrap();
            assert_eq!(restored.schema(), old.schema());
            assert_eq!(serde_json::from_slice::<u64>(&restored.payload).unwrap(), 41);
            assert!(SystemFact::restore(&prepared.id, current.identify().1, &prepared.payload).is_err());
            assert!(SystemFact::restore(&prepared.id, prepared.version, &serde_json::json!(old.payload)).is_err());
            let mut malformed = serde_json::to_value(&old).unwrap();
            malformed["version"] = serde_json::json!(current.identify().1);
            let malformed = serde_json::from_value(malformed).unwrap();
            assert!(crate::state::Prepared::system(&malformed).is_err());
            let mut changed_source = old.clone();
            changed_source.source = FactSource::Work("host".into());
            assert!(crate::state::Prepared::system(&changed_source).is_err());
        }
    }

    #[test]
    fn sidecar_owner_parity() {
        let cases: Vec<(String, bool)> = serde_json::from_str(include_str!("../../tests/fixtures/owner-identities.json")).unwrap();
        for (owner, valid) in cases {
            let fact = SystemFact::new(FactSource::Sidecar(owner.clone()), "result/sample".to_owned(), vec![0, 255, 17]);
            assert_eq!(crate::state::Prepared::system(&fact).is_ok(), valid, "admission {owner:?}");
            let payload = serde_json::json!({ "schema": fact.schema(), "payload": fact.payload });
            let restored = SystemFact::restore(&fact.identify().0, fact.identify().1, &payload);
            // A Slash Separates The Owner From The Fact Name; Only Exact Round Trips Preserve The Source.
            assert_eq!(restored.is_ok_and(|value| value == Some(fact)), valid, "restore {owner:?}");
        }
    }
}
