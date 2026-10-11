/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "route_component/contract.rs"]
mod contract;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use std::time::Duration;
// Third Party Imports

// Local Imports
use contract::{Decline, Echo, Echoed, Fail, ProviderEcho, Spin, Wait};
use harmos_contract::StreamSpec;
use harmos_contract::catalog::Node;
use harmos_contract::inventory::{contracts, listeners, observers, publications};
use harmos_contract::operations::{
    Operation, OperationPolicy, RetryPolicy, ServingError, WireDeclaration, WireTable, host,
    lifecycle,
};
use harmos_contract::plugin::{Declared, Grant, Plugin, Requirement};
use harmos_contract::schema::Described;
use harmos_guest::{Context, Message, OperationRefusal, PublishError};

/* === Constants === */
const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    retries: RetryPolicy::Never,
};
const PROVIDED: &[Node<WireDeclaration>] = &[
    Node::Leaf(WireDeclaration::of::<Echo>()),
    Node::Leaf(WireDeclaration::of::<Decline>()),
    Node::Leaf(WireDeclaration::of::<Spin>()),
    Node::Leaf(WireDeclaration::of::<Wait>()),
    Node::Leaf(WireDeclaration::of::<Fail>()),
];
const REQUIRED: &[Node<Requirement>] = &[
    Node::Leaf(Requirement::of::<ProviderEcho>(&[])),
    Node::Leaf(Requirement::of::<host::Invoke>(&[])),
    Node::Leaf(Requirement::of::<host::Publish>(&[])),
    Node::Leaf(Requirement::of::<host::Egress>(&[Grant::Egress(
        Cow::Borrowed("vendor"),
    )])),
];
const STREAMS: &[publications::Entry] = &[publications::Entry {
    id: Echoed::ID,
    schema: Echoed::VERSION,
    window: Echoed::WINDOW,
    fields: Echoed::FIELDS,
    metadata_json: "",
    row_metadata_json: "[]",
}];
const SDK: [harmos_contract::operations::OperationPolicy; 2] = [POLICY, POLICY];

#[used]
static IDENTITY: [u8; contracts::identity_len(
    "guest",
    RouteComponent::ID,
    RouteComponent::VERSION,
    RouteComponent::DESCRIPTION,
    "String",
)] = contracts::encode_identity(
    "guest",
    RouteComponent::ID,
    RouteComponent::VERSION,
    RouteComponent::DESCRIPTION,
    "String",
);

#[used]
static CONTRACTS: [u8; contracts::encoded_tree_len(
    "guest",
    RouteComponent::ID,
    PROVIDED,
    REQUIRED,
    &SDK,
)] = contracts::encode_tree("guest", RouteComponent::ID, PROVIDED, REQUIRED, &SDK);

#[used]
static PUBLICATIONS: [u8; publications::encoded_len("guest", RouteComponent::ID, STREAMS)] =
    publications::encode("guest", RouteComponent::ID, STREAMS);

#[used]
static LISTENERS: [u8; listeners::encoded_len("guest", RouteComponent::ID, &[])] =
    listeners::encode("guest", RouteComponent::ID, &[]);

#[used]
static OBSERVERS: [u8; observers::encoded_tree_len("guest", RouteComponent::ID, &[], &[])] =
    observers::encode_tree("guest", RouteComponent::ID, &[], &[]);

// Co-Linked Inventories Do Not Grant This Owner Or Facility Any Authority
static LINKED_CONTRACTS: [&[u8]; 2] = {
    const LINKED: &[Node<WireDeclaration>] = &[Node::Leaf(WireDeclaration::of::<ProviderEcho>())];
    #[used]
    static NATIVE: [u8; contracts::encoded_tree_len("sidecar", "route-guest", LINKED, &[], &SDK)] =
        contracts::encode_tree("sidecar", "route-guest", LINKED, &[], &SDK);
    #[used]
    static OTHER: [u8; contracts::encoded_tree_len("guest", "other", LINKED, &[], &SDK)] =
        contracts::encode_tree("guest", "other", LINKED, &[], &SDK);
    [&NATIVE, &OTHER]
};
static LINKED_PUBLICATIONS: [&[u8]; 2] = {
    use harmos_guest::inventory::publications::{Entry, encode, encoded_len};
    const STREAMS: &[Entry] = &[Entry {
        metadata_json: "",
        row_metadata_json: "[]",
        id: "foreign",
        schema: 1,
        window: 8,
        fields: &[],
    }];
    #[used]
    static OTHER: [u8; encoded_len("guest", "other", STREAMS)] = encode("guest", "other", STREAMS);
    #[used]
    static NATIVE: [u8; encoded_len("sidecar", "route-guest", STREAMS)] =
        encode("sidecar", "route-guest", STREAMS);
    [&OTHER, &NATIVE]
};

/* === Macros === */
harmos_guest::export_plugin!(RouteComponent, capacity = 5);

/* === Type Aliases === */

/* === Structs === */
struct RouteComponent {
    config: String,
    check_publication_revocation: bool,
    check_dependency_revocation: bool,
    owner: Arc<Owner>,
}

#[derive(Default)]
struct Owner {
    initialized: Mutex<Option<Context>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl lifecycle::Policy for RouteComponent {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

impl Declared for RouteComponent {
    const ID: &'static str = "route-guest";
    const VERSION: &'static str = "1.0.0";
    const DESCRIPTION: &'static str = "Answers the harness's operation probes.";
    const PROVIDES: &'static [Node<WireDeclaration>] = PROVIDED;
    const REQUIRES: &'static [Node<Requirement>] = REQUIRED;
    const STREAMS: &'static [publications::Entry] = STREAMS;
    fn markers() -> Vec<&'static [u8]> {
        vec![
            &IDENTITY,
            &CONTRACTS,
            &PUBLICATIONS,
            &LISTENERS,
            &OBSERVERS,
            LINKED_CONTRACTS[0],
            LINKED_CONTRACTS[1],
            LINKED_PUBLICATIONS[0],
            LINKED_PUBLICATIONS[1],
        ]
    }
}

impl Plugin for RouteComponent {
    type Config = String;
    type Error = std::io::Error;
    type Context = Context;
    type Resources = Owner;
    fn prepare(config: String) -> Result<Self, Self::Error> {
        Ok(Self {
            config,
            check_publication_revocation: false,
            check_dependency_revocation: false,
            owner: Arc::new(Owner::default()),
        })
    }
    fn resources(&self) -> Arc<Owner> {
        Arc::clone(&self.owner)
    }
    fn bind_operations(&self, table: &mut WireTable<Context>) -> Result<(), ServingError> {
        table.bind_operation::<Echoing>(self.resources())?;
        table.bind_operation::<Declining>(self.resources())?;
        table.bind_operation::<Spinning>(self.resources())?;
        table.bind_operation::<Waiting>(self.resources())?;
        table.bind_operation::<Failing>(self.resources())
    }
    async fn initialize(&mut self, context: &Context) -> Result<(), Self::Error> {
        *self
            .owner
            .initialized
            .lock()
            .map_err(|error| std::io::Error::other(error.to_string()))? = Some(context.clone());
        if self.config == "publications" || self.config == "failed-publications" {
            progress(context, "initialize")
                .await
                .map_err(std::io::Error::other)?;
            if self.config == "failed-publications" {
                return Err(std::io::Error::other(
                    "initialization refused after publication",
                ));
            }
            self.check_publication_revocation = true;
        }
        if self.config == "dependencies" {
            context
                .operation::<ProviderEcho>(Echo {
                    say: "initialize".into(),
                })
                .await
                .map_err(std::io::Error::other)?;
            self.check_dependency_revocation = true;
        }
        Ok(())
    }
    async fn shutdown(&mut self, _: &Context) -> Result<(), Self::Error> {
        let context = self
            .owner
            .initialized
            .lock()
            .map_err(|error| std::io::Error::other(error.to_string()))?
            .clone()
            .expect("original initialization context is retained");
        if self.check_publication_revocation {
            let result = progress(&context, "cleanup").await;
            if !matches!(result, Err(PublishError::Call(_))) {
                return Err(std::io::Error::other(
                    "retirement must refuse a new publication",
                ));
            }
        }
        if self.check_dependency_revocation {
            let result = context
                .operation::<ProviderEcho>(Echo {
                    say: "cleanup".into(),
                })
                .await;
            if result.is_ok() {
                return Err(std::io::Error::other(
                    "retirement must refuse a new dependency call",
                ));
            }
        }
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.owner
            .initialized
            .lock()
            .map_err(|error| std::io::Error::other(error.to_string()))?
            .take();
        Ok(())
    }
}

struct Echoing {
    resources: Arc<Owner>,
}

impl Operation for Echoing {
    type Declaration = Echo;
    type Resources = Owner;
    type Error = OperationRefusal;
    type Endpoints = ();
    type Context<'a> = &'a Context;
    fn new(resources: Arc<Owner>) -> Result<Self, OperationRefusal> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut Echo,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<Echoed, OperationRefusal> {
        if let Some(value) = request.say.strip_prefix("saved-publish:") {
            let saved = self
                .resources
                .initialized
                .lock()
                .map_err(OperationRefusal::of)?
                .clone()
                .ok_or_else(|| OperationRefusal::of("SDK initialization context is absent"))?;
            progress(&saved, value)
                .await
                .map_err(OperationRefusal::of)?;
            return Ok(Echoed {
                said: format!("published {value}"),
            });
        }
        if let Some(value) = request.say.strip_prefix("saved-dependency:") {
            let saved = self
                .resources
                .initialized
                .lock()
                .map_err(OperationRefusal::of)?
                .clone()
                .ok_or_else(|| OperationRefusal::of("SDK initialization context is absent"))?;
            return saved
                .operation::<ProviderEcho>(Echo { say: value.into() })
                .await;
        }

        if let Some(say) = request.say.strip_prefix("dependency:") {
            return context
                .operation::<ProviderEcho>(Echo {
                    say: say.to_owned(),
                })
                .await;
        }

        if let Some(value) = request.say.strip_prefix("invoke:") {
            let mut fields = value.splitn(3, ':');
            let grant = fields.next().unwrap_or_default();
            let key = fields.next().unwrap_or_default();
            let payload = fields.next().unwrap_or_default();
            let answered = context
                .operation::<host::Invoke>(host::InvokeRequest {
                    grant: grant.into(),
                    key: key.into(),
                    payload: payload.as_bytes().to_vec(),
                })
                .await;
            return Ok(Echoed {
                said: match answered {
                    Ok(receipt) => format!("receipt:{}:{}", receipt.position, receipt.correlation),
                    Err(error) => error.to_string(),
                },
            });
        }

        if let Some(name) = request.say.strip_prefix("egress:") {
            let answered = context
                .operation::<host::Egress>(host::EgressRequest {
                    name: name.into(),
                    method: "get".into(),
                    path: "/reading".into(),
                    headers: vec![],
                    body: vec![],
                })
                .await;
            return Ok(Echoed {
                said: match answered {
                    Ok(response) => format!("egress status {}", response.status),
                    Err(error) => error.to_string(),
                },
            });
        }

        if let Some(value) = request.say.strip_prefix("publish:") {
            progress(context, value)
                .await
                .map_err(OperationRefusal::of)?;
        }
        if request.say == "publish-many" {
            for index in 0..8 {
                progress(context, &index.to_string())
                    .await
                    .map_err(OperationRefusal::of)?;
            }
        }
        if request.say == "publish-foreign" {
            let refused = context
                .operation::<host::Publish>(host::Publication {
                    stream: "foreign".into(),
                    schema: 1,
                    sequence: None,
                    published_unix_micros: None,
                    context: None,
                    payload: ().encode_to_vec(),
                })
                .await
                .expect_err("co-linked declarations grant nothing");
            return Ok(Echoed {
                said: refused.to_string(),
            });
        }
        if request.say == "grow" {
            std::hint::black_box(vec![1u8; 128 * 1024 * 1024]);
        }
        Ok(Echoed {
            said: format!("heard {}", request.say),
        })
    }
}

struct Declining {
    _resources: Arc<Owner>,
}

impl Operation for Declining {
    type Declaration = Decline;
    type Resources = Owner;
    type Error = OperationRefusal;
    type Endpoints = ();
    type Context<'a> = &'a Context;
    fn new(resources: Arc<Owner>) -> Result<Self, OperationRefusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        _context: Self::Context<'ctx>,
        _request: &mut Decline,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<Echoed, OperationRefusal> {
        Err(OperationRefusal::new(7, "the probe declines"))
    }
}

struct Spinning {
    _resources: Arc<Owner>,
}

impl Operation for Spinning {
    type Declaration = Spin;
    type Resources = Owner;
    type Error = OperationRefusal;
    type Endpoints = ();
    type Context<'a> = &'a Context;
    fn new(resources: Arc<Owner>) -> Result<Self, OperationRefusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        _context: Self::Context<'ctx>,
        _request: &mut Spin,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<Echoed, OperationRefusal> {
        let mut counted = 0u64;
        loop {
            counted = counted.wrapping_add(1);
            std::hint::black_box(counted);
        }
    }
}

struct Waiting {
    _resources: Arc<Owner>,
}

impl Operation for Waiting {
    type Declaration = Wait;
    type Resources = Owner;
    type Error = OperationRefusal;
    type Endpoints = ();
    type Context<'a> = &'a Context;
    fn new(resources: Arc<Owner>) -> Result<Self, OperationRefusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        _request: &mut Wait,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<Echoed, OperationRefusal> {
        // The host answers this slowly. Nothing here burns fuel while
        // it waits, so the bound this call blew is wall-clock alone.
        let _ = context
            .operation::<host::Egress>(host::EgressRequest {
                name: "vendor".into(),
                method: "get".into(),
                path: "/slow".into(),
                headers: vec![],
                body: vec![],
            })
            .await;
        // Resuming is where the epoch check the guest ran past is
        // reached: a suspended guest executes no instrumented code.
        for step in 0..4096u32 {
            std::hint::black_box(step);
        }
        Ok(Echoed {
            said: "waited".to_owned(),
        })
    }
}

struct Failing {
    _resources: Arc<Owner>,
}

impl Operation for Failing {
    type Declaration = Fail;
    type Resources = Owner;
    type Error = OperationRefusal;
    type Endpoints = ();
    type Context<'a> = &'a Context;
    fn new(resources: Arc<Owner>) -> Result<Self, OperationRefusal> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        _context: Self::Context<'ctx>,
        _request: &mut Fail,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<Echoed, OperationRefusal> {
        panic!("requested guest trap")
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn progress(context: &Context, value: &str) -> Result<(), PublishError> {
    context
        .publisher::<RouteComponent>()
        .publish_on::<Echoed>(&Echoed { said: value.into() })
        .await
}

/* === Tests === */
