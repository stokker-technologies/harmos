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


struct Bridge;

#[derive(Default)]
struct Watch;

#[harmos::stream(id = "undeclared.output", window = 4)]
struct Output { value: u64 }


#[harmos::catalog]
enum Observers { Watch(Watch) }


impl harmos::builtins::lifecycle::Policy for Bridge {
    const INITIALIZATION_POLICY: harmos::operations::OperationPolicy = controls::POLICY;
    const SHUTDOWN_POLICY: harmos::operations::OperationPolicy = controls::POLICY;
}

#[harmos::observer(interval_ms = 5, streams(Output))]
impl Watch {
    async fn observe(&mut self, _: &harmos::sidecar::ObserverContext<'_>) -> Result<(), harmos::sidecar::Refusal> { Ok(()) }
}

impl harmos::Lifecycle for Bridge {
    type Config = ();
    type Context = harmos::sidecar::Supervisor;
    type Error = std::convert::Infallible;

    async fn prepare(_: (), _: &mut harmos::Setup<Self>) -> Result<Self, Self::Error> { Ok(Self) }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> { Ok(()) }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> { Ok(()) }
    async fn cleanup(&mut self) -> Result<(), Self::Error> { Ok(()) }
}

#[harmos::sidecar::plugin(id = "missing_output", requires = (), observers = Observers, restart = "never")]
impl harmos::Plugin for Bridge {
    const CATALOGS: &'static [harmos::operations::Catalog] = &[];
}


fn main() {}
