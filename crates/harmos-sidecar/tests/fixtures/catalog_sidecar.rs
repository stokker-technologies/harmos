/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use harmos_domain::inventory::{contracts, listeners, observers, publications};
use harmos_operations::lifecycle;
use harmos_operations::lifecycle::Policy;
use harmos_sdk::operations::{OperationPolicy, RetryPolicy};
use harmos_sdk::plugin::{Declared, Plugin};
use harmos_sidecar::{Refusal, Registrar, RegistrationError, Supervisor};

const SDK: [harmos_domain::operations::OperationPolicy; 2] = [Catalog::INITIALIZATION_POLICY, Catalog::SHUTDOWN_POLICY];

#[used]
static IDENTITY: [u8; contracts::identity_len("sidecar", Catalog::ID, Catalog::VERSION, Catalog::DESCRIPTION, "()")] =
    contracts::encode_identity("sidecar", Catalog::ID, Catalog::VERSION, Catalog::DESCRIPTION, "()");

#[used]
static CONTRACTS: [u8; contracts::encoded_tree_len("sidecar", Catalog::ID, Catalog::PROVIDES, &[], &SDK)] =
    contracts::encode_tree("sidecar", Catalog::ID, Catalog::PROVIDES, &[], &SDK);

#[used]
static PUBLICATIONS: [u8; publications::encoded_len("sidecar", Catalog::ID, &[])] = publications::encode("sidecar", Catalog::ID, &[]);

#[used]
static LISTENERS: [u8; listeners::encoded_len("sidecar", Catalog::ID, &[])] = listeners::encode("sidecar", Catalog::ID, &[]);

#[used]
static OBSERVERS: [u8; observers::encoded_tree_len("sidecar", Catalog::ID, &[])] = observers::encode_tree("sidecar", Catalog::ID, &[]);

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

#[harmos_macros::sidecar_catalog(harmos_path = ::harmos_sidecar)]
enum Readings {
    Read(ReadOperation),
}

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

impl Plugin for Catalog {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[Readings::CATALOG];

    fn register(registrar: &mut Registrar) -> Result<(), RegistrationError> {
        registrar.register("zeta.catalog", 2, &serde_json::json!({"suite": "probe"}))?;
        registrar.register("alpha.catalog", 1, &vec![1, 2])
    }
}

impl Declared for Catalog {
    const ID: &'static str = "catalog_fixture";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const DESCRIPTION: &'static str = "Exports a catalog and never runs.";
    const PROVIDES: &'static [harmos_domain::catalog::Node<harmos_domain::operations::WireDeclaration>] =
        <Self as Plugin>::CATALOGS[0].declarations;
    const REQUIRES: &'static [harmos_domain::catalog::Node<harmos_domain::plugin::Requirement>] = &[];
    const STREAMS: &'static [publications::Entry] = &[];
    fn markers() -> Result<Vec<&'static [u8]>, harmos_domain::schema::graph::Error> {
        harmos_domain::plugin::schema_markers::<Self>(
            vec![&IDENTITY, &CONTRACTS, &PUBLICATIONS, &LISTENERS, &OBSERVERS],
            &harmos_domain::config::SchemaSettings::default(),
        )
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    harmos_sidecar::run_plugin::<Catalog>(std::num::NonZeroUsize::new(1).unwrap()).await
}
