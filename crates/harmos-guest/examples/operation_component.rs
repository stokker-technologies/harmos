/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "operation_component/contract.rs"]
mod contract;
#[path = "operation_component/streams.rs"]
mod streams;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;
// Third Party Imports

// Local Imports
use contract::{Counts, Read, Value};
use harmos_contract::catalog::Node;
use harmos_contract::inventory::{contracts, listeners, observers, publications};
use harmos_contract::plugin::{Declared, Plugin};
use harmos_guest::Context;
use harmos_guest::operations::{
    Failure, Operation, OperationPolicy, RetryPolicy, ServingError, WireDeclaration, WireTable,
    lifecycle,
};

/* === Constants === */
const SDK_POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_millis(500),
    retries: RetryPolicy::Never,
};
const SDK: [harmos_guest::operations::OperationPolicy; 2] = [SDK_POLICY, SDK_POLICY];

#[used]
static IDENTITY: [u8; contracts::identity_len(
    "guest",
    Program::ID,
    Program::VERSION,
    Program::DESCRIPTION,
    "String",
)] = contracts::encode_identity(
    "guest",
    Program::ID,
    Program::VERSION,
    Program::DESCRIPTION,
    "String",
);

#[used]
static CONTRACTS: [u8; contracts::encoded_tree_len(
    "guest",
    Program::ID,
    Program::PROVIDES,
    &[],
    &SDK,
)] = contracts::encode_tree("guest", Program::ID, Program::PROVIDES, &[], &SDK);

#[used]
static PUBLICATIONS: [u8; publications::encoded_len("guest", Program::ID, &[])] =
    publications::encode("guest", Program::ID, &[]);

#[used]
static LISTENERS: [u8; listeners::encoded_len("guest", Program::ID, &[])] =
    listeners::encode("guest", Program::ID, &[]);

#[used]
static OBSERVERS: [u8; observers::encoded_tree_len("guest", Program::ID, &[], &[])] =
    observers::encode_tree("guest", Program::ID, &[], &[]);

/* === Macros === */
harmos_guest::export_plugin!(Program, capacity = 2);

/* === Type Aliases === */

/* === Structs === */
struct Program {
    owner: Arc<Owner>,
    mismatch: bool,
    pending_shutdown: bool,
}

#[derive(Default)]
struct Owner {
    fail_initialize: AtomicBool,
    child_initialize: AtomicBool,
    initialized: AtomicUsize,
    ran: AtomicUsize,
    stopped: AtomicUsize,
    finalized: AtomicUsize,
}

struct Counting(Arc<Owner>);

struct ReadOperation {
    owner: Arc<Owner>,
    initialized: bool,
    value: String,
    attempts: usize,
    cleanups: usize,
    saved: Option<Context>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Operation for ReadOperation {
    type Declaration = Read;
    type Resources = Owner;
    type Error = std::io::Error;
    type Endpoints = ();
    type Context<'a> = &'a Context;
    fn new(owner: Arc<Owner>) -> Result<Self, Self::Error> {
        Ok(Self {
            owner,
            initialized: false,
            value: String::new(),
            attempts: 0,
            cleanups: 0,
            saved: None,
        })
    }
    async fn initialize<'ctx>(&mut self, context: Self::Context<'ctx>) -> Result<(), Self::Error> {
        assert!(!self.initialized, "operation initialized only once");
        self.initialized = true;
        self.owner.initialized.fetch_add(1, Ordering::SeqCst);
        if self.owner.fail_initialize.swap(false, Ordering::SeqCst) {
            return Err(std::io::Error::other("partial initialization failed"));
        }
        if self.owner.child_initialize.swap(false, Ordering::SeqCst) {
            let child = context
                .operation::<Read>(Value {
                    value: "leaf".into(),
                })
                .await
                .map_err(std::io::Error::other)?;
            assert_eq!(child.value, "leaf:1");
            self.saved = Some(context.clone());
        }
        Ok(())
    }
    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        value: &mut Value,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_guest::operations::Response<Value>, Self::Error> {
        let result: ::core::result::Result<Value, Self::Error> = async move {
            assert!(self.initialized, "initialization precedes attempts");
            self.value.clone_from(&value.value);
            self.owner.ran.fetch_add(1, Ordering::SeqCst);
            if self.value == "initialize-child" {
                let saved = self
                    .saved
                    .as_ref()
                    .expect("original initialization context");
                if saved
                    .operation::<Read>(Value {
                        value: "leaf".into(),
                    })
                    .await
                    .is_ok()
                {
                    return Err(std::io::Error::other(
                        "initialization context remained active",
                    ));
                }
            }
            if self.value == "context-leaf" {
                let stage = context
                    .context()
                    .value::<contract::Stage>()
                    .map_err(std::io::Error::other)?
                    .unwrap();
                return Ok(Value { value: stage.id });
            }
            if self.value == "context-parent" {
                let parent = context
                    .context()
                    .value::<contract::Stage>()
                    .map_err(std::io::Error::other)?
                    .unwrap();
                assert_eq!(parent.id, "parent");
                let caller = context
                    .with_value(contract::Stage { id: "child".into() })
                    .map_err(std::io::Error::other)?;
                let child = caller
                    .operation::<Read>(Value {
                        value: "context-leaf".into(),
                    })
                    .await
                    .map_err(std::io::Error::other)?;
                let original = context
                    .context()
                    .value::<contract::Stage>()
                    .map_err(std::io::Error::other)?
                    .unwrap();
                return Ok(Value {
                    value: format!("{}:{}", original.id, child.value),
                });
            }
            self.attempts += 1;
            if self.value == "leaf" {
                return Ok(Value {
                    value: "leaf:1".into(),
                });
            }
            if self.value == "parent" {
                if let Some(saved) = &self.saved
                    && saved
                        .operation::<Read>(Value {
                            value: "leaf".into(),
                        })
                        .await
                        .is_ok()
                {
                    return Err(std::io::Error::other("stale context was accepted"));
                }
                let child = context
                    .operation::<Read>(Value {
                        value: "leaf".into(),
                    })
                    .await
                    .map_err(std::io::Error::other)?;
                if self.attempts > 1 {
                    return Ok(Value {
                        value: format!("parent:2:stale:{}", child.value),
                    });
                }
                self.saved = Some(context.clone());
            }
            if self.attempts == 1 {
                return Err(std::io::Error::other("first transient attempt"));
            }
            Ok(Value {
                value: format!("{}:{}", self.value, self.attempts),
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.owner.stopped.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn finalize(&mut self) -> Result<(), Self::Error> {
        self.owner.finalized.fetch_add(1, Ordering::SeqCst);
        self.cleanups += 1;
        if self.value == "cleanup" && self.cleanups == 1 {
            return Err(std::io::Error::other("first cleanup retained"));
        }
        Ok(())
    }
    fn classify(_: &Self::Error) -> Failure {
        Failure::Transient
    }
}

impl Operation for Counting {
    type Declaration = Counts;
    type Resources = Owner;
    type Error = std::io::Error;
    type Endpoints = ();
    type Context<'a> = &'a Context;

    fn new(owner: Arc<Owner>) -> Result<Self, Self::Error> {
        Ok(Self(owner))
    }

    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        _: &mut Value,
        _: &mut Self::Endpoints,
    ) -> Result<harmos_guest::operations::Response<Value>, Self::Error> {
        let result: ::core::result::Result<Value, Self::Error> = async move {
            Ok(Value {
                value: format!(
                    "{}:{}:{}:{}",
                    self.0.initialized.load(Ordering::SeqCst),
                    self.0.ran.load(Ordering::SeqCst),
                    self.0.stopped.load(Ordering::SeqCst),
                    self.0.finalized.load(Ordering::SeqCst)
                ),
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl lifecycle::Policy for Program {
    const INITIALIZATION_POLICY: OperationPolicy = SDK_POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = SDK_POLICY;
}

impl Declared for Program {
    const ID: &'static str = "operation-guest";
    const VERSION: &'static str = "1.0.0";
    const DESCRIPTION: &'static str = "Retained operation fixture.";
    const PROVIDES: &'static [Node<WireDeclaration>] = &[
        Node::Leaf(WireDeclaration::of::<Read>()),
        Node::Leaf(WireDeclaration::of::<Counts>()),
        Node::Leaf(WireDeclaration::of::<contract::Upload>()),
        Node::Leaf(WireDeclaration::of::<contract::Download>()),
        Node::Leaf(WireDeclaration::of::<contract::Exchange>()),
    ];
    const REQUIRES: &'static [Node<harmos_contract::plugin::Requirement>] = &[];
    const STREAMS: &'static [publications::Entry] = &[];
    fn markers() -> Result<Vec<&'static [u8]>, harmos_contract::schema::graph::Error> {
        harmos_contract::plugin::schema_markers::<Self>(
            vec![&IDENTITY, &CONTRACTS, &PUBLICATIONS, &LISTENERS, &OBSERVERS],
            &harmos_contract::config::SchemaSettings::default(),
        )
    }
}

impl Plugin for Program {
    type Config = String;
    type Error = std::io::Error;
    type Context = Context;
    type Resources = Owner;
    fn prepare(config: String) -> Result<Self, Self::Error> {
        Ok(Self {
            owner: Arc::new(Owner {
                fail_initialize: AtomicBool::new(config == "initialize-fail"),
                child_initialize: AtomicBool::new(config == "initialize-child"),
                ..Owner::default()
            }),
            mismatch: config == "mismatch",
            pending_shutdown: config == "shutdown-pending",
        })
    }
    fn resources(&self) -> Arc<Owner> {
        Arc::clone(&self.owner)
    }
    async fn initialize(&mut self, _: &Context) -> Result<(), Self::Error> {
        Ok(())
    }
    fn bind_operations(&self, table: &mut WireTable<Context>) -> Result<(), ServingError> {
        if !self.mismatch {
            table.bind_operation::<ReadOperation>(self.resources())?;
            table.bind_operation::<Counting>(self.resources())?;
            streams::bind(table, self.resources())?;
        }
        Ok(())
    }
    async fn shutdown(&mut self, _: &Context) -> Result<(), Self::Error> {
        if self.pending_shutdown {
            std::future::pending::<()>().await;
        }
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
