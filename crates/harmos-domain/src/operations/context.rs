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
use std::any::{Any, TypeId, type_name};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
// Third Party Imports

// Local Imports
use crate::{Message, MessageError};

/* === Constants === */

pub const PORTABLE_VALUES: usize = 64;

pub const PORTABLE_BYTES: usize = 64 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Default)]
pub struct CallContext {
    values: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
    portable: BTreeMap<String, Arc<[u8]>>,
    portable_types: HashMap<&'static str, TypeId>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallValue {
    pub key: String,
    pub payload: Vec<u8>,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("call context already carries `{name}`")]
    Duplicate { name: String },

    #[error("call context does not carry `{name}`")]
    Missing { name: String },

    #[error("call context value `{key}` is invalid: {source}")]
    Decode {
        key: String,

        #[source]
        source: MessageError,
    },

    #[error("call context portable values exceed {PORTABLE_VALUES} entries or {PORTABLE_BYTES} bytes")]
    Budget,
}

/* === Traits === */

pub trait ContextValue: Message + Send + Sync + 'static {
    const KEY: &'static str;
}

/* === Implementations === */

impl CallContext {
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T) -> Result<(), ContextError> {
        let key = TypeId::of::<T>();
        if self.values.contains_key(&key) {
            return Err(ContextError::Duplicate { name: type_name::<T>().to_owned() });
        }
        self.values.insert(key, Arc::new(value));
        Ok(())
    }

    pub fn insert_portable<T: ContextValue>(&mut self, value: T) -> Result<(), ContextError> {
        if self.portable.contains_key(T::KEY) {
            return Err(ContextError::Duplicate { name: T::KEY.to_owned() });
        }
        let payload = value.encode();
        let bytes = self.portable.iter().map(|(key, value)| key.len() + value.len()).sum::<usize>() + T::KEY.len() + payload.len();
        if self.portable.len() >= PORTABLE_VALUES || bytes > PORTABLE_BYTES {
            return Err(ContextError::Budget);
        }
        self.insert(value)?;
        self.portable.insert(T::KEY.to_owned(), payload.into());
        self.portable_types.insert(T::KEY, TypeId::of::<T>());
        Ok(())
    }

    // Derive a child context with a replaced portable value. Local-only values
    // retain their original allocation and cannot be replaced by this method.
    pub fn with_value<T: ContextValue>(&self, value: T) -> Result<Self, ContextError> {
        let kind = TypeId::of::<T>();
        if self.values.contains_key(&kind) && self.portable_types.get(T::KEY) != Some(&kind) {
            return Err(ContextError::Duplicate { name: type_name::<T>().to_owned() });
        }
        let replacement = CallValue { key: T::KEY.to_owned(), payload: value.encode() };
        let mut child = self.with_values(vec![replacement])?;
        child.values.insert(kind, Arc::new(value));
        child.portable_types.insert(T::KEY, kind);
        Ok(child)
    }

    // Overlay portable values without changing trusted local-only values.
    // Replaced typed projections are invalidated; read wire values with `value`.
    pub fn with_values(&self, values: Vec<CallValue>) -> Result<Self, ContextError> {
        let replacements = Self::from_portable(values)?;
        let retained = self.portable.iter().filter(|(key, _)| !replacements.portable.contains_key(*key));
        let mut entries = replacements.portable.len();
        let mut bytes =
            replacements.portable.iter().map(|(key, value)| key.len().saturating_add(value.len())).fold(0usize, usize::saturating_add);
        for (key, value) in retained {
            entries = entries.saturating_add(1);
            bytes = bytes.saturating_add(key.len()).saturating_add(value.len());
        }
        if entries > PORTABLE_VALUES || bytes > PORTABLE_BYTES {
            return Err(ContextError::Budget);
        }
        let mut child = self.clone();
        for (key, payload) in replacements.portable {
            if let Some(kind) = child.portable_types.remove(key.as_str()) {
                child.values.remove(&kind);
            }
            child.portable.insert(key, payload);
        }
        Ok(child)
    }

    pub fn from_portable(values: Vec<CallValue>) -> Result<Self, ContextError> {
        let bytes = values.iter().map(|value| value.key.len() + value.payload.len()).sum::<usize>();
        if values.len() > PORTABLE_VALUES || bytes > PORTABLE_BYTES {
            return Err(ContextError::Budget);
        }
        let mut context = Self::default();
        for value in values {
            if context.portable.contains_key(&value.key) {
                return Err(ContextError::Duplicate { name: value.key });
            }
            context.portable.insert(value.key, value.payload.into());
        }
        Ok(context)
    }

    pub fn get<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.values.get(&TypeId::of::<T>())?.downcast_ref()
    }

    pub fn require<T: Send + Sync + 'static>(&self) -> Result<&T, ContextError> {
        self.get().ok_or_else(|| ContextError::Missing { name: type_name::<T>().to_owned() })
    }

    pub fn value<T: ContextValue>(&self) -> Result<Option<T>, ContextError> {
        let Some(payload) = self.portable.get(T::KEY) else {
            return Ok(None);
        };
        T::decode(payload).map(Some).map_err(|source| ContextError::Decode { key: T::KEY.to_owned(), source })
    }

    pub fn portable(&self) -> Vec<CallValue> {
        self.portable.iter().map(|(key, payload)| CallValue { key: key.clone(), payload: payload.to_vec() }).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.portable.is_empty()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
