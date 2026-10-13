/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "test_component/contract.rs"]
mod contract;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
// Third Party Imports

// Local Imports
use contract::{Heard, Metrics, NOTE, Note, POLICY, Progress};
use harmos_contract::catalog::Node;
use harmos_contract::inventory::{contracts, listeners, observers, publications};
use harmos_contract::operations::{
    Operation, OperationPolicy, ServingError, WireDeclaration, WireTable, host, lifecycle,
};
use harmos_contract::plugin::{Declared, Grant, Plugin, Requirement};
use harmos_contract::schema::Described;
use harmos_contract::{Refusal, StreamSpec, listeners::Delivery};
use harmos_guest::Context;
use harmos_guest::Host;

/* === Constants === */
const REQUIRED: &[Node<Requirement>] = &[
    Node::Leaf(Requirement::of::<host::Publish>(&[])),
    Node::Leaf(Requirement::of::<host::RecordSelf>(&[])),
    Node::Leaf(Requirement::of::<host::RecallSelf>(&[])),
    Node::Leaf(Requirement::of::<host::Invoke>(&[])),
    Node::Leaf(Requirement::of::<host::Egress>(&[Grant::Egress(
        Cow::Borrowed("vendor"),
    )])),
    Node::Leaf(Requirement::of::<host::Survey>(&[Grant::Survey(
        Cow::Borrowed("capture"),
    )])),
];
const PROVIDED: &[Node<WireDeclaration>] = &[Node::Leaf(WireDeclaration::of::<Heard>())];
const STREAMS: &[publications::Entry] = &[
    publications::Entry {
        id: Progress::ID,
        schema: Progress::VERSION,
        window: Progress::WINDOW,
        row: Progress::DEFINITION,
        metadata_json: "",
        row_metadata_json: "[]",
    },
    publications::Entry {
        id: Metrics::ID,
        schema: Metrics::VERSION,
        window: Metrics::WINDOW,
        row: Metrics::DEFINITION,
        metadata_json: "",
        row_metadata_json: "[]",
    },
];
const LISTENERS: &[Node<harmos_contract::listeners::Entry>] =
    &[Node::Leaf(harmos_contract::listeners::Entry {
        id: "fixture.notes",
        source: harmos_contract::listeners::SourceDeclaration::Record {
            id: NOTE,
            version: 1,
            row: Note::DEFINITION,
        },
        callback: WireDeclaration::of::<Heard>(),
    })];
const SDK: [harmos_contract::operations::OperationPolicy; 2] = [POLICY, POLICY];

#[used]
static IDENTITY: [u8; contracts::identity_len(
    "guest",
    TestComponent::ID,
    TestComponent::VERSION,
    TestComponent::DESCRIPTION,
    "String",
)] = contracts::encode_identity(
    "guest",
    TestComponent::ID,
    TestComponent::VERSION,
    TestComponent::DESCRIPTION,
    "String",
);

#[used]
static CONTRACTS: [u8; contracts::encoded_tree_len(
    "guest",
    TestComponent::ID,
    PROVIDED,
    REQUIRED,
    &SDK,
)] = contracts::encode_tree("guest", TestComponent::ID, PROVIDED, REQUIRED, &SDK);

#[used]
static PUBLICATIONS: [u8; publications::encoded_len("guest", TestComponent::ID, STREAMS)] =
    publications::encode("guest", TestComponent::ID, STREAMS);

#[used]
static AUDIENCE: [u8; listeners::encoded_len("guest", TestComponent::ID, LISTENERS)] =
    listeners::encode("guest", TestComponent::ID, LISTENERS);

#[used]
static OBSERVERS: [u8; observers::encoded_tree_len("guest", TestComponent::ID, &[], &[])] =
    observers::encode_tree("guest", TestComponent::ID, &[], &[]);

/* === Macros === */
harmos_guest::export_plugin!(TestComponent, capacity = 2);

/* === Type Aliases === */

/* === Structs === */
struct TestComponent {
    config: String,
    deactivate: u8,
    resources: Arc<Resources>,
}

struct Resources {
    refuse_delivery: AtomicBool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl lifecycle::Policy for TestComponent {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

impl Plugin for TestComponent {
    fn register(
        registrar: &mut harmos_guest::Registrar,
    ) -> Result<(), harmos_guest::RegistrationError> {
        registrar.register(
            "example.catalog",
            1,
            &serde_json::json!({ "suite": "guest" }),
        )
    }

    type Config = String;
    type Error = std::io::Error;
    type Context = Context;
    type Resources = Resources;
    fn prepare(config: String) -> Result<Self, Self::Error> {
        Ok(Self {
            config,
            deactivate: 0,
            resources: Arc::new(Resources {
                refuse_delivery: AtomicBool::new(false),
            }),
        })
    }
    fn resources(&self) -> Arc<Resources> {
        Arc::clone(&self.resources)
    }
    fn bind_operations(&self, table: &mut WireTable<Context>) -> Result<(), ServingError> {
        table.bind_operation::<Hearing>(self.resources())
    }
    async fn initialize(&mut self, context: &Context) -> Result<(), Self::Error> {
        self.initialize_mode(context)
            .await
            .map_err(std::io::Error::other)
    }
    async fn stop(&mut self, context: &Context) -> Result<(), Self::Error> {
        if self.deactivate == 1 {
            let result = context
                .call_as::<host::Invoke>(host::InvokeRequest {
                    grant: "counter.increment".into(),
                    key: "after-revoke".into(),
                    payload: br#"{"amount":1}"#.to_vec(),
                })
                .await;
            match result {
                Err(_) => {}
                Ok(_) => {
                    return Err(std::io::Error::other(
                        "revocation must close admission before shutdown",
                    ));
                }
            }
        }
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        if self.deactivate == 2 {
            spin_forever();
        }
        Ok(())
    }
}

impl TestComponent {
    async fn initialize_mode(&mut self, core: &Context) -> Result<(), String> {
        let config = self.config.clone();
        match config.as_str() {
            "idle" => Ok(()),
            "invoke" => invoke(core, "accepted").await,
            "revoke" => {
                self.deactivate = 1;
                invoke(core, "admitted").await
            }
            // A key nobody has used before, taken from the position the guest's
            // own fact landed at. An idempotent retry of an earlier key would
            // answer with the earlier receipt and never reach the application,
            // so a fresh key is how a guest asks the application the question
            // again.
            "invoke-fresh" => {
                let receipt = record(core, "finding", 1, br#"{"message":"fresh"}"#)
                    .await
                    .map_err(refusal)?;
                invoke_request(
                    core,
                    "counter.increment",
                    &format!("fresh-{}", receipt.position),
                    br#"{"amount":1}"#,
                )
                .await
                .map(|_| ())
                .map_err(refusal)
            }
            "undeclared" => invoke_request(core, "counter.denied", "denied", br#"{"amount":1}"#)
                .await
                .map(|_| ())
                .map_err(refusal),
            "record" => record(core, "finding", 1, br#"{"message":"landed"}"#)
                .await
                .map(|_| ())
                .map_err(refusal),
            // Its own history, read back through its own namespace: the whole
            // page, the page after the first fact, and a page the guest asked
            // to be shorter than either.
            "recall" => {
                let first = record(core, "finding", 1, br#"{"message":"one"}"#)
                    .await
                    .map_err(refusal)?;
                record(core, "finding", 1, br#"{"message":"two"}"#)
                    .await
                    .map_err(refusal)?;
                let all = recall(core, "finding", 0, 10).await.map_err(refusal)?;
                let rest = recall(core, "finding", first.position, 10)
                    .await
                    .map_err(refusal)?;
                let bounded = recall(core, "finding", 0, 1).await.map_err(refusal)?;
                if (all.len(), rest.len(), bounded.len()) != (2, 1, 1) {
                    return Err(format!(
                        "recalled {} then {} bounded {}",
                        all.len(),
                        rest.len(),
                        bounded.len()
                    ));
                }
                if all[0].name != "finding"
                    || all[0].version != 1
                    || all[0].payload != br#"{"message":"one"}"#
                    || all[1].position <= all[0].position
                {
                    return Err("the recalled page is not the history that was written".into());
                }
                finding(core, "recalled").await
            }
            // The namespace is the host's to supply, so a name that carries one
            // is refused for carrying it.
            "recall-elsewhere" => recall(core, "guest:other/finding", 0, 8)
                .await
                .map(|_| ())
                .map_err(refusal),
            // One recall with distinctive arguments, for the world test's stub
            // to record and answer: what is under test there is the crossing.
            "recall-wire" => {
                let page = recall(core, "finding", 3, 5).await.map_err(refusal)?;
                let [one] = page.as_slice() else {
                    return Err(format!("one remembered, not {}", page.len()));
                };
                if one.name != "finding"
                    || one.version != 2
                    || one.position != 9
                    || one.payload != br#"{"message":"kept"}"#
                {
                    return Err("the remembered fact did not cross intact".into());
                }
                Ok(())
            }
            "submit" => submitted(core).await,
            "publish" => {
                let publisher = core.publisher::<TestComponent>();
                publisher
                    .publish_on::<Progress>(&Progress { percent: 25 })
                    .await
                    .map_err(refusal)?;
                publisher
                    .publish_on::<Metrics>(&Metrics { count: 3 })
                    .await
                    .map_err(refusal)
            }
            "survey" => {
                let answered = survey(core, "capture", 1, 8, "mean")
                    .await
                    .map_err(refusal)?;
                let [measured] = answered.as_slice() else {
                    return Err(format!("one measured field, not {}", answered.len()));
                };
                core.publisher::<TestComponent>()
                    .publish_on::<Metrics>(&Metrics {
                        count: measured.value as i64,
                    })
                    .await
                    .map_err(refusal)
            }
            // `median` is a real statistic and deliberately not a name in the
            // vocabulary: the median is `p50`, and one statistic has one
            // spelling. A host that answered this anyway would have two.
            "survey-unsupported" => survey(core, "capture", 1, 8, "median")
                .await
                .map(|_| ())
                .map_err(refusal),
            "survey-undeclared" => survey(core, "unknown", 1, 8, "mean")
                .await
                .map(|_| ())
                .map_err(refusal),
            // One request the host performs, whose answer the guest writes
            // into its own history — so what a test reads is what crossed
            // back, not what a stub was told to say.
            "egress" => {
                let answered = egress(core, "vendor", "get", "/reading", &[], b"")
                    .await
                    .map_err(refusal)?;
                if answered.status != 200 {
                    return Err(format!("the vendor answered {}", answered.status));
                }
                let body = std::str::from_utf8(&answered.body)
                    .map_err(|_| "the vendor answers text".to_owned())?;
                finding(core, body).await
            }
            // A path that would leave the family an operator answered for.
            "egress-escape" => egress(core, "vendor", "get", "/../elsewhere", &[], b"")
                .await
                .map(|_| ())
                .map_err(refusal),
            // An answer larger than a guest could be handed.
            "egress-large" => egress(core, "vendor", "get", "/large", &[], b"")
                .await
                .map(|_| ())
                .map_err(refusal),
            // An answer that never comes inside the host's own deadline.
            "egress-slow" => egress(core, "vendor", "get", "/slow", &[], b"")
                .await
                .map(|_| ())
                .map_err(refusal),
            // A family this manifest does not declare.
            "egress-undeclared" => egress(core, "other", "get", "/reading", &[], b"")
                .await
                .map(|_| ())
                .map_err(refusal),
            // Subscribed and about to refuse whatever arrives, so the host's
            // answer to a guest that cannot handle its own subscription is
            // observable from the outside.
            "refuse-delivery" => {
                self.resources
                    .refuse_delivery
                    .store(true, Ordering::Release);
                Ok(())
            }
            "trap" => panic!("requested guest trap"),
            "fuel" => spin_forever(),
            "hang-deactivate" => {
                self.deactivate = 2;
                Ok(())
            }
            // Anything else is an unknown mode and refused — which is also how
            // a test asks a guest to reject a configuration.
            other => match echoed(other) {
                Some(message) => finding(core, message).await,
                None => Err("unknown test mode".to_owned()),
            },
        }
    }
}

impl Declared for TestComponent {
    const ID: &'static str = "test-component";
    const VERSION: &'static str = "1.0.0";
    const DESCRIPTION: &'static str = "Guest capability and lifecycle fixture.";
    const PROVIDES: &'static [Node<WireDeclaration>] = PROVIDED;
    const REQUIRES: &'static [Node<Requirement>] = REQUIRED;
    const LISTENERS: &'static [Node<harmos_contract::listeners::Entry>] = LISTENERS;
    const STREAMS: &'static [publications::Entry] = STREAMS;
    fn markers() -> Result<Vec<&'static [u8]>, harmos_contract::schema::graph::Error> {
        harmos_contract::plugin::schema_markers::<Self>(
            vec![&IDENTITY, &CONTRACTS, &PUBLICATIONS, &AUDIENCE, &OBSERVERS],
            &harmos_contract::config::SchemaSettings::default(),
        )
    }
}

struct Hearing {
    resources: Arc<Resources>,
}

impl Operation for Hearing {
    type Declaration = Heard;
    type Resources = Resources;
    type Error = std::io::Error;
    type Endpoints = ();
    type Context<'a> = &'a Context;
    async fn initialize<'ctx>(
        resources: Arc<Resources>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Self::Error> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        delivery: &mut Delivery<Note>,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_guest::operations::Response<()>, Self::Error> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            if self.resources.refuse_delivery.load(Ordering::Acquire) {
                return Err(std::io::Error::other(
                    "this delivery is one the guest refuses",
                ));
            }
            finding(context, &format!("{} {}", NOTE, delivery.row().note))
                .await
                .map_err(std::io::Error::other)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn invoke(core: &Context, key: &str) -> Result<(), String> {
    invoke_request(core, "counter.increment", key, br#"{"amount":1}"#)
        .await
        .map(|_| ())
        .map_err(refusal)
}

fn echoed(config: &str) -> Option<&str> {
    config.strip_prefix("echo:")
}

async fn finding(core: &Context, message: &str) -> Result<(), String> {
    record(
        core,
        "finding",
        1,
        format!(r#"{{"message":"{message}"}}"#).as_bytes(),
    )
    .await
    .map(|_| ())
    .map_err(refusal)
}

async fn record(
    core: &Context,
    name: &str,
    version: u32,
    payload: &[u8],
) -> Result<host::Receipt, Refusal> {
    core.call_as::<host::RecordSelf>(host::RecordRequest {
        name: name.into(),
        version,
        payload: payload.to_vec(),
    })
    .await
    .map(harmos_guest::operations::Response::into_owned)
}

async fn invoke_request(
    core: &Context,
    grant: &str,
    key: &str,
    payload: &[u8],
) -> Result<host::Receipt, Refusal> {
    core.call_as::<host::Invoke>(host::InvokeRequest {
        grant: grant.into(),
        key: key.into(),
        payload: payload.to_vec(),
    })
    .await
    .map(harmos_guest::operations::Response::into_owned)
}

async fn recall(
    core: &Context,
    name: &str,
    after: u64,
    limit: u32,
) -> Result<Vec<host::Remembered>, Refusal> {
    core.call_as::<host::RecallSelf>(host::RecallRequest {
        name: name.into(),
        after,
        limit,
    })
    .await
    .map(|response| response.into_owned().rows)
}

async fn survey(
    core: &Context,
    stream: &str,
    first: u64,
    last: u64,
    aggregate: &str,
) -> Result<Vec<host::Measured>, Refusal> {
    core.call_as::<host::Survey>(host::SurveyRequest {
        stream: stream.into(),
        first,
        last,
        aggregate: aggregate.into(),
    })
    .await
    .map(|response| response.into_owned().rows)
}

async fn egress(
    core: &Context,
    name: &str,
    method: &str,
    path: &str,
    headers: &[host::Header],
    body: &[u8],
) -> Result<host::Fetched, Refusal> {
    core.call_as::<host::Egress>(host::EgressRequest {
        name: name.into(),
        method: method.into(),
        path: path.into(),
        headers: headers.to_vec(),
        body: body.to_vec(),
    })
    .await
    .map(harmos_guest::operations::Response::into_owned)
}

// Unary children through the guest binding: status and join, cancel and join,
// a dropped handle and a detached one.
async fn submitted(core: &Context) -> Result<(), String> {
    use harmos_guest::operations::Instance;
    use harmos_guest::operations::instance::Phase;
    let request = |key: &str| host::InvokeRequest {
        grant: "counter.increment".into(),
        key: key.into(),
        payload: br#"{"amount":1}"#.to_vec(),
    };

    let joined = core
        .submit_as::<host::Invoke>(request("joined"))
        .await
        .map_err(refusal)?;
    let status = joined.status().await.map_err(refusal)?;
    if status.phase != Phase::Running {
        return Err(format!("a live child reported {:?}", status.phase));
    }
    let receipt = joined.join().await.map_err(refusal)?;
    if receipt.position != 1 {
        return Err(format!("joined receipt {}", receipt.position));
    }

    let cancelled = core
        .submit_as::<host::Invoke>(request("cancelled"))
        .await
        .map_err(refusal)?;
    if Instance::cancel_and_join(cancelled).await.is_ok() {
        return Err("a cancelled child must not answer".into());
    }

    let dropped = core
        .submit_as::<host::Invoke>(request("dropped"))
        .await
        .map_err(refusal)?;
    drop(dropped);

    let detached = core
        .submit_as::<host::Invoke>(request("detached"))
        .await
        .map_err(refusal)?;
    Instance::detach(detached);
    Ok(())
}

fn refusal(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn spin_forever() -> ! {
    loop {
        std::hint::spin_loop();
    }
}

/* === Tests === */
