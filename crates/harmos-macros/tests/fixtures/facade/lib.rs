/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


pub mod __harmos {
    pub use harmos::{
        __private, MemorySize, Message, MessageError, Metadata, OperationDeclaration, OperationDefinition, OperationPolicy, RowMetadata,
        StreamField, StreamSpec,
    };
    #[cfg(feature = "runtime")]
    pub use harmos::runtime;
    #[cfg(feature = "runtime")]
    pub use harmos::{Context, Dependent, Operation, ExecutionContext, Resource, Response};
    #[cfg(feature = "runtime")]
    pub use harmos::operations::OperationCatalog;
    pub use harmos::{builtins, catalog, guest, listeners, model, operations, plugin, schema, sidecar, streams};
}


pub use harmos::{catalog, lanes, listener, message, observer, operation, plugin, stream};
pub use serde::*;


#[harmos::stream(id = "reading", window = 8)]
#[harmos::message]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Reading {
    pub value: u32,
}
