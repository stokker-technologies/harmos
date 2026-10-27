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
use harmos_domain::inventory::{contracts, listeners, observers, publications};
use harmos_operations::lifecycle;
use harmos_operations::lifecycle::Policy;
use harmos_sdk::operations::{OperationPolicy, RetryPolicy, ServingError, WireTable};
use harmos_sdk::plugin::{Declared, Plugin};
use harmos_sidecar::{Refusal, Registrar, RegistrationError, Supervisor};

/* === Constants === */

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

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Catalog;
#[harmos_sidecar::message(harmos_path = ::harmos_sidecar)]
#[derive(Clone, PartialEq)]
struct Read {}

#[harmos_sidecar::operation(
    request = Read,
    infer_request,
    harmos_path = ::harmos_sidecar,
    id = "read",
    response = (),
    policy = Catalog::INITIALIZATION_POLICY,
    grpc(package = "fixture.v1", service = "Native", method = "Read"),
)]
struct ReadOperation;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl lifecycle::Policy for Catalog {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: std::time::Duration::from_secs(120),
        attempt_timeout: std::time::Duration::from_secs(120),
        retries: RetryPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

impl Plugin for Catalog {
    fn register(registrar: &mut Registrar) -> Result<(), RegistrationError> {
        registrar.register("zeta.catalog", 2, &serde_json::json!({"suite": "probe"}))?;
        registrar.register("alpha.catalog", 1, &vec![1, 2])
    }

    type Config = ();
    type Error = Refusal;
    type Context = Supervisor;
    type Resources = ();
    fn prepare(_: ()) -> Result<Self, Refusal> {
        panic!("export must never prepare an authored instance")
    }
    fn resources(&self) -> std::sync::Arc<()> {
        panic!("export must never acquire resources")
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        panic!("export must never initialize resources")
    }
    fn bind_operations(&self, _: &mut WireTable) -> Result<(), ServingError> {
        panic!("export must never bind authored handlers")
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        panic!("export must never enter domain shutdown")
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        panic!("export must never acquire a cleanup owner")
    }
}

impl Declared for Catalog {
    const ID: &'static str = "catalog-fixture";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const DESCRIPTION: &'static str = "Exports a catalog and never runs.";
    const PROVIDES: &'static [harmos_domain::catalog::Node<harmos_domain::operations::WireDeclaration>] =
        &[harmos_domain::catalog::Node::Leaf(harmos_domain::operations::WireDeclaration::of::<ReadOperation>())];
    const REQUIRES: &'static [harmos_domain::catalog::Node<harmos_domain::plugin::Requirement>] = &[];
    const STREAMS: &'static [publications::Entry] = &[];
    fn markers() -> Result<Vec<&'static [u8]>, harmos_domain::schema::graph::Error> {
        harmos_domain::plugin::schema_markers::<Self>(
            vec![&IDENTITY, &CONTRACTS, &PUBLICATIONS, &LISTENERS, &OBSERVERS],
            &harmos_domain::config::SchemaSettings::default(),
        )
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    harmos_sidecar::run_plugin::<Catalog>(std::num::NonZeroUsize::new(1).unwrap()).await
}

/* === Tests === */
