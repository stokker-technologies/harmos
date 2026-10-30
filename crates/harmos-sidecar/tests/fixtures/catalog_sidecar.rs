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

// Third Party Imports

// Local Imports
use harmos_operations::lifecycle;
use harmos_operations::lifecycle::Policy;
use harmos_sdk::operations::{OperationPolicy, RetryPolicy};
use harmos_sdk::plugin::Plugin;
use harmos_sidecar::{Refusal, Registrar, RegistrationError, Supervisor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Catalog;

#[harmos_sidecar::operation(
    harmos_path = ::harmos_sidecar,
    id = "catalog.read",
    response = (),
    policy = Catalog::INITIALIZATION_POLICY,
    visibility = Public, grpc(package = "fixture.v1", service = "Native", method = "Read"),
)]
#[derive(Clone, PartialEq)]
struct ReadOperation {}

/* === Enums === */

#[harmos_macros::sidecar_catalog(harmos_path = ::harmos_sidecar)]
enum Readings {
    Read(ReadOperation),
}

/* === Traits === */

/* === Implementations === */

impl harmos_sdk::Operation for ReadOperation {
    type Error = Refusal;

    async fn run(self, _: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::Response<()>, Refusal> {
        panic!("catalog export must never execute an operation")
    }
}

impl lifecycle::Policy for Catalog {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: std::time::Duration::from_secs(120),
        attempt_timeout: std::time::Duration::from_secs(120),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: RetryPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

impl harmos_sdk::Lifecycle for Catalog {
    type Config = ();
    type Error = Refusal;
    type Context = Supervisor;

    async fn prepare(_: (), _setup: &mut harmos_sdk::Setup<Self>) -> Result<Self, Refusal> {
        panic!("export must never prepare an authored instance")
    }

    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        panic!("export must never initialize resources")
    }

    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        panic!("export must never enter domain shutdown")
    }

    async fn cleanup(&mut self) -> Result<(), Refusal> {
        panic!("export must never acquire a cleanup owner")
    }
}

#[harmos_macros::plugin(
    harmos_path = ::harmos_sidecar,
    id = "catalog_fixture",
    description = "Exports a catalog and never runs.",
    requires(ReadOperation),
)]
impl Plugin for Catalog {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[Readings::CATALOG];

    fn register(registrar: &mut Registrar) -> Result<(), RegistrationError> {
        registrar.register("zeta.catalog", 2, &serde_json::json!({"suite": "probe"}))?;
        registrar.register("alpha.catalog", 1, &vec![1, 2])
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    harmos_sidecar::run_plugin::<Catalog>(std::num::NonZeroUsize::new(1).unwrap()).await
}

/* === Tests === */
