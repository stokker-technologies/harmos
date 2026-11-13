/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod aggregate;
pub mod config;
mod error;
pub mod identity;
pub mod journal;
pub mod message;
mod metadata;
pub mod model;
mod presentation;
pub mod schema;
pub mod state;
pub mod streams;
mod value;

/* === Re-Exports === */

pub use aggregate::Aggregate;
pub use error::{Error, Result};
pub use identity::{DefinitionId, Identified, IdentityError, QualifiedId, artifact_subset, stream_subset};
pub use metadata::{Identifier, IdentifierError, Metadata};
pub use schema::marker::{FRAGMENT_LIMIT, PREFIX};
pub use streams::{Definition, StreamEntry, StreamSpec};
pub use value::{VALUE_DEPTH, Value, ValueError};

#[cfg(feature = "derive")]
pub use harmos_sdk_derive::{Described, Row, StoredMessage as Message};
pub use message::{Digest, Fingerprint, MemorySize, Message, MessageError, Portable, Storable, btree_map_memory};
pub use presentation::{RowMetadata, StreamField};

/* === Imports === */

extern crate self as harmos_domain;

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

pub mod __private {
    /* === Modules === */

    /* === Re-Exports === */

    pub use crate::message::allocation::{add as decoding_add, multiply as decoding_multiply, vector as decoding_vector};
    pub use crate::message::postcard::{Reader as PostcardReader, Value as PostcardValue};
    pub use crate::message::{MapEntry, MessageError};
    pub use crate::postcard_product;

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports
    pub extern crate serde;
    pub extern crate serde_json;

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
}

/* === Tests === */
