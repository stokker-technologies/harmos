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
use harmos_runtime::operations::{
    Backoff, Declarations, OperationDeclaration, OperationPolicy, ReplaySafety, RestartPolicy, RetryPolicy, Visibility,
};

/* === Constants === */

/* === Macros === */
macro_rules! inert_plugin {
    ($name:ident, $id:literal, $context:ty, $provided:ty) => {
        impl harmos_runtime::operations::lifecycle::Policy for $name {
            const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
                timeout: Duration::from_secs(2),
                attempt_timeout: Duration::from_secs(2),
                retries: RetryPolicy::Never,
                restart: RestartPolicy::Never,
            };
            const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
        }
        #[harmos_runtime::plugin(id = $id, description = "Inert Inventory Fixture", provides = $provided, requires = (), harmos_path = crate::operations::wire::artifact::facade::__harmos)]
        impl harmos_runtime::Plugin for $name {
            type Config = ();
            type Error = std::convert::Infallible;
            type Context = $context;
            type Resources = ();
            fn prepare(_: ()) -> Result<Self, Self::Error> { panic!("inert inspection never prepares resources") }
            fn resources(&self) -> std::sync::Arc<()> { panic!("inert inspection never acquires resources") }
            async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> { panic!("inert inspection never initializes resources") }
            fn bind_operations(&self, _: &mut harmos_runtime::operations::WireTable<Self::Context>) -> Result<(), harmos_runtime::operations::ServingError> { panic!("inert inspection never binds resources") }
            async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> { panic!("inert inspection never shuts resources down") }
            async fn cleanup(&mut self) -> Result<(), Self::Error> { panic!("inert inspection never releases resources") }
        }
    };
}

#[cfg(feature = "plugin-sidecar")]
inert_plugin!(Native, "selected-native", harmos_runtime::sidecar::Supervisor, Operations);

#[cfg(feature = "plugin-sidecar")]
inert_plugin!(EmptyNative, "empty-native", harmos_runtime::sidecar::Supervisor, ());

#[cfg(feature = "plugin-sidecar")]
inert_plugin!(ExplicitEmptyNative, "explicit-empty-native", harmos_runtime::sidecar::Supervisor, ());

#[cfg(feature = "plugin-guest")]
inert_plugin!(Guest, "selected-guest", harmos_runtime::guest::Context, Operations);

#[cfg(feature = "plugin-guest")]
inert_plugin!(EmptyGuest, "empty-guest", harmos_runtime::guest::Context, ());

#[cfg(feature = "plugin-guest")]
inert_plugin!(ExplicitEmptyGuest, "explicit-empty-guest", harmos_runtime::guest::Context, ());

/* === Type Aliases === */

/* === Structs === */
#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "fixture.selected",
    version = 1,
    response = Response,
    description = "Selected \"operation\" Русский",
    visibility = Visibility::Shared,
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
        restart: RestartPolicy::Never,
    },
)]
struct Selected {}

#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "fixture.unselected",
    version = 1,
    response = Response,
    policy = Selected::POLICY
)]
struct Unselected {}

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
    Selected(Selected),
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
    assert_ne!(expected[0].id.as_str(), Unselected::ID);

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
    assert_ne!(expected[0].id.as_str(), Unselected::ID);

    for bytes in [<EmptyGuest as Declared>::markers().unwrap().concat(), <ExplicitEmptyGuest as Declared>::markers().unwrap().concat()] {
        assert_eq!(Declaration::inspect(&bytes, Facility::Guest).unwrap().operations, vec![]);
    }
}
