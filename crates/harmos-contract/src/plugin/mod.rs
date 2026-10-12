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
mod requirement;

/* === Re-Exports === */
pub use owner::{Error, Owner, Table};
pub use requirement::{Grant, Requirement, Requirements};

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

    fn initialize(
        &mut self,
        context: &Self::Context,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn bind_operations(&self, table: &mut WireTable<Self::Context>) -> Result<(), ServingError>;

    fn shutdown(
        &mut self,
        context: &Self::Context,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn cleanup(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn attachment(&self) -> Option<Arc<dyn Attachment<Self::Context>>> {
        None
    }
}

pub trait Contribution: 'static {
    const NAMESPACE: &'static str;
    const SECTION: crate::inventory::Section;

    fn register(registrar: &mut crate::Registrar) -> Result<(), crate::RegistrationError>;
}

pub trait Attachment<C>: Send + Sync + 'static {
    fn observers(&self) -> Vec<crate::inventory::observers::Declaration>;
    fn resources(&self) -> Arc<dyn std::any::Any + Send + Sync>;
    fn quiesce(&self) -> Pin<Box<dyn Future<Output = Result<(), ServingError>> + Send + '_>>;
}

pub trait Declared: Send + Sync + 'static {
    const ID: &'static str;
    const VERSION: &'static str;
    const DESCRIPTION: &'static str;
    const PROVIDES: &'static [crate::catalog::Node<crate::operations::WireDeclaration>];
    const REQUIRES: &'static [crate::catalog::Node<Requirement>];
    const CONTRIBUTIONS: &'static [crate::inventory::Section] = &[];
    const LISTENERS: &'static [crate::catalog::Node<crate::listeners::Entry>] = &[];
    const OBSERVERS: &'static [crate::catalog::Node<crate::inventory::observers::Entry>] = &[];
    const OBSERVER_CONTROLS: &'static [crate::operations::WireDeclaration] = &[];
    const STREAMS: &'static [crate::inventory::publications::Entry];

    fn markers() -> Vec<&'static [u8]>;
}

/* === Implementations === */
impl Context for () {
    fn with_generation(&self, _: [u8; 32]) -> Self {}
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
