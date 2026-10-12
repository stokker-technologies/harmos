/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

#[cfg(feature = "guest")]
mod facade {
    pub use harmos as __harmos;
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos::operations::{
    Backoff, Declarations, OperationDeclaration, OperationPolicy, ReplaySafety, RetryPolicy,
    Visibility,
};

/* === Constants === */

/* === Macros === */
macro_rules! inert_plugin {
    ($name:ident, $id:literal, $context:ty, $provided:ty) => {
        impl harmos::operations::lifecycle::Policy for $name {
            const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
                timeout: Duration::from_secs(2),
                attempt_timeout: Duration::from_secs(2),
                retries: RetryPolicy::Never,
            };
            const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
        }
        #[harmos::plugin(id = $id, description = "Inert Inventory Fixture", provides = $provided, requires = (), harmos_path = crate::artifact::facade::__harmos)]
        impl harmos::Plugin for $name {
            type Config = ();
            type Error = std::convert::Infallible;
            type Context = $context;
            type Resources = ();
            fn prepare(_: ()) -> Result<Self, Self::Error> { panic!("inert inspection never prepares resources") }
            fn resources(&self) -> std::sync::Arc<()> { panic!("inert inspection never acquires resources") }
            async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> { panic!("inert inspection never initializes resources") }
            fn bind_operations(&self, _: &mut harmos::operations::WireTable<Self::Context>) -> Result<(), harmos::operations::ServingError> { panic!("inert inspection never binds resources") }
            async fn shutdown(&mut self, _: &Self::Context) -> Result<(), Self::Error> { panic!("inert inspection never shuts resources down") }
            async fn cleanup(&mut self) -> Result<(), Self::Error> { panic!("inert inspection never releases resources") }
        }
    };
}

#[cfg(feature = "sidecar")]
inert_plugin!(
    Native,
    "selected-native",
    harmos::sidecar::Supervisor,
    Operations
);

#[cfg(feature = "sidecar")]
inert_plugin!(EmptyNative, "empty-native", harmos::sidecar::Supervisor, ());

#[cfg(feature = "sidecar")]
inert_plugin!(
    ExplicitEmptyNative,
    "explicit-empty-native",
    harmos::sidecar::Supervisor,
    ()
);

#[cfg(feature = "guest")]
inert_plugin!(Guest, "selected-guest", harmos::guest::Context, Operations);

#[cfg(feature = "guest")]
inert_plugin!(EmptyGuest, "empty-guest", harmos::guest::Context, ());

#[cfg(feature = "guest")]
inert_plugin!(
    ExplicitEmptyGuest,
    "explicit-empty-guest",
    harmos::guest::Context,
    ()
);

/* === Type Aliases === */

/* === Structs === */
struct Selected;

struct Unselected;

#[harmos::message]
struct Request {}

#[harmos::message]
struct Response {
    value: u32,
}

#[cfg(feature = "guest")]
struct Guest;

#[cfg(feature = "guest")]
struct EmptyGuest;

#[cfg(feature = "guest")]
struct ExplicitEmptyGuest;

#[cfg(feature = "sidecar")]
struct Native;

#[cfg(feature = "sidecar")]
struct EmptyNative;

#[cfg(feature = "sidecar")]
struct ExplicitEmptyNative;

/* === Enums === */
#[harmos::catalog]
enum Leaves {
    Selected(Selected),
}

#[harmos::catalog]
enum Nested {
    #[nested]
    Empty(()),
    #[nested]
    Leaves(Leaves),
}

#[harmos::catalog]
enum Operations {
    #[nested]
    Nested(Nested),
}

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for Selected {
    type Request = Request;
    type Response = Response;
    const ID: &'static str = "fixture.selected";
    const DESCRIPTION: &'static str = "Selected \"operation\" Русский";
    const VISIBILITY: Visibility = Visibility::Shared;
    const REPLAY: ReplaySafety = ReplaySafety::Deduplicated;
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_nanos(17),
        retries: RetryPolicy::OnTransient {
            max_retries: 3,
            backoff: Backoff::Exponential {
                initial: Duration::from_nanos(3),
                maximum: Duration::from_millis(10),
                jitter: harmos::operations::Jitter::Deterministic,
            },
        },
    };
}

impl OperationDeclaration for Unselected {
    type Request = Request;
    type Response = Response;
    const ID: &'static str = "fixture.unselected";
    const POLICY: OperationPolicy = Selected::POLICY;
}

impl harmos::operations::WireOperation for Selected {
    const VERSION: u32 = 1;
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

pub(super) fn contract_scenario() {
    #[cfg(feature = "sidecar")]
    {
        scenario_runtime::check("native_inventory_contains", || {
            use harmos_contract::inventory::{Declaration, Facility};
            use harmos_contract::plugin::Declared;

            let bytes = <Native as Declared>::markers().concat();
            let inspected = Declaration::inspect(&bytes, Facility::Sidecar).unwrap();
            let expected = Declarations::of::<Operations>().unwrap();
            assert_eq!(inspected.operations, expected.clone());
            assert_eq!(
                harmos::operations::Contracts::from_tree(Native::PROVIDES).unwrap()[0]
                    .operation()
                    .clone(),
                expected[0]
            );
            assert_ne!(expected[0].id.as_str(), Unselected::ID);

            for bytes in [
                <EmptyNative as Declared>::markers().concat(),
                <ExplicitEmptyNative as Declared>::markers().concat(),
            ] {
                assert_eq!(
                    Declaration::inspect(&bytes, Facility::Sidecar)
                        .unwrap()
                        .operations,
                    vec![]
                );
            }
        });
    }
    #[cfg(feature = "guest")]
    {
        scenario_runtime::check("guest_inventory_contains", || {
            use harmos_contract::inventory::{Declaration, Facility};
            use harmos_contract::plugin::Declared;

            let bytes = <Guest as Declared>::markers().concat();
            let inspected = Declaration::inspect(&bytes, Facility::Guest).unwrap();
            let expected = Declarations::of::<Operations>().unwrap();
            assert_eq!(inspected.operations, expected.clone());
            assert_eq!(
                harmos::operations::Contracts::from_tree(Guest::PROVIDES).unwrap()[0]
                    .operation()
                    .clone(),
                expected[0]
            );
            assert_ne!(expected[0].id.as_str(), Unselected::ID);

            for bytes in [
                <EmptyGuest as Declared>::markers().concat(),
                <ExplicitEmptyGuest as Declared>::markers().concat(),
            ] {
                assert_eq!(
                    Declaration::inspect(&bytes, Facility::Guest)
                        .unwrap()
                        .operations,
                    vec![]
                );
            }
        });
    }
}
