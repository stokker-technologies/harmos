/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod owner;

/* === Re-Exports === */

pub use harmos_domain::plugin::*;
pub use owner::{Error, Owner, Table};

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// Third Party Imports
use serde::de::DeserializeOwned;

// Local Imports
use crate::operations::{ServingError, WireTable, lifecycle};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

pub trait Context: Clone + Send + Sync + 'static {
    fn with_generation(&self, token: [u8; 32]) -> Self;
}

pub trait Plugin: lifecycle::Policy + Send + Sync + Sized + 'static {
    fn register(_registrar: &mut crate::Registrar) -> Result<(), crate::RegistrationError> {
        Ok(())
    }

    type Config: DeserializeOwned + Default + Send + 'static;
    type Error: std::error::Error + Send + Sync + 'static;
    type Context: Context;
    type Resources: Send + Sync + 'static;

    fn resources(&self) -> Arc<Self::Resources>;

    fn prepare(config: Self::Config) -> Result<Self, Self::Error>;

    fn initialize(&mut self, context: &Self::Context) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn bind_operations(&self, table: &mut WireTable<Self::Context>) -> Result<(), ServingError>;

    fn stop(&mut self, context: &Self::Context) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn cleanup(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn attachment(&self) -> Option<Arc<dyn Attachment<Self::Context>>> {
        None
    }
}

pub trait Attachment<C>: Send + Sync + 'static {
    fn observers(&self) -> Vec<crate::inventory::observers::Declaration>;
    fn resources(&self) -> Arc<dyn std::any::Any + Send + Sync>;
    fn quiesce(&self) -> Pin<Box<dyn Future<Output = Result<(), ServingError>> + Send + '_>>;
}

/* === Implementations === */

impl Context for () {
    fn with_generation(&self, _: [u8; 32]) -> Self {}
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
