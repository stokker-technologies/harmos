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
pub use harmos_domain::DefinitionId;

/* === Imports === */
// Standard Library Imports
use std::any::Any;
use std::sync::Arc;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::entry::{Metadata, Position};
use crate::Application;

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

pub enum Recorded<A: Application> {
    Transaction(Arc<A::Transaction>),
    Record(A::Record),
    System(SystemFact),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FactSource {
    Work(String),
    Guest(String),
    Sidecar(String),
}

/* === Traits === */

/* === Implementations === */
impl<A: Application> Clone for Recorded<A>
where
    A::Record: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::Transaction(value) => Self::Transaction(Arc::clone(value)),
            Self::Record(value) => Self::Record(value.clone()),
            Self::System(value) => Self::System(value.clone()),
        }
    }
}

impl<A: Application> Recorded<A> {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Transaction(_) => Kind::Transaction,
            Self::Record(_) => Kind::Record,
            Self::System(_) => Kind::Record,
        }
    }

    pub(crate) fn select<X: Clone + 'static>(&self) -> Option<X> {
        if let Some(value) = (self as &dyn Any).downcast_ref::<X>() {
            return Some(value.clone());
        }
        match self {
            Self::Transaction(value) => (&**value as &dyn Any).downcast_ref::<X>().cloned(),
            Self::Record(value) => (value as &dyn Any).downcast_ref::<X>().cloned(),
            Self::System(value) => (value as &dyn Any).downcast_ref::<X>().cloned(),
        }
    }
}

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
        system_identity(&self.identify().0)
            .is_ok_and(|value| value == Some((self.source.clone(), self.name.clone())))
    }

    pub fn restore<'de, D>(
        id: &DefinitionId,
        version: u32,
        payload: D,
    ) -> Result<Option<Self>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let Some((source, name)) = system_identity(id).map_err(serde::de::Error::custom)? else {
            return Ok(None);
        };
        let payload = Vec::<u8>::deserialize(payload)?;

        Ok(Some(Self {
            source,
            name,
            version,
            payload,
        }))
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn system_identity(id: &DefinitionId) -> Result<Option<(FactSource, String)>, &'static str> {
    if let Some(namespaced) = id.as_str().strip_prefix("harmos:work/") {
        let (service, name) = namespaced
            .split_once('/')
            .ok_or("a work fact id must contain its local fact name")?;
        if service.is_empty() || name.is_empty() {
            return Err("a work fact id must name both service and fact");
        }
        return Ok(Some((
            FactSource::Work(service.to_owned()),
            name.to_owned(),
        )));
    }

    if let Some(namespaced) = id.as_str().strip_prefix("harmos:guest/") {
        let (guest, name) = namespaced
            .split_once('/')
            .ok_or("a guest fact id must contain its local fact name")?;
        if guest.is_empty() || name.is_empty() {
            return Err("a guest fact id must name both guest and fact");
        }
        return Ok(Some((FactSource::Guest(guest.to_owned()), name.to_owned())));
    }

    if let Some(namespaced) = id.as_str().strip_prefix("harmos:sidecar/") {
        let (owner, name) = namespaced
            .split_once('/')
            .ok_or("a sidecar fact id must contain its local fact name")?;
        if !owner.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
            || !owner
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
            || name.is_empty()
        {
            return Err("a sidecar fact must name a portable artifact owner and nonempty fact");
        }
        return Ok(Some((
            FactSource::Sidecar(owner.to_owned()),
            name.to_owned(),
        )));
    }
    Ok(None)
}

/* === Tests === */
