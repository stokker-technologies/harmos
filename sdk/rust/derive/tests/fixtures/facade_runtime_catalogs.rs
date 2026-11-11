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
use harmos::Lifecycle;

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

#[derive(harmos::Message, Debug, Default)]
#[harmos_path(harmos)]
#[derive(harmos::Request, Clone)]

struct GuestOperation;
static GUEST_DESCRIPTION: harmos::operations::WireDeclaration = harmos::operations::WireDeclaration::request::<GuestOperation>("fixture.guest", POLICY);

#[derive(harmos::Message, Debug, Default)]
#[harmos_path(harmos)]
#[derive(harmos::Request, Clone)]

struct ApplicationOperation;
static APPLICATION_DESCRIPTION: harmos::operations::WireDeclaration = harmos::operations::WireDeclaration::request::<ApplicationOperation>("fixture.application", POLICY);

struct GuestPlugin;

/* === Enums === */

static GUEST_DECLARATIONS: [harmos::catalog::Node<harmos::operations::WireDeclaration>; 1] = [harmos::catalog::Node::Leaf(GUEST_DESCRIPTION.copied())];

static APPLICATION_DECLARATIONS: [harmos::catalog::Node<harmos::operations::WireDeclaration>; 1] = [harmos::catalog::Node::Leaf(APPLICATION_DESCRIPTION.copied())];

/* === Traits === */

/* === Implementations === */





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

#[harmos::guest(harmos_path = ::harmos, id = "guest_fixture", requires = ())]
impl harmos::guest::Plugin for GuestPlugin {
    const CATALOGS: &'static [harmos::operations::CatalogDescription] = &[];
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    assert_eq!(GUEST_DECLARATIONS.len(), 1);
    assert_eq!(APPLICATION_DECLARATIONS.len(), 1);

    assert!(<GuestPlugin as harmos::__private::PluginDeclared>::PROVIDES.is_empty());

    let guest = <GuestPlugin as harmos::builtins::plugin::Declared>::markers().unwrap().concat();
    let guest = harmos::builtins::inventory::Declaration::inspect(&guest, harmos::builtins::inventory::Facility::Guest).unwrap();
    assert!(guest.operations.is_empty());
}

/* === Tests === */
