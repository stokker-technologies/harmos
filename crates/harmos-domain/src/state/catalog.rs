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

// Third Party Imports

// Local Imports
use super::{DecodeError, Kind};
use crate::DefinitionId;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type TransactionDecoder<T> = fn(&DefinitionId, u32, &[u8]) -> Result<Option<T>, DecodeError>;
pub type Inverse<S, T> = fn(&T, &S) -> Option<T>;

/* === Structs === */

pub struct TransactionDefinitions<S, T> {
    pub definitions: Vec<(Kind, DefinitionId, u32)>,
    pub decode: TransactionDecoder<T>,
    pub inverses: Inverse<S, T>,
}

pub struct RecordDefinitions<R> {
    pub definitions: Vec<(Kind, DefinitionId, u32)>,
    pub marker: std::marker::PhantomData<fn() -> R>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<S, T> Clone for TransactionDefinitions<S, T> {
    fn clone(&self) -> Self {
        Self { definitions: self.definitions.clone(), decode: self.decode, inverses: self.inverses }
    }
}

impl<R> Clone for RecordDefinitions<R> {
    fn clone(&self) -> Self {
        Self { definitions: self.definitions.clone(), marker: std::marker::PhantomData }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
