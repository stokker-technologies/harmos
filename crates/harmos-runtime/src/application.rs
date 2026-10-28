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
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::{Handle, Lifecycle, ResourceError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Startup<A: Application> {
    pub runtime: Handle<A>,
}

/* === Enums === */

/* === Traits === */

pub trait Application: Lifecycle<Context = Startup<Self>> + Sync {
    type Lanes: crate::operations::Lane;

    type State: Send + Sync + 'static;

    type Transaction: crate::state::Transaction<Self::State, Error: Send + Sync + 'static>
        + crate::state::TransactionCatalog<Self::State>
        + Send
        + Sync
        + 'static;

    type Record: crate::state::RecordCatalog + Send + Sync + 'static;

    type Streams: Send + Sync + 'static;
}

/* === Implementations === */

impl<A: Application> Clone for Startup<A> {
    fn clone(&self) -> Self {
        Self { runtime: self.runtime.clone() }
    }
}

impl<A: Application> harmos_sdk::lifecycle::Context for Startup<A> {
    type Configuration = crate::runtime::Builder<A>;
}

impl<A: Application> Startup<A> {
    pub fn resource<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, ResourceError> {
        self.runtime.operations.context().resource::<T>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
