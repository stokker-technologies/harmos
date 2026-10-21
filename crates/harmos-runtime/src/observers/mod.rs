/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The observer kind of resident operation. An observer's declaration (its
// interval, streams, demand keys and activation) is attribute metadata; the
// operation manager activates it through `submit_with_resources`, shares one
// instance among every activation over the same resources and reports its
// phases through the shared operation status. A backend drives one
// activation: in process over an authored observer, or remotely through a
// sidecar's observe control.

/* === Modules === */

mod driver;

#[cfg(feature = "plugin-sidecar")]
mod native;

/* === Re-Exports === */

pub use crate::operations::resident::Staged as ObserverRegistration;
pub use driver::ObserverActivation;

pub(crate) use driver::Backend;
#[cfg(feature = "plugin-sidecar")]
pub(crate) use driver::ChildSettlement;
#[cfg(feature = "plugin-sidecar")]
pub(crate) use native::Native;

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeSet;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports

// Local Imports
use crate::{Error, Result};
use harmos_domain::inventory::StreamDeclaration;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct ObserverDefinition {
    pub id: String,
    pub interval: Duration,
    pub streams: Vec<StreamDeclaration>,
    pub demand_keys: Vec<String>,
}

/* === Enums === */

/* === Traits === */

pub trait ObserverBackend: Send + Sync + 'static {
    type Resources: Send + Sync + 'static;
    fn definition(&self) -> crate::Result<ObserverDefinition>;
    fn run(&self, resources: Arc<Self::Resources>, activation: ObserverActivation) -> impl Future<Output = crate::Result<()>> + Send;
}

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

pub(crate) fn validate(definition: &ObserverDefinition) -> Result<()> {
    if definition.id.trim().is_empty() || definition.id.chars().any(char::is_control) {
        return Err(Error::InvalidInput { field: "observer identity", reason: "must be nonempty and contain no control characters" });
    }
    if definition.interval.is_zero() {
        return Err(Error::InvalidInput { field: "observer interval", reason: "must be positive" });
    }
    let mut names = BTreeSet::new();
    for stream in &definition.streams {
        if stream.id.is_empty() || stream.schema == 0 || stream.window == 0 || !names.insert(stream.id.as_str()) {
            return Err(Error::InvalidInput {
                field: "observer streams",
                reason: "stream identities must be unique and schema/window must be positive",
            });
        }
    }
    for key in &definition.demand_keys {
        if key.is_empty() || !names.insert(key.as_str()) {
            return Err(Error::InvalidInput {
                field: "observer demand",
                reason: "demand keys must be nonempty, unique, and distinct from streams",
            });
        }
    }
    Ok(())
}

/* === Tests === */
