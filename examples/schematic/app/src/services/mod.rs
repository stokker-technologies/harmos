/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod guest;
pub mod queries;
pub mod sidecar;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::Path;

// Third Party Imports

// Local Imports
use self::sidecar::Running;
use crate::app::{Runtime, SchematicEditor};
use harmos::OperationRequest;
use harmos::encoding::Message;
use harmos_runtime::guest::{Loaded, PluginHost};
use harmos_runtime::{Error, MessageError, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

pub enum Plugin {
    Guest { manager: PluginHost<SchematicEditor> },
    Sidecar { running: Running },
}

/* === Traits === */

/* === Implementations === */

impl Plugin {
    pub async fn guest(runtime: &Runtime, loaded: Loaded<SchematicEditor>) -> Result<Self> {
        guest::start(runtime, loaded).await?;
        Ok(Self::Guest { manager: runtime.plugin.clone() })
    }

    pub async fn sidecar(runtime: &Runtime, path: &Path, directory: &Path) -> Result<Self> {
        let running = sidecar::start(runtime, path, directory).await?;
        Ok(Self::Sidecar { running })
    }

    pub async fn stop(self) -> Result<()> {
        match self {
            Self::Guest { manager } => manager.stop(guest::NAME).await,
            Self::Sidecar { running } => running.stop().await,
        }
    }
}

/* === Public Functions === */

pub async fn call<Q: OperationRequest>(runtime: &Runtime, request: Q) -> Result<Q::Response> {
    let response = runtime.operations.call_encoded(Q::ID, request.encode_to_vec()).await?;
    Q::Response::decode(response.as_slice()).map_err(|error| Error::OperationPayload {
        operation: Q::ID.into(),
        source: MessageError::new(error.to_string()),
    })
}

/* === Private Functions === */

/* === Tests === */
