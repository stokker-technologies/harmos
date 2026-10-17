/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Re-Exports === */
pub use harmos_domain::operations::lifecycle::*;

/* === Public Functions === */
// Bind A Plugin's Two Lifecycle Handlers Under Its Own Lifecycle Policy.
pub fn bind<P, I, S, C>(
    table: &mut crate::operations::WireTable<C>, resources: std::sync::Arc<I::Resources>,
) -> Result<(), crate::operations::ServingError>
where
    P: Policy,
    C: Send + Sync + 'static,
    I: for<'a> crate::operations::Operation<Declaration = Initialize, Context<'a> = &'a C, Endpoints = ()>,
    S: for<'a> crate::operations::Operation<Declaration = Shutdown, Resources = I::Resources, Context<'a> = &'a C, Endpoints = ()>,
{
    let initialize = Descriptor::of_initialize::<P>()?;
    let shutdown = Descriptor::of_shutdown::<P>()?;
    table.bind_owned_lifecycle::<I>(std::sync::Arc::clone(&resources), initialize)?;
    table.bind_owned_lifecycle::<S>(resources, shutdown)
}
