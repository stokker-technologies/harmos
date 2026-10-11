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
use std::borrow::Cow;
// Third Party Imports

// Local Imports
use crate::schema::{Cardinality, Described, Field, Kind};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "grpc", derive(prost::Message))]
#[cfg_attr(not(feature = "grpc"), derive(Debug, Default))]
pub struct Control {
    #[cfg_attr(feature = "grpc", prost(string, tag = "1"))]
    pub observer: String,
    #[cfg_attr(feature = "grpc", prost(uint64, tag = "2"))]
    pub generation: u64,
    #[cfg_attr(feature = "grpc", prost(bytes = "vec", tag = "3"))]
    pub context: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "grpc", derive(prost::Message))]
#[cfg_attr(not(feature = "grpc"), derive(Debug, Default))]
pub struct Status {
    #[cfg_attr(feature = "grpc", prost(enumeration = "Exit", tag = "1"))]
    pub exit: i32,
    #[cfg_attr(feature = "grpc", prost(bool, tag = "2"))]
    pub settled: bool,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[cfg_attr(feature = "grpc", derive(prost::Enumeration))]
#[repr(i32)]
pub enum Exit {
    Cancelled = 0,
    BindingPanicked = 1,
    CallbackPanicked = 2,
    StopPanicked = 3,
    StopTimedOut = 4,
    Interrupted = 5,
    StopFailed = 6,
    FinalizeFailed = 7,
    FinalizePanicked = 8,
    TickExhausted = 9,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum State {
    Starting,
    Running,
    Stopping,
    Stopped(Exit),
    Finalizing,
    Retained(Exit),
}

/* === Traits === */

/* === Implementations === */
crate::message::postcard_product!(Control { observer: String, generation: u64, context: Vec<u8> });
crate::message::postcard_product!(Status {
    exit: i32,
    settled: bool
});
impl std::fmt::Display for Exit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "observer activation ended: {self:?}")
    }
}

impl std::error::Error for Exit {}

impl crate::MemorySize for Control {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.observer.capacity())
            .saturating_add(self.context.capacity())
    }
}

impl crate::MemorySize for Status {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl Described for Control {
    const FIELDS: &'static [Field] = &[
        Field {
            name: Cow::Borrowed("observer"),
            tag: 1,
            kind: Kind::String,
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("generation"),
            tag: 2,
            kind: Kind::U64,
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("context"),
            tag: 3,
            kind: Kind::Bytes,
            cardinality: Cardinality::Required,
        },
    ];
}

impl Described for Status {
    const FIELDS: &'static [Field] = &[
        Field {
            name: Cow::Borrowed("exit"),
            tag: 1,
            kind: Kind::I32,
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("settled"),
            tag: 2,
            kind: Kind::Bool,
            cardinality: Cardinality::Required,
        },
    ];
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(not(feature = "grpc"))]
impl TryFrom<i32> for Exit {
    type Error = crate::MessageError;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Cancelled),
            1 => Ok(Self::BindingPanicked),
            2 => Ok(Self::CallbackPanicked),
            3 => Ok(Self::StopPanicked),
            4 => Ok(Self::StopTimedOut),
            5 => Ok(Self::Interrupted),
            6 => Ok(Self::StopFailed),
            7 => Ok(Self::FinalizeFailed),
            8 => Ok(Self::FinalizePanicked),
            9 => Ok(Self::TickExhausted),
            _ => Err(crate::MessageError::borrowed("unknown observer exit")),
        }
    }
}
