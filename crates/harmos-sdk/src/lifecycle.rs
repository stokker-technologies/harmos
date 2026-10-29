/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Application And Plugin Lifecycle

pub use crate::setup::{ApplicationConfiguration, SetupConfiguration};

use std::future::Future;

use serde::de::DeserializeOwned;

use crate::setup::Setup;

pub trait Context: Clone + Send + Sync + 'static {
    #[doc(hidden)]
    type Configuration: SetupConfiguration;
}

pub trait Lifecycle: Sized + Send + 'static {
    type Config: DeserializeOwned + Default + Send + 'static;
    type Error: std::error::Error + Send + Sync + 'static;
    type Context: Context;

    fn prepare(config: Self::Config, setup: &mut Setup<Self>) -> impl Future<Output = Result<Self, Self::Error>> + Send;
    fn initialize(&mut self, _context: &Self::Context) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
    fn stop(&mut self, _context: &Self::Context) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
    fn cleanup(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
}
