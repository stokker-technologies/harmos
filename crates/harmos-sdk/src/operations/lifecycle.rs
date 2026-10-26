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

pub use harmos_domain::operations::lifecycle::*;

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

/* === Implementations === */

/* === Public Functions === */

// Bind A Plugin's Two Lifecycle Handlers Under Its Own Lifecycle Policy.
pub fn bind<P, I, S>(
    table: &mut crate::operations::WireTable, dependencies: crate::Dependencies,
) -> Result<(), crate::operations::ServingError>
where
    P: Policy,
    I: crate::Operation<Request = <Initialize as crate::OperationDeclaration>::Request, Response = (), Endpoints = ()>,
    S: crate::Operation<Request = <Shutdown as crate::OperationDeclaration>::Request, Response = (), Endpoints = ()>,
{
    let initialize = Descriptor::of_initialize::<P>()?;
    let shutdown = Descriptor::of_shutdown::<P>()?;
    table.install_lifecycle::<Initialize, I>(dependencies.clone(), initialize)?;
    table.install_lifecycle::<Shutdown, S>(dependencies, shutdown)
}

/* === Private Functions === */

/* === Tests === */
