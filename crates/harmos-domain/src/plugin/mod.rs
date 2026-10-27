/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod requirement;

/* === Re-Exports === */

pub use requirement::{Grant, Requirement, Requirements, aggregate, callee_count};

/* === Imports === */

// Standard Library Imports
// Third Party Imports
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

pub trait Contribution: 'static {
    const NAMESPACE: &'static str;
    const SECTION: crate::inventory::Section;

    fn register(registrar: &mut crate::Registrar) -> Result<(), crate::RegistrationError>;
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
    const STREAMS: &'static [crate::inventory::publications::Entry];

    fn markers() -> Result<Vec<&'static [u8]>, crate::schema::graph::Error>;
}

/* === Implementations === */

/* === Public Functions === */

// Retain Each Shared Type Once Alongside The Artifact's Carrier Sections.
pub fn schema_markers<P: Declared>(
    mut carrier: Vec<&'static [u8]>, settings: &crate::config::SchemaSettings,
) -> Result<Vec<&'static [u8]>, crate::schema::graph::Error> {
    use crate::schema::Described;
    use crate::schema::declaration::Definition;
    use crate::schema::graph::Error;
    let carrier_bytes = carrier.capacity().checked_mul(std::mem::size_of::<&[u8]>()).ok_or(Error::Budget)?;
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
        if crate::catalog::count(P::OBSERVERS) != 0 {
            append(&crate::operations::WireDeclaration::of::<crate::observers::Control>())?;
        }
        crate::catalog::try_for_each(P::LISTENERS, |listener| append(&listener.callback))?;
    }
    for stream in P::STREAMS {
        push_schema_root(&mut roots, stream.row, carrier_bytes, settings)?;
    }
    crate::catalog::try_for_each(P::LISTENERS, |listener| push_schema_root(&mut roots, listener.source.row(), carrier_bytes, settings))?;
    push_schema_root(&mut roots, crate::model::operations::lifecycle::Action::Initialize.request(), carrier_bytes, settings)?;
    push_schema_root(&mut roots, <()>::DEFINITION, carrier_bytes, settings)?;
    let retained = roots
        .capacity()
        .checked_mul(std::mem::size_of::<Definition>())
        .and_then(|bytes| bytes.checked_add(carrier.capacity() * std::mem::size_of::<&[u8]>()))
        .ok_or(Error::Budget)?;
    let remaining = settings.preparation_bytes.get().checked_sub(retained).and_then(std::num::NonZeroUsize::new).ok_or(Error::Budget)?;
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
    roots: &mut Vec<crate::schema::declaration::Definition>, definition: crate::schema::declaration::Definition, carrier_bytes: usize,
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
