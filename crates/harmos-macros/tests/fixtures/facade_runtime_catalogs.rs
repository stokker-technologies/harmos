/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


use std::convert::Infallible;
use std::time::Duration;


use harmos::{Lifecycle, Operation};


const POLICY: harmos::OperationPolicy = harmos::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    retries: harmos::RetryPolicy::Never,
};


#[harmos::sidecar::operation(id = "fixture.sidecar", response = (), policy = POLICY)]
struct SidecarOperation;

#[harmos::guest::operation(id = "fixture.guest", response = (), policy = POLICY)]
struct GuestOperation;

#[harmos::runtime::operation(id = "fixture.application", resources = [usize], response = (), policy = POLICY)]
struct ApplicationOperation;

struct Plugin;

struct GuestPlugin;


#[harmos::sidecar::catalog]
enum SidecarCatalog {
    Sidecar(SidecarOperation),
}

#[harmos::guest::catalog]
enum GuestCatalog {
    Guest(GuestOperation),
}

#[harmos::runtime::catalog]
enum ApplicationCatalog {
    Application(ApplicationOperation),
}


impl Operation for SidecarOperation {
    type Error = Infallible;

    async fn run(self, _: harmos::Context<'_, Self>) -> Result<harmos::Response<()>, Self::Error> {
        Ok(().into())
    }
}

impl Operation for GuestOperation {
    type Error = Infallible;

    async fn run(self, _: harmos::Context<'_, Self>) -> Result<harmos::Response<()>, Self::Error> {
        Ok(().into())
    }
}

impl Operation for ApplicationOperation {
    type Error = Infallible;

    async fn run(self, _: harmos::Context<'_, Self>) -> Result<harmos::Response<()>, Self::Error> {
        Ok(().into())
    }
}

impl harmos::builtins::lifecycle::Policy for Plugin {
    const INITIALIZATION_POLICY: harmos::OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: harmos::OperationPolicy = POLICY;
}

impl Lifecycle for Plugin {
    type Config = ();
    type Context = harmos::sidecar::Supervisor;
    type Error = Infallible;

    async fn prepare(_: (), _: &mut harmos::Setup<Self>) -> Result<Self, Self::Error> { panic!("inventory inspection never prepares plugins") }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> { panic!("inventory inspection never initializes plugins") }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> { panic!("inventory inspection never stops plugins") }
    async fn cleanup(&mut self) -> Result<(), Self::Error> { panic!("inventory inspection never cleans up plugins") }
}

#[harmos::sidecar(id = "facade_fixture")]
impl harmos::Plugin for Plugin {
    const CATALOGS: &'static [harmos::operations::Catalog] = &[SidecarCatalog::CATALOG];
}

impl harmos::builtins::lifecycle::Policy for GuestPlugin {
    const INITIALIZATION_POLICY: harmos::OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: harmos::OperationPolicy = POLICY;
}

impl Lifecycle for GuestPlugin {
    type Config = ();
    type Context = harmos::guest::kit::Context;
    type Error = Infallible;

    async fn prepare(_: (), _: &mut harmos::Setup<Self>) -> Result<Self, Self::Error> { panic!("inventory inspection never prepares plugins") }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> { panic!("inventory inspection never initializes plugins") }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> { panic!("inventory inspection never stops plugins") }
    async fn cleanup(&mut self) -> Result<(), Self::Error> { panic!("inventory inspection never cleans up plugins") }
}

#[harmos::guest(id = "guest_fixture", requires = ())]
impl harmos::Plugin for GuestPlugin {
    const CATALOGS: &'static [harmos::operations::Catalog] = &[];
}


fn main() {
    assert_eq!(SidecarCatalog::CATALOG.declarations.len(), 1);
    assert_eq!(GuestCatalog::CATALOG.declarations.len(), 1);
    assert_eq!(ApplicationCatalog::CATALOG.declarations.len(), 1);

    assert_eq!(<Plugin as harmos::__private::PluginDeclared>::PROVIDES.len(), 1);
    assert!(<GuestPlugin as harmos::__private::PluginDeclared>::PROVIDES.is_empty());

    let native = <Plugin as harmos::domain::plugin::Declared>::markers().unwrap().concat();
    let native = harmos::domain::inventory::Declaration::inspect(&native, harmos::domain::inventory::Facility::Sidecar).unwrap();
    assert_eq!(native.operations.len(), 1);
    assert_eq!(native.operations[0].id.as_str(), "fixture.sidecar");

    let guest = <GuestPlugin as harmos::domain::plugin::Declared>::markers().unwrap().concat();
    let guest = harmos::domain::inventory::Declaration::inspect(&guest, harmos::domain::inventory::Facility::Guest).unwrap();
    assert!(guest.operations.is_empty());
}
