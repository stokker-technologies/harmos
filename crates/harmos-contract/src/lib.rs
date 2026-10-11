/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod answer;
pub mod catalog;
mod config;
mod error;
#[cfg(feature = "reflection")]
pub mod grpc;
mod identity;
pub mod inventory;
pub mod journal;
pub mod listeners;
mod message;
mod metadata;
pub mod observers;
pub mod operations;
pub mod plugin;
mod presentation;
mod refusal;
mod registration;
mod scan;
pub mod schema;
mod value;

pub mod __private {
    pub use crate::answer::Answer;
    #[cfg(feature = "grpc")]
    pub use crate::message::Protobuf;
    pub use crate::message::{
        DecodeField, DecodeInput, DecodeScalar, DecodeValue, MessageError, decoding_add,
        decoding_multiply, decoding_vector,
    };
    pub use crate::message::{PostcardReader, PostcardValue};
    pub use crate::with_protobuf;
    #[cfg(feature = "grpc")]
    pub use prost;
}

/* === Re-Exports === */
pub use config::{ConfigError, decode_config, encode_config, validate_config};
pub use error::{Error, Result};
pub use identity::{
    Requirement, artifact_subset, qualified, qualified_len, qualify, stream_subset,
};
pub use inventory::StreamDeclaration;
pub use inventory::publications::StreamSpec;
pub use metadata::{Identifier, IdentifierError, Metadata};
pub use plugin::Plugin;
pub use refusal::{RESERVED, Refusal, UNKNOWN, UNREADABLE, reserved};
pub use registration::{Registrar, Registration, RegistrationError};
pub use scan::{FRAGMENT_LIMIT, KINDS, PREFIX, ScanError, fragments};
pub use value::{VALUE_DEPTH, Value, ValueError};

#[cfg(feature = "derive")]
pub use harmos_macros::{catalog, message, operation};
#[cfg(feature = "grpc")]
pub use message::Protobuf;
pub use message::{MemorySize, Message, MessageError, Storable, btree_map_memory};
pub use presentation::{RowMetadata, StreamField};

#[cfg(feature = "grpc")]
#[macro_export]
macro_rules! with_protobuf { ($($tokens:tt)*) => { $($tokens)* }; }
#[cfg(not(feature = "grpc"))]
#[macro_export]
macro_rules! with_protobuf {
    ($($tokens:tt)*) => {};
}

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
