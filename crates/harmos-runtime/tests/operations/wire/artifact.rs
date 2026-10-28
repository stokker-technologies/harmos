/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod facade {
    pub use harmos_runtime as __harmos;
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos_runtime::operations::{Backoff, Declarations, OperationDeclaration, OperationPolicy, ReplaySafety, RetryPolicy};

/* === Constants === */

/* === Macros === */

macro_rules! inert_plugin {
    ($name:ident, $id:literal, $context:ty, $catalogs:expr) => {
        impl harmos_operations::lifecycle::Policy for $name {
            const INITIALIZATION_POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
                timeout: Duration::from_secs(2),
                attempt_timeout: Duration::from_secs(2),
                retries: RetryPolicy::Never,
            };
            const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
        }
        impl harmos_runtime::Lifecycle for $name {
            type Config = ();
            type Error = std::convert::Infallible;
            type Context = $context;
            async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> Result<Self, Self::Error> { Ok(Self) }
            async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> { panic!("inert inspection never initializes resources") }
            async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> { panic!("inert inspection never shuts resources down") }
            async fn cleanup(&mut self) -> Result<(), Self::Error> { panic!("inert inspection never releases resources") }
        }
        #[harmos_runtime::plugin(id = $id, description = "Inert Inventory Fixture", requires = (), harmos_path = crate::operations::wire::artifact::facade::__harmos)]
        impl harmos_runtime::Plugin for $name {
            const CATALOGS: &'static [harmos_runtime::operations::Catalog] = $catalogs;
        }
    };
}

#[cfg(feature = "plugin-sidecar")]
inert_plugin!(Native, "selected_native", harmos_runtime::sidecar::Supervisor, &[Operations::CATALOG]);

#[cfg(feature = "plugin-sidecar")]
inert_plugin!(EmptyNative, "empty_native", harmos_runtime::sidecar::Supervisor, &[]);

#[cfg(feature = "plugin-sidecar")]
inert_plugin!(ExplicitEmptyNative, "explicit_empty_native", harmos_runtime::sidecar::Supervisor, &[]);

#[cfg(feature = "plugin-guest")]
inert_plugin!(Guest, "selected_guest", harmos_runtime::guest::kit::Context, &[Operations::CATALOG]);

#[cfg(feature = "plugin-guest")]
inert_plugin!(EmptyGuest, "empty_guest", harmos_runtime::guest::kit::Context, &[]);

#[cfg(feature = "plugin-guest")]
inert_plugin!(ExplicitEmptyGuest, "explicit_empty_guest", harmos_runtime::guest::kit::Context, &[]);

/* === Type Aliases === */

/* === Structs === */

#[harmos_runtime::operation(id = "fixture.selected",
    response = Response,
    description = "SelectedDeclaration \"operation\" Русский",
    visibility = Shared,
    replay = ReplaySafety::Deduplicated,
    policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_nanos(17),
        retries: RetryPolicy::OnTransient {
            max_retries: 3,
            backoff: Backoff::Exponential {
                initial: Duration::from_nanos(3),
                maximum: Duration::from_millis(10),
                jitter: harmos_runtime::operations::Jitter::Deterministic,
            },
        },
    },
)]
struct SelectedDeclaration {}

#[harmos_runtime::operation(id = "fixture.unselected",
    response = Response,
    policy = SelectedDeclaration::POLICY
)]
struct UnselectedDeclaration {}

#[harmos_runtime::message]
struct Response {
    value: u32,
}

#[cfg(feature = "plugin-guest")]
struct Guest;

#[cfg(feature = "plugin-guest")]
struct EmptyGuest;

#[cfg(feature = "plugin-guest")]
struct ExplicitEmptyGuest;

#[cfg(feature = "plugin-sidecar")]
struct Native;

#[cfg(feature = "plugin-sidecar")]
struct EmptyNative;

#[cfg(feature = "plugin-sidecar")]
struct ExplicitEmptyNative;

/* === Enums === */

#[harmos_runtime::catalog]
enum Leaves {
    SelectedDeclaration(SelectedDeclaration),
}

#[harmos_runtime::catalog]
enum Nested {
    #[nested]
    Empty(()),

    #[nested]
    Leaves(Leaves),
}

#[harmos_runtime::catalog]
enum Operations {
    #[nested]
    Nested(Nested),
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Operation for SelectedDeclaration {
    type Error = std::convert::Infallible;

    async fn run(self, _: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Response>, Self::Error> {
        panic!("inert artifact inspection never executes operations")
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(feature = "plugin-sidecar")]
#[test]
fn native_inventory_contains() {
    use harmos_domain::inventory::{Declaration, Facility};
    use harmos_domain::plugin::Declared;

    let bytes = <Native as Declared>::markers().unwrap().concat();
    let inspected = Declaration::inspect(&bytes, Facility::Sidecar).unwrap();
    let expected = Declarations::of::<Operations>().unwrap();
    assert_eq!(inspected.operations, expected.clone());
    assert_eq!(harmos_runtime::operations::Contracts::from_tree(Native::PROVIDES).unwrap()[0].operation().clone(), expected[0]);
    assert_ne!(expected[0].id.as_str(), UnselectedDeclaration::ID);

    for bytes in [<EmptyNative as Declared>::markers().unwrap().concat(), <ExplicitEmptyNative as Declared>::markers().unwrap().concat()] {
        assert_eq!(Declaration::inspect(&bytes, Facility::Sidecar).unwrap().operations, vec![]);
    }
}

#[cfg(feature = "plugin-guest")]
#[test]
fn guest_inventory_contains() {
    use harmos_domain::inventory::{Declaration, Facility};
    use harmos_domain::plugin::Declared;

    let bytes = <Guest as Declared>::markers().unwrap().concat();
    let inspected = Declaration::inspect(&bytes, Facility::Guest).unwrap();
    let expected = Declarations::of::<Operations>().unwrap();
    assert_eq!(inspected.operations, expected.clone());
    assert_eq!(harmos_runtime::operations::Contracts::from_tree(Guest::PROVIDES).unwrap()[0].operation().clone(), expected[0]);
    assert_ne!(expected[0].id.as_str(), UnselectedDeclaration::ID);

    for bytes in [<EmptyGuest as Declared>::markers().unwrap().concat(), <ExplicitEmptyGuest as Declared>::markers().unwrap().concat()] {
        assert_eq!(Declaration::inspect(&bytes, Facility::Guest).unwrap().operations, vec![]);
    }
}
