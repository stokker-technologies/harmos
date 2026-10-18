/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Plugin Composition And Explicit Lifecycle Ownership.

/* === Modules === */
pub mod domain;
mod operations;


/* === Re-Exports === */

/* === Imports === */
use std::sync::Arc;


// Standard Library Imports

// Third Party Imports

// Local Imports


/* === Constants === */

/* === Macros === */
harmos_guest::export_plugin!(Plugin, capacity = 4);


/* === Type Aliases === */

/* === Structs === */
pub struct Plugin { resources: Arc<()> }


/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos_guest::operations::lifecycle::Policy for Plugin {
    const INITIALIZATION_POLICY: harmos_domain::operations::OperationPolicy = domain::operations::POLICY;
    const SHUTDOWN_POLICY: harmos_domain::operations::OperationPolicy = domain::operations::POLICY;
}

#[harmos_guest::plugin(id = "__PACKAGE_NAME__", provides = domain::operations::Operations, requires = ())]
impl harmos_guest::Plugin for Plugin {
    type Config = ();
    type Error = std::io::Error;
    type Context = harmos_guest::Context;
    type Resources = ();
    fn prepare(_: ()) -> Result<Self, Self::Error> { Ok(Self { resources: Arc::new(()) }) }
    fn resources(&self) -> Arc<()> { Arc::clone(&self.resources) }
    fn bind_operations(&self, table: &mut harmos_guest::operations::WireTable<Self::Context>) -> Result<(), harmos_guest::operations::ServingError> {
        table.bind_operation::<operations::greeting::Greeting>(self.resources())
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> { Ok(()) }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> { Ok(()) }
    async fn cleanup(&mut self) -> Result<(), Self::Error> { Ok(()) }
}


/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
