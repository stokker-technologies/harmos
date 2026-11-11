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
use std::time::Duration;

// Third Party Imports

// Local Imports
use harmos_runtime::operations::{Backoff, OperationPolicy, ReplaySafety, RetryPolicy};

/* === Constants === */

/* === Macros === */

macro_rules! inert_plugin {
    ($name:ident, $id:literal, $context:ty, $catalogs:expr) => {
        impl harmos_operations::lifecycle::Policy for $name {
            const INITIALIZATION_POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
                timeout: Duration::from_secs(2),
                attempt_timeout: Duration::from_secs(2),
                cancellation_timeout: std::time::Duration::ZERO,
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
        #[harmos_runtime::guest(id = $id, description = "Inert Inventory Fixture", requires = (), harmos_path = crate::operations::wire::artifact::facade::__harmos)]
        impl harmos_runtime::guest::Plugin for $name {
            const CATALOGS: &'static [harmos_runtime::operations::CatalogDescription] = $catalogs;
        }
    };
}

#[cfg(feature = "plugin-guest")]
inert_plugin!(Guest, "selected_guest", harmos_runtime::guest::kit::Context, &[OPERATIONS_CATALOG_DESCRIPTION]);

#[cfg(feature = "plugin-guest")]
inert_plugin!(EmptyGuest, "empty_guest", harmos_runtime::guest::kit::Context, &[]);

#[cfg(feature = "plugin-guest")]
inert_plugin!(ExplicitEmptyGuest, "explicit_empty_guest", harmos_runtime::guest::kit::Context, &[]);

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Response)]
#[derive(Clone)]
struct SelectedDeclaration {}

static SELECTED_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<SelectedDeclaration>(
        "fixture.selected",
        OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_nanos(17),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::OnTransient {
                max_retries: 3,
                backoff: Backoff::Exponential {
                    initial: Duration::from_nanos(3),
                    maximum: Duration::from_millis(10),
                    jitter: harmos_runtime::operations::Jitter::Deterministic,
                },
            },
        },
    )
    .description("SelectedDeclaration \"operation\" Русский")
    .visibility(harmos_runtime::operations::Visibility::Public)
    .replay(ReplaySafety::Deduplicated);

const SELECTED_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<SelectedDeclaration> =
    harmos_runtime::operations::OperationSpec::<SelectedDeclaration>::new(&SELECTED_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Response)]
#[derive(Clone)]
struct UnselectedDeclaration {}

static UNSELECTED_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<UnselectedDeclaration>(
        "fixture.unselected",
        SELECTED_DECLARATION_OPERATION.entry().policy,
    );

const UNSELECTED_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<UnselectedDeclaration> =
    harmos_runtime::operations::OperationSpec::<UnselectedDeclaration>::new(&UNSELECTED_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Clone, Debug, Default)]
struct Response {
    value: u32,
}

#[cfg(feature = "plugin-guest")]
struct Guest;

#[cfg(feature = "plugin-guest")]
struct EmptyGuest;

#[cfg(feature = "plugin-guest")]
struct ExplicitEmptyGuest;

/* === Enums === */

static LEAVES_CATALOG_DECLARATIONS: [harmos_runtime::catalog::Node<harmos_runtime::operations::WireDeclaration>; 1] =
    [harmos_runtime::catalog::Node::Leaf(SELECTED_DECLARATION_OPERATION.wire())];
static LEAVES_CATALOG_CALLEES: [harmos_runtime::catalog::Node<&'static [harmos_runtime::operations::WireDeclaration]>; 1] =
    [harmos_runtime::catalog::Node::Leaf(SELECTED_DECLARATION_OPERATION.callees())];

const LEAVES_CATALOG_DESCRIPTION: harmos_runtime::operations::CatalogDescription = harmos_runtime::operations::CatalogDescription::new(
    concat!(module_path!(), "::Leaves"),
    &LEAVES_CATALOG_DECLARATIONS,
    &LEAVES_CATALOG_CALLEES,
    Some(leaves_catalog_batch),
);

fn leaves_catalog() -> harmos_runtime::operations::Catalog {
    leaves_catalog_batch().named(concat!(module_path!(), "::Leaves"))
}

fn leaves_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(selected_declaration_operation().expect("representable operation"));
    batch
}

static NESTED_CATALOG_DECLARATIONS: [harmos_runtime::catalog::Node<harmos_runtime::operations::WireDeclaration>; 1] =
    [harmos_runtime::catalog::Node::Nested(LEAVES_CATALOG_DESCRIPTION.declarations)];
static NESTED_CATALOG_CALLEES: [harmos_runtime::catalog::Node<&'static [harmos_runtime::operations::WireDeclaration]>; 1] =
    [harmos_runtime::catalog::Node::Nested(LEAVES_CATALOG_DESCRIPTION.callees)];

const NESTED_CATALOG_DESCRIPTION: harmos_runtime::operations::CatalogDescription = harmos_runtime::operations::CatalogDescription::new(
    concat!(module_path!(), "::Nested"),
    &NESTED_CATALOG_DECLARATIONS,
    &NESTED_CATALOG_CALLEES,
    Some(nested_catalog_batch),
);

fn nested_catalog() -> harmos_runtime::operations::Catalog {
    nested_catalog_batch().named(concat!(module_path!(), "::Nested"))
}

fn nested_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.extend(leaves_catalog()).expect("bound nested catalog");
    batch
}

static OPERATIONS_CATALOG_DECLARATIONS: [harmos_runtime::catalog::Node<harmos_runtime::operations::WireDeclaration>; 1] =
    [harmos_runtime::catalog::Node::Nested(NESTED_CATALOG_DESCRIPTION.declarations)];
static OPERATIONS_CATALOG_CALLEES: [harmos_runtime::catalog::Node<&'static [harmos_runtime::operations::WireDeclaration]>; 1] =
    [harmos_runtime::catalog::Node::Nested(NESTED_CATALOG_DESCRIPTION.callees)];

const OPERATIONS_CATALOG_DESCRIPTION: harmos_runtime::operations::CatalogDescription = harmos_runtime::operations::CatalogDescription::new(
    concat!(module_path!(), "::Operations"),
    &OPERATIONS_CATALOG_DECLARATIONS,
    &OPERATIONS_CATALOG_CALLEES,
    Some(operations_catalog_batch),
);

fn operations_catalog() -> harmos_runtime::operations::Catalog {
    operations_catalog_batch().named(concat!(module_path!(), "::Operations"))
}

fn operations_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.extend(nested_catalog()).expect("bound nested catalog");
    batch
}

/* === Traits === */

/* === Implementations === */

async fn selected_declaration_handler(
    _request: SelectedDeclaration, _: harmos_runtime::operations::FunctionContext<SelectedDeclaration>,
) -> Result<harmos_runtime::Response<Response>, std::convert::Infallible> {
    panic!("inert artifact inspection never executes operations")
}

fn selected_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        SELECTED_DECLARATION_OPERATION,
        const { &[] },
        selected_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

/* === Public Functions === */

/* === Private Functions === */

mod facade {
    /* === Modules === */

    /* === Re-Exports === */

    pub use harmos_runtime as __harmos;

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

/* === Tests === */

#[cfg(feature = "plugin-guest")]
#[test]
fn guest_inventory_contains() {
    use harmos_operations::inventory::{Declaration, Facility};
    use harmos_operations::plugin::Declared;

    let bytes = <Guest as Declared>::markers().unwrap().concat();
    let inspected = Declaration::inspect(&bytes, Facility::Guest).unwrap();
    let expected = operations_catalog().definitions().unwrap();
    assert_eq!(inspected.operations, expected.clone());
    assert_eq!(harmos_runtime::operations::Contracts::from_tree(Guest::PROVIDES).unwrap()[0].operation().clone(), expected[0]);
    assert_ne!(expected[0].id.as_str(), UNSELECTED_DECLARATION_OPERATION.entry().id);

    for bytes in [<EmptyGuest as Declared>::markers().unwrap().concat(), <ExplicitEmptyGuest as Declared>::markers().unwrap().concat()] {
        assert_eq!(Declaration::inspect(&bytes, Facility::Guest).unwrap().operations, vec![]);
    }
}
