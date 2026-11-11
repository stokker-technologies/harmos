/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Own Native Record Entries and Typed Restoration

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::any::{Any, TypeId};
use std::sync::Arc;

// Third Party Imports
use serde::{Serialize, de::DeserializeOwned};

// Local Imports
use super::{DefinitionId, Kind, PreparationError, Prepared, RecordEntry};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Project<C> = dyn for<'a> Fn(&'a C) -> Option<&'a dyn Any> + Send + Sync;
type Prepare<C> = dyn Fn(&C) -> Result<Option<Prepared>, PreparationError> + Send + Sync;
type Restore<C> = dyn Fn(u32, &serde_json::Value) -> Result<Option<C>, serde_json::Error> + Send + Sync;

/* === Structs === */

pub struct RecordRegistry<C> {
    entries: Vec<Arc<Entry<C>>>,
}

struct Entry<C> {
    id: DefinitionId,
    version: u32,
    type_id: TypeId,
    project: Arc<Project<C>>,
    prepare: Arc<Prepare<C>>,
    restore: Arc<Restore<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C> Clone for RecordRegistry<C> {
    fn clone(&self) -> Self {
        Self { entries: self.entries.clone() }
    }
}

impl<C> Default for RecordRegistry<C> {
    fn default() -> Self {
        Self { entries: Vec::new() }
    }
}

impl<C: 'static> RecordRegistry<C> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entry<T>(mut self, entry: RecordEntry<T>, wrap: fn(T) -> C, project: for<'a> fn(&'a C) -> Option<&'a T>) -> Self
    where
        T: crate::schema::Described + Serialize + DeserializeOwned + 'static,
    {
        let (id, version) = entry.definition();
        self.entries.push(Arc::new(Entry {
            id,
            version,
            type_id: TypeId::of::<T>(),
            project: Arc::new(move |value| project(value).map(|value| value as &dyn Any)),
            prepare: Arc::new(move |value| project(value).map(|value| entry.prepare(value)).transpose()),
            restore: Arc::new(move |version, payload| entry.restore(version, payload).map(|value| value.map(wrap))),
        }));
        self
    }

    pub fn extend(mut self, registry: Self) -> Self {
        self.entries.extend(registry.entries);
        self
    }

    pub fn validate(&self) -> crate::Result<()> {
        let mut seen = std::collections::BTreeSet::new();
        for entry in &self.entries {
            if !seen.insert((entry.id.clone(), entry.version)) {
                return Err(
                    crate::MessageError::new(format!("record `{}` revision {} is registered twice", entry.id, entry.version)).into()
                );
            }
        }
        Ok(())
    }

    pub fn build(self) -> crate::Result<Self> {
        self.validate()?;
        Ok(self)
    }

    pub fn definitions(&self) -> Vec<(Kind, DefinitionId, u32)> {
        self.entries.iter().map(|entry| (Kind::Record, entry.id.clone(), entry.version)).collect()
    }

    pub fn definition<T: 'static>(&self) -> Option<(DefinitionId, u32)> {
        let mut entries = self.entries.iter().filter(|entry| entry.type_id == TypeId::of::<T>());
        let entry = entries.next()?;
        entries.next().is_none().then(|| (entry.id.clone(), entry.version))
    }

    pub fn project<'a, T: 'static>(&self, value: &'a C) -> Option<&'a T> {
        self.entries.iter().filter(|entry| entry.type_id == TypeId::of::<T>()).find_map(|entry| (entry.project)(value)?.downcast_ref())
    }

    pub fn prepare(&self, value: &C) -> Result<Option<Prepared>, PreparationError> {
        let mut prepared = None;
        for entry in &self.entries {
            if let Some(value) = (entry.prepare)(value)? {
                if prepared.is_some() {
                    return Err(PreparationError::Membership);
                }
                prepared = Some(value);
            }
        }
        Ok(prepared)
    }

    pub fn restore<'de, D: serde::Deserializer<'de>>(&self, id: &DefinitionId, version: u32, payload: D) -> Result<Option<C>, D::Error> {
        for entry in &self.entries {
            if &entry.id == id && entry.version == version {
                let payload = <serde_json::Value as serde::Deserialize>::deserialize(payload)?;
                return (entry.restore)(version, &payload).map_err(serde::de::Error::custom);
            }
        }
        Ok(None)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(crate::Described, Serialize, serde::Deserialize, PartialEq, Debug)]
    #[harmos_path(crate)]
    struct Noted {
        value: u64,
    }

    enum Event {
        Noted(Noted),
        Other,
    }

    fn registry() -> RecordRegistry<Event> {
        RecordRegistry::new().entry(RecordEntry::new("ledger.noted"), Event::Noted, |value| match value {
            Event::Noted(value) => Some(value),
            Event::Other => None,
        })
    }

    #[test]
    fn registry_prepares_projects_and_restores() {
        let registry = registry().build().unwrap();
        let value = Event::Noted(Noted { value: 37 });
        let prepared = registry.prepare(&value).unwrap().unwrap();
        let restored = registry.restore(&prepared.id, prepared.version, &prepared.payload).unwrap().unwrap();
        assert_eq!(registry.project::<Noted>(&restored), Some(&Noted { value: 37 }));
        assert!(registry.prepare(&Event::Other).unwrap().is_none());
        assert!(registry.restore(&prepared.id, prepared.version.wrapping_add(1), &prepared.payload).unwrap().is_none());
        assert!(RecordRegistry::<Event>::new().project::<Noted>(&value).is_none());
        assert!(registry.clone().extend(registry).build().is_err());
    }
}
