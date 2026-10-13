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
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos_contract::catalog::Node;
use harmos_contract::inventory::{contracts, observers, publications};
use harmos_contract::operations::{
    Operation, OperationDeclaration, OperationPolicy, RetryPolicy, WireDeclaration, WireOperation,
    WireTable, lifecycle,
};
use harmos_contract::plugin::{Declared, Plugin};
use harmos_guest::Context;

/* === Constants === */
const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    retries: RetryPolicy::Never,
};
const PROVIDED: &[Node<WireDeclaration>] = &[Node::Leaf(WireDeclaration::of::<Read>())];
const SDK: [harmos_contract::operations::OperationPolicy; 2] = [POLICY, POLICY];

#[used]
static IDENTITY: [u8; contracts::identity_len(
    "guest",
    "unified-guest",
    env!("CARGO_PKG_VERSION"),
    "Unified SDK fixture.",
    "String",
)] = contracts::encode_identity(
    "guest",
    "unified-guest",
    env!("CARGO_PKG_VERSION"),
    "Unified SDK fixture.",
    "String",
);

#[used]
static CONTRACTS: [u8; contracts::encoded_tree_len("guest", "unified-guest", PROVIDED, &[], &SDK)] =
    contracts::encode_tree("guest", "unified-guest", PROVIDED, &[], &SDK);

#[used]
static PUBLICATIONS: [u8; publications::encoded_len("guest", "unified-guest", &[])] =
    publications::encode("guest", "unified-guest", &[]);

#[used]
static OBSERVERS: [u8; observers::encoded_tree_len("guest", "unified-guest", &[], &[])] =
    observers::encode_tree("guest", "unified-guest", &[], &[]);

/* === Macros === */
harmos_guest::export_plugin!(Program, capacity = 2);

/* === Type Aliases === */

/* === Structs === */
struct Program {
    value: Arc<String>,
}

struct Reader {
    value: Arc<String>,
    initializations: usize,
}

struct Read;

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
struct Reply {
    value: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl lifecycle::Policy for Program {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

impl Declared for Program {
    const ID: &'static str = "unified-guest";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const DESCRIPTION: &'static str = "Unified SDK fixture.";
    const PROVIDES: &'static [Node<WireDeclaration>] = PROVIDED;
    const REQUIRES: &'static [Node<harmos_contract::plugin::Requirement>] = &[];
    const STREAMS: &'static [publications::Entry] = &[];
    fn markers() -> Result<Vec<&'static [u8]>, harmos_contract::schema::graph::Error> {
        harmos_contract::plugin::schema_markers::<Self>(
            vec![&IDENTITY, &CONTRACTS, &PUBLICATIONS, &OBSERVERS],
            &harmos_contract::config::SchemaSettings::default(),
        )
    }
}

impl Plugin for Program {
    fn register(
        registrar: &mut harmos_contract::Registrar,
    ) -> Result<(), harmos_contract::RegistrationError> {
        registrar.register("zeta.catalog", 2, &"second")?;
        registrar.register("alpha.catalog", 1, &"first")
    }

    type Config = String;
    type Error = std::io::Error;
    type Context = Context;
    type Resources = String;
    fn resources(&self) -> Arc<String> {
        self.value.clone()
    }
    fn prepare(config: String) -> Result<Self, Self::Error> {
        Ok(Self {
            value: Arc::new(config),
        })
    }
    async fn initialize(&mut self, _: &Context) -> Result<(), Self::Error> {
        Arc::get_mut(&mut self.value)
            .expect("original resources before binding")
            .push_str(":initialized");
        Ok(())
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Context>,
    ) -> Result<(), harmos_contract::operations::ServingError> {
        table.bind_operation::<Reader>(self.resources())
    }
    async fn shutdown(&mut self, _: &Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Arc::get_mut(&mut self.value)
            .expect("bindings settled before resource cleanup")
            .clear();
        Ok(())
    }
}

impl OperationDeclaration for Read {
    type Request = ();
    type Response = Reply;
    const ID: &'static str = "fixture.read";
    const POLICY: OperationPolicy = POLICY;
}

impl WireOperation for Read {
    const VERSION: u32 = 1;
}

impl Operation for Reader {
    type Declaration = Read;
    type Resources = String;
    type Error = std::io::Error;
    type Endpoints = ();
    type Context<'a> = &'a Context;

    fn new(value: Arc<String>) -> Result<Self, Self::Error> {
        Ok(Self {
            value,
            initializations: 0,
        })
    }
    async fn initialize<'ctx>(&mut self, _: Self::Context<'ctx>) -> Result<(), Self::Error> {
        self.initializations += 1;
        Ok(())
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        _request: &mut (),
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_guest::operations::Response<Reply>, Self::Error> {
        let result: ::core::result::Result<Reply, Self::Error> = async move {
            Ok(Reply {
                value: format!(
                    "{}:operation-initialized:{}",
                    self.value, self.initializations
                ),
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
