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

pub use harmos::{lanes, message, plugin, stream};
pub use harmos_operations::{catalog, operation};
pub use serde::*;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::stream(id = "reading", window = 8)]
#[harmos::message]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Reading {
    pub value: u32,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

pub mod __harmos {
    /* === Modules === */

    /* === Re-Exports === */

    pub use harmos::{
        __private, MemorySize, Message, MessageError, Metadata, OperationDeclaration, OperationDefinition, OperationPolicy, RowMetadata,
        StreamField, StreamSpec,
    };

    #[cfg(feature = "runtime")]
    pub use harmos::runtime;

    #[cfg(feature = "runtime")]
    pub use harmos::{Context, Dependent, ExecutionContext, Operation, Resource, Response};

    #[cfg(feature = "runtime")]
    pub use harmos::operations::OperationCatalog;
    pub use harmos::{builtins, catalog, guest, listeners, model, operations, plugin, schema, streams};

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
