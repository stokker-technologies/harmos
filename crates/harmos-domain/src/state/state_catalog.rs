/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Own Transaction Identity, Restoration and Inverse Routing

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::any::{Any, TypeId};
use std::sync::Arc;

// Third Party Imports
use serde::{Serialize, de::DeserializeOwned};

// Local Imports
use super::{CatalogCodec, DecodeError, DefinitionId, Invertible, Kind, PreparationError, Prepared, RecordEntry, Transaction};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type StateInverse<S, C> = Arc<dyn Fn(&C, &S) -> Option<C> + Send + Sync>;
pub type StateDecoder<C> = Arc<dyn Fn(&DefinitionId, u32, &[u8]) -> Result<Option<C>, DecodeError> + Send + Sync>;

type Erased = Box<dyn Any + Send + Sync>;
type Prepare<C> = dyn Fn(&C) -> Result<Option<Prepared>, PreparationError> + Send + Sync;
type Restore<C> = dyn Fn(&serde_json::Value) -> Result<C, serde_json::Error> + Send + Sync;
type Wrap<C> = dyn Fn(Erased) -> Option<C> + Send + Sync;
type Reverse<S, C> = dyn Fn(&C, &S) -> Option<Erased> + Send + Sync;

/* === Structs === */

pub struct TransactionEntry<S, T> {
    record: RecordEntry<T>,
    disposition: Disposition<S, T>,
}

pub struct StateCatalog<S, C> {
    entries: Vec<Arc<Entry<S, C>>>,
}

struct Entry<S, C> {
    id: DefinitionId,
    version: u32,
    type_id: TypeId,
    declared: bool,
    inverse: Option<Inverse<S, C>>,
    prepare: Arc<Prepare<C>>,
    restore: Arc<Restore<C>>,
    wrap: Arc<Wrap<C>>,
}

struct Inverse<S, C> {
    type_id: TypeId,
    name: &'static str,
    callback: Arc<Reverse<S, C>>,
}

struct EntryValue<'a, S, T> {
    entry: TransactionEntry<S, T>,
    value: &'a T,
}

/* === Enums === */

enum Disposition<S, T> {
    Undeclared,
    Barrier,
    Reversible { type_id: fn() -> TypeId, name: fn() -> &'static str, inverse: fn(&T, &S) -> Erased },
}

/* === Traits === */

/* === Implementations === */

impl<S, T> Copy for Disposition<S, T> {}
impl<S, T> Clone for Disposition<S, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S, T> Copy for TransactionEntry<S, T> {}
impl<S, T> Clone for TransactionEntry<S, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S, T> TransactionEntry<S, T> {
    pub const fn new(id: &'static str) -> Self {
        Self {
            record: RecordEntry::new(id),
            disposition: Disposition::Undeclared,
        }
    }

    pub const fn id(self) -> &'static str {
        self.record.id()
    }

    pub const fn barrier(mut self) -> Self {
        self.disposition = Disposition::Barrier;
        self
    }

    pub const fn reversible<I>(mut self) -> Self
    where
        T: Invertible<S, Inverse = I>,
        I: Transaction<S> + Send + Sync + 'static,
    {
        self.disposition = Disposition::Reversible {
            type_id: TypeId::of::<I>,
            name: std::any::type_name::<I>,
            inverse: reverse::<S, T>,
        };
        self
    }
}

impl<S, T: crate::schema::Described> TransactionEntry<S, T> {
    pub fn definition(self) -> (DefinitionId, u32) {
        self.record.definition()
    }

    pub fn prepare(self, value: &T) -> Result<Prepared, PreparationError>
    where
        T: Serialize,
    {
        Prepared::encode(&EntryValue { entry: self, value })
    }

    pub fn restore<'de, D: serde::Deserializer<'de>>(self, revision: u32, payload: D) -> Result<Option<T>, D::Error>
    where
        T: serde::Deserialize<'de>,
    {
        self.record.restore(revision, payload)
    }
}

impl<S, T: crate::schema::Described + Serialize> CatalogCodec for EntryValue<'_, S, T> {
    fn identify(&self) -> (DefinitionId, u32) {
        self.entry.definition()
    }
    fn encode<D: serde::Serializer>(&self, serializer: D) -> Result<D::Ok, D::Error> {
        self.value.serialize(serializer)
    }
}

impl<S, C> Clone for StateCatalog<S, C> {
    fn clone(&self) -> Self {
        Self { entries: self.entries.clone() }
    }
}
impl<S, C> Default for StateCatalog<S, C> {
    fn default() -> Self {
        Self { entries: Vec::new() }
    }
}

impl<S: 'static, C: 'static> StateCatalog<S, C> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn transaction<T>(mut self, entry: TransactionEntry<S, T>, wrap: fn(T) -> C, project: for<'a> fn(&'a C) -> Option<&'a T>) -> Self
    where
        T: Transaction<S> + crate::schema::Described + Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        let (id, version) = entry.definition();
        let inverse = match entry.disposition {
            Disposition::Reversible { type_id, name, inverse } => {
                let callback: Arc<Reverse<S, C>> = Arc::new(move |change, before| project(change).map(|change| inverse(change, before)));
                Some(Inverse {
                    type_id: type_id(),
                    name: name(),
                    callback,
                })
            }
            _ => None,
        };
        self.entries.push(Arc::new(Entry {
            id,
            version,
            type_id: TypeId::of::<T>(),
            declared: !matches!(entry.disposition, Disposition::Undeclared),
            inverse,
            prepare: Arc::new(move |value| project(value).map(|value| entry.prepare(value)).transpose()),
            restore: Arc::new(move |value| serde::Deserialize::deserialize(value).map(wrap)),
            wrap: Arc::new(move |value| value.downcast::<T>().ok().map(|value| wrap(*value))),
        }));
        self
    }

    pub fn extend(mut self, catalog: Self) -> Self {
        self.entries.extend(catalog.entries);
        self
    }

    pub fn validate(&self) -> crate::Result<()> {
        let mut identities = std::collections::BTreeSet::new();
        let mut types = std::collections::HashSet::new();
        for entry in &self.entries {
            if !identities.insert((entry.id.clone(), entry.version)) || !types.insert(entry.type_id) {
                return Err(crate::MessageError::new(format!("transaction `{}` is registered twice", entry.id)).into());
            }
            if !entry.declared {
                return Err(
                    crate::MessageError::new(format!("transaction `{}` must declare a reversible inverse or a barrier", entry.id)).into()
                );
            }
            if let Some(inverse) = &entry.inverse {
                let partner = self.entries.iter().find(|partner| partner.type_id == inverse.type_id).ok_or_else(|| {
                    crate::MessageError::new(format!("transaction `{}` inverse `{}` is not registered", entry.id, inverse.name))
                })?;
                if partner.inverse.as_ref().map(|inverse| inverse.type_id) != Some(entry.type_id) {
                    return Err(crate::MessageError::new(format!(
                        "transaction `{}` inverse `{}` does not name it back",
                        entry.id, partner.id
                    ))
                    .into());
                }
            }
        }
        Ok(())
    }

    pub fn build(self) -> crate::Result<Self> {
        self.validate()?;
        Ok(self)
    }

    pub fn definitions(&self) -> Vec<(Kind, DefinitionId, u32)> {
        self.entries.iter().map(|entry| (Kind::Transaction, entry.id.clone(), entry.version)).collect()
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
        let Some(entry) = self.entries.iter().find(|entry| &entry.id == id && entry.version == version) else { return Ok(None) };
        let value = <serde_json::Value as serde::Deserialize>::deserialize(payload)?;
        (entry.restore)(&value).map(Some).map_err(serde::de::Error::custom)
    }

    pub fn decode(&self, id: &DefinitionId, version: u32, bytes: &[u8]) -> Result<Option<C>, DecodeError> {
        let malformed = || DecodeError::Malformed { id: id.clone(), version };
        let payload: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| malformed())?;
        self.restore(id, version, &payload).map_err(|_| malformed())
    }

    pub fn inverse(&self, change: &C, before: &S) -> Option<C> {
        for entry in &self.entries {
            let Some(inverse) = &entry.inverse else { continue };
            if let Some(value) = (inverse.callback)(change, before) {
                return self.entries.iter().find(|entry| entry.type_id == inverse.type_id).and_then(|entry| (entry.wrap)(value));
            }
        }
        None
    }

    pub fn replay(&self, prepared: &Prepared, state: &mut S) -> Result<(), DecodeError>
    where
        C: Transaction<S>,
    {
        let malformed = || DecodeError::Malformed {
            id: prepared.id.clone(),
            version: prepared.version,
        };
        let change = self.restore(&prepared.id, prepared.version, &prepared.payload).map_err(|_| malformed())?.ok_or_else(malformed)?;
        change.apply(state);
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn reverse<S, T: Invertible<S>>(value: &T, before: &S) -> Erased
where
    T::Inverse: Send + Sync + 'static,
{
    Box::new(value.inverse(before))
}

/* === Tests === */
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(crate::Described, Serialize, serde::Deserialize, Debug, PartialEq)]
    #[harmos_path(crate)]
    struct Replace {
        value: i64,
    }

    impl Transaction<i64> for Replace {
        type Error = std::convert::Infallible;
        fn check(&self, _: &i64) -> Result<(), Self::Error> {
            Ok(())
        }
        fn apply(&self, state: &mut i64) {
            *state = self.value;
        }
    }

    impl Invertible<i64> for Replace {
        type Inverse = Self;
        fn inverse(&self, before: &i64) -> Self {
            Self { value: *before }
        }
    }

    #[test]
    fn accepted_payload_replays_and_inverse_restores() {
        let entry = TransactionEntry::<i64, Replace>::new("sheet.replace").reversible::<Replace>();
        let catalog = StateCatalog::new().transaction(entry, std::convert::identity, |value| Some(value)).build().unwrap();
        let mut change = Replace { value: 7 };
        let prepared = catalog.prepare(&change).unwrap().unwrap();
        let stored = serde_json::to_vec(&prepared).unwrap();
        let restored: Prepared = serde_json::from_slice(&stored).unwrap();
        change.value = 99;
        let mut state = 2;
        change.apply(&mut state);
        assert_eq!(state, 99);
        catalog.replay(&restored, &mut state).unwrap();
        assert_eq!(state, 7);
        let original = catalog.restore(&restored.id, restored.version, &restored.payload).unwrap().unwrap();
        let inverse = catalog.inverse(&original, &2).unwrap();
        inverse.apply(&mut state);
        assert_eq!(state, 2);
        assert!(catalog.clone().extend(catalog).build().is_err());
    }

    #[test]
    fn barrier_must_be_explicit() {
        let entry = TransactionEntry::<i64, Replace>::new("sheet.replace");
        assert!(StateCatalog::new().transaction(entry, std::convert::identity, |value| Some(value)).build().is_err());
        let catalog = StateCatalog::new().transaction(entry.barrier(), std::convert::identity, |value| Some(value)).build().unwrap();
        let change = Replace { value: 8 };
        assert!(catalog.inverse(&change, &2).is_none());
        let prepared = catalog.prepare(&change).unwrap().unwrap();
        let mut state = 2;
        catalog.replay(&prepared, &mut state).unwrap();
        assert_eq!(state, 8);
    }

    #[derive(crate::Described, Serialize, serde::Deserialize)]
    #[harmos_path(crate)]
    struct Forward {
        value: i64,
    }
    #[derive(crate::Described, Serialize, serde::Deserialize)]
    #[harmos_path(crate)]
    struct Backward {
        value: i64,
    }
    enum Change {
        Forward(Forward),
        Backward(Backward),
    }
    impl Transaction<i64> for Forward {
        type Error = std::convert::Infallible;
        fn check(&self, _: &i64) -> Result<(), Self::Error> {
            Ok(())
        }
        fn apply(&self, state: &mut i64) {
            *state += self.value;
        }
    }
    impl Transaction<i64> for Backward {
        type Error = std::convert::Infallible;
        fn check(&self, _: &i64) -> Result<(), Self::Error> {
            Ok(())
        }
        fn apply(&self, state: &mut i64) {
            *state -= self.value;
        }
    }
    impl Invertible<i64> for Forward {
        type Inverse = Backward;
        fn inverse(&self, _: &i64) -> Backward {
            Backward { value: self.value }
        }
    }
    impl Invertible<i64> for Backward {
        type Inverse = Forward;
        fn inverse(&self, _: &i64) -> Forward {
            Forward { value: self.value }
        }
    }
    #[test]
    fn inverse_membership_and_mutual_pairing_are_checked() {
        let forward = TransactionEntry::<i64, Forward>::new("sheet.forward").reversible::<Backward>();
        let backward = TransactionEntry::<i64, Backward>::new("sheet.backward");
        let catalog = StateCatalog::new().transaction(forward, Change::Forward, |value| match value {
            Change::Forward(value) => Some(value),
            _ => None,
        });
        assert!(catalog.clone().build().is_err());
        assert!(
            catalog
                .clone()
                .transaction(backward.barrier(), Change::Backward, |value| {
                    match value {
                        Change::Backward(value) => Some(value),
                        _ => None,
                    }
                })
                .build()
                .is_err()
        );
        assert!(
            catalog
                .transaction(backward.reversible::<Forward>(), Change::Backward, |value| {
                    match value {
                        Change::Backward(value) => Some(value),
                        _ => None,
                    }
                })
                .build()
                .is_ok()
        );
    }
}
