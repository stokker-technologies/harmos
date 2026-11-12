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
pub mod sidecar;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::Path;

// Third Party Imports

// Local Imports
use crate::app::{Runtime, SchematicEditor};
use harmos::{OperationRequest, encoding::Message as _};
use harmos_runtime::guest::Loaded;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

pub enum Plugin {
    Guest { manager: harmos_runtime::guest::PluginHost<SchematicEditor> },
    Sidecar { attached: harmos_runtime::plugin::grpc::Attached },
}

/* === Traits === */

/* === Implementations === */

impl Plugin {
    pub async fn guest(runtime: &Runtime, loaded: Loaded<SchematicEditor>) -> harmos_runtime::Result<Self> {
        guest::start(runtime, loaded).await?;
        Ok(Self::Guest { manager: runtime.plugin.clone() })
    }

    pub async fn sidecar(runtime: &Runtime, path: &Path, directory: &Path) -> harmos_runtime::Result<Self> {
        let attached = sidecar::start(runtime, path, directory).await?;
        Ok(Self::Sidecar { attached })
    }

    pub async fn stop(self) -> harmos_runtime::Result<()> {
        match self {
            Self::Guest { manager } => manager.stop(guest::NAME).await,
            Self::Sidecar { attached } => attached
                .stop("Example Complete")
                .await
                .map_err(|source| harmos_runtime::Error::OperationFailed { operation: sidecar::NAME.into(), source }),
        }
    }
}

/* === Public Functions === */

pub async fn call<Q: OperationRequest>(runtime: &Runtime, request: Q) -> harmos_runtime::Result<Q::Response> {
    let response = runtime.operations.call_encoded(Q::ID, request.encode_to_vec()).await?;
    Q::Response::decode(response.as_slice()).map_err(|error| harmos_runtime::Error::OperationPayload {
        operation: Q::ID.into(),
        source: harmos_runtime::MessageError::new(error.to_string()),
    })
}

/* === Private Functions === */

/* === Tests === */
