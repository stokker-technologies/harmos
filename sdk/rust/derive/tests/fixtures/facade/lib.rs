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

pub use harmos::{Message, Request, Row, plugin};
pub use serde::*;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos::Row)]
#[derive(Debug, Default, PartialEq)]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Reading {
    pub value: u32,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos::streams::StreamRow for Reading {
    const ENTRY: harmos::StreamEntry<Self> = harmos::StreamEntry::new("reading", 8).label("Reading");
}

/* === Public Functions === */

/* === Private Functions === */

pub mod __harmos {
    /* === Modules === */

    /* === Re-Exports === */

    pub use harmos::{
        MemorySize, Message, MessageError, Metadata, OperationDefinition, OperationPolicy, Request, RequestShape, RowMetadata, StreamEntry,
        StreamField, StreamSpec, __private,
    };

    #[cfg(feature = "runtime")]
    pub use harmos::runtime;

    #[cfg(feature = "runtime")]
    pub use harmos::{Context, ExecutionContext, Operation, Resource, Response};

    pub use harmos::{catalog, guest, listeners, model, operations, plugin, schema, streams};

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
}

/* === Tests === */
