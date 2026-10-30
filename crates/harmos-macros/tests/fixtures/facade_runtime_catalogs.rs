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

/* === Imports === */

// Standard Library Imports
use std::convert::Infallible;
use std::time::Duration;

// Third Party Imports

// Local Imports
use harmos::{Lifecycle, Operation};

/* === Constants === */

const POLICY: harmos::OperationPolicy = harmos::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::sidecar::operation(id = "fixture.sidecar", response = (), policy = POLICY)]
struct SidecarOperation;

#[harmos::runtime::operation(id = "fixture.application", resources = [usize], response = (), policy = POLICY)]
struct ApplicationOperation;

struct Plugin;

struct EmptyPlugin;

/* === Enums === */

#[harmos::sidecar::catalog]
enum SidecarCatalog {
    Sidecar(SidecarOperation),
}

#[harmos::runtime::catalog]
enum ApplicationCatalog {
    Application(ApplicationOperation),
}

/* === Traits === */

/* === Implementations === */

impl Operation for SidecarOperation {
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

impl harmos::builtins::lifecycle::Policy for EmptyPlugin {
    const INITIALIZATION_POLICY: harmos::OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: harmos::OperationPolicy = POLICY;
}

impl Lifecycle for EmptyPlugin {
    type Config = ();
    type Context = harmos::sidecar::Supervisor;
    type Error = Infallible;

    async fn prepare(_: (), _: &mut harmos::Setup<Self>) -> Result<Self, Self::Error> { panic!("inventory inspection never prepares plugins") }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> { panic!("inventory inspection never initializes plugins") }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> { panic!("inventory inspection never stops plugins") }
    async fn cleanup(&mut self) -> Result<(), Self::Error> { panic!("inventory inspection never cleans up plugins") }
}

#[harmos::sidecar(id = "empty_fixture", requires = ())]
impl harmos::Plugin for EmptyPlugin {
    const CATALOGS: &'static [harmos::operations::Catalog] = &[];
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    assert_eq!(SidecarCatalog::CATALOG.declarations.len(), 1);
    assert_eq!(ApplicationCatalog::CATALOG.declarations.len(), 1);

    assert_eq!(<Plugin as harmos::__private::PluginDeclared>::PROVIDES.len(), 1);
    assert!(<EmptyPlugin as harmos::__private::PluginDeclared>::PROVIDES.is_empty());

    let native = <Plugin as harmos::domain::plugin::Declared>::markers().unwrap().concat();
    let native = harmos::domain::inventory::Declaration::inspect(&native, harmos::domain::inventory::Facility::Sidecar).unwrap();
    assert_eq!(native.operations.len(), 1);
    assert_eq!(native.operations[0].id.as_str(), "fixture.sidecar");

    let empty = <EmptyPlugin as harmos::domain::plugin::Declared>::markers().unwrap().concat();
    let empty = harmos::domain::inventory::Declaration::inspect(&empty, harmos::domain::inventory::Facility::Sidecar).unwrap();
    assert!(empty.operations.is_empty());
}

/* === Tests === */
