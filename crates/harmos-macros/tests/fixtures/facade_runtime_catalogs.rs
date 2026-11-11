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

#[harmos::guest::operation(id = "fixture.guest", response = (), policy = POLICY)]
struct GuestOperation;

#[harmos::runtime::operation(id = "fixture.application", resources = [usize], response = (), policy = POLICY)]
struct ApplicationOperation;

struct GuestPlugin;

/* === Enums === */

#[harmos::guest::catalog]
enum GuestCatalog {
    Guest(GuestOperation),
}

#[harmos::runtime::catalog]
enum ApplicationCatalog {
    Application(ApplicationOperation),
}

/* === Traits === */

/* === Implementations === */

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

impl harmos::builtins::lifecycle::Policy for GuestPlugin {
    const INITIALIZATION_POLICY: harmos::OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: harmos::OperationPolicy = POLICY;
}

impl Lifecycle for GuestPlugin {
    type Config = ();
    type Context = harmos::guest::kit::Context;
    type Error = Infallible;

    async fn prepare(_: (), _: &mut harmos::Setup<Self>) -> Result<Self, Self::Error> {
        panic!("inventory inspection never prepares plugins")
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        panic!("inventory inspection never initializes plugins")
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        panic!("inventory inspection never stops plugins")
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        panic!("inventory inspection never cleans up plugins")
    }
}

#[harmos::guest(id = "guest_fixture", requires = ())]
impl harmos::Plugin for GuestPlugin {
    const CATALOGS: &'static [harmos::operations::Catalog] = &[];
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    assert_eq!(GuestCatalog::CATALOG.declarations.len(), 1);
    assert_eq!(ApplicationCatalog::CATALOG.declarations.len(), 1);

    assert!(<GuestPlugin as harmos::__private::PluginDeclared>::PROVIDES.is_empty());

    let guest = <GuestPlugin as harmos::domain::plugin::Declared>::markers().unwrap().concat();
    let guest = harmos::domain::inventory::Declaration::inspect(&guest, harmos::domain::inventory::Facility::Guest).unwrap();
    assert!(guest.operations.is_empty());
}

/* === Tests === */
