/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


#[path = "policy.rs"]
mod controls;


struct Plugin;


impl harmos::builtins::lifecycle::Policy for Plugin {
    const INITIALIZATION_POLICY: harmos::OperationPolicy = controls::POLICY;
    const SHUTDOWN_POLICY: harmos::OperationPolicy = controls::POLICY;
}

impl harmos::Lifecycle for Plugin {
    type Config = ();
    type Context = harmos::sidecar::Supervisor;
    type Error = std::convert::Infallible;

    async fn prepare(_: (), setup: &mut harmos::Setup<Self>) -> Result<Self, Self::Error> {
        setup.operations::<()>(());
        Ok(Self)
    }

    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> { Ok(()) }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> { Ok(()) }
    async fn cleanup(&mut self) -> Result<(), Self::Error> { Ok(()) }
}

#[harmos::sidecar::plugin(id = "setup_operations", requires = ())]
impl harmos::Plugin for Plugin {
    const CATALOGS: &'static [harmos::operations::Catalog] = &[];
}


fn main() {}
