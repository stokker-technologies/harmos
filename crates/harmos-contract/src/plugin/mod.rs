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

    fn stop(
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
    // Empty, or the one observe control serving this plugin's observers.
    const OBSERVER_CONTROL: &'static [crate::operations::WireDeclaration] = &[];
    const STREAMS: &'static [crate::inventory::publications::Entry];

    fn markers() -> Result<Vec<&'static [u8]>, crate::schema::graph::Error>;
}

/* === Implementations === */
impl Context for () {
    fn with_generation(&self, _: [u8; 32]) -> Self {}
}

/* === Public Functions === */
// Retain Each Shared Type Once Alongside The Artifact's Carrier Sections.
pub fn schema_markers<P: Declared>(
    mut carrier: Vec<&'static [u8]>,
    settings: &crate::config::SchemaSettings,
) -> Result<Vec<&'static [u8]>, crate::schema::graph::Error> {
    use crate::schema::Described;
    use crate::schema::declaration::Definition;
    use crate::schema::graph::Error;
    let carrier_bytes = carrier
        .capacity()
        .checked_mul(std::mem::size_of::<&[u8]>())
        .ok_or(Error::Budget)?;
    if carrier_bytes > settings.preparation_bytes.get() {
        return Err(Error::Budget);
    }
    let mut roots = Vec::new();
    {
        let mut append = |definition: &crate::operations::WireDeclaration| {
            push_schema_root(&mut roots, definition.request, carrier_bytes, settings)?;
            push_schema_root(&mut roots, definition.response, carrier_bytes, settings)?;
            for item in [definition.input, definition.output].into_iter().flatten() {
                push_schema_root(&mut roots, item, carrier_bytes, settings)?;
            }
            Ok(())
        };
        crate::catalog::try_for_each(P::PROVIDES, &mut append)?;
        crate::catalog::try_for_each(P::REQUIRES, |requirement| append(&requirement.declaration))?;
        for declaration in P::OBSERVER_CONTROL {
            append(declaration)?;
        }
        crate::catalog::try_for_each(P::LISTENERS, |listener| append(&listener.callback))?;
    }
    for stream in P::STREAMS {
        push_schema_root(&mut roots, stream.row, carrier_bytes, settings)?;
    }
    crate::catalog::try_for_each(P::LISTENERS, |listener| {
        push_schema_root(&mut roots, listener.source.row(), carrier_bytes, settings)
    })?;
    push_schema_root(
        &mut roots,
        crate::operations::lifecycle::InitializeRequest::DEFINITION,
        carrier_bytes,
        settings,
    )?;
    push_schema_root(&mut roots, <()>::DEFINITION, carrier_bytes, settings)?;
    let retained = roots
        .capacity()
        .checked_mul(std::mem::size_of::<Definition>())
        .and_then(|bytes| bytes.checked_add(carrier.capacity() * std::mem::size_of::<&[u8]>()))
        .ok_or(Error::Budget)?;
    let remaining = settings
        .preparation_bytes
        .get()
        .checked_sub(retained)
        .and_then(std::num::NonZeroUsize::new)
        .ok_or(Error::Budget)?;
    let allowance = crate::config::SchemaSettings {
        preparation_bytes: remaining,
        ..*settings
    };
    let declarations = crate::schema::declaration::markers_many(roots, &allowance)?;
    let output = carrier
        .len()
        .checked_add(declarations.len())
        .and_then(|count| count.checked_mul(4 * std::mem::size_of::<&[u8]>()))
        .and_then(|bytes| bytes.checked_add(declarations.capacity() * std::mem::size_of::<&[u8]>()))
        .ok_or(Error::Budget)?;
    if output > settings.preparation_bytes.get() {
        return Err(Error::Budget);
    }
    carrier.extend(declarations);
    Ok(carrier)
}

/* === Private Functions === */
fn push_schema_root(
    roots: &mut Vec<crate::schema::declaration::Definition>,
    definition: crate::schema::declaration::Definition,
    carrier_bytes: usize,
    settings: &crate::config::SchemaSettings,
) -> Result<(), crate::schema::graph::Error> {
    use crate::schema::graph::Error;
    let count = roots.len().checked_add(1).ok_or(Error::Budget)?;
    let bytes = count
        .checked_mul(4 * std::mem::size_of::<crate::schema::declaration::Definition>())
        .and_then(|bytes| bytes.checked_add(carrier_bytes))
        .ok_or(Error::Budget)?;
    if bytes > settings.preparation_bytes.get() || count > settings.preparation_work.get() {
        return Err(Error::Budget);
    }
    roots.push(definition);
    Ok(())
}

/* === Tests === */
