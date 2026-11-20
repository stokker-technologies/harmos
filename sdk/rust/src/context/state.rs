/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Durable Plugin State

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports
use prost::Message;
use tokio::sync::Mutex;

// Local Imports
use super::Context;
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// A Revision Observation Shared by Clones; Call Get Before Updating Existing State.
#[derive(Clone)]
pub struct StateHandle {
    context: Context,
    revision: Arc<Mutex<u64>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Context {
    /// Declare PluginsStateGet and PluginsStatePut; This Also Works During Initialize.
    pub fn state(&self) -> StateHandle {
        StateHandle {
            context: self.clone(),
            revision: Arc::new(Mutex::new(0)),
        }
    }
}

impl StateHandle {
    /// Decode the Opaque Value as Protobuf, Like Settings Payloads.
    pub async fn get<T: Message + Default>(&self) -> Result<T> {
        let mut revision = self.revision.lock().await;
        let response = self.context.host().plugins.state.get().await?;
        let value = T::decode(response.value.as_slice()).map_err(|error| Error::Invalid(format!("Decode Plugin State: {error}")))?;
        *revision = response.revision;
        Ok(value)
    }

    /// Record Intent Durably Before Performing an External Effect; Conflicts Require Get.
    pub async fn put<T: Message>(&self, value: &T) -> Result<u64> {
        let mut revision = self.revision.lock().await;
        let next = self.context.host().plugins.state.put(*revision, value.encode_to_vec()).await?;
        *revision = next;
        Ok(next)
    }

    /// Clear the Opaque Value While Advancing Its Revision.
    pub async fn clear(&self) -> Result<u64> {
        let mut revision = self.revision.lock().await;
        let next = self.context.host().plugins.state.put(*revision, Vec::new()).await?;
        *revision = next;
        Ok(next)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
