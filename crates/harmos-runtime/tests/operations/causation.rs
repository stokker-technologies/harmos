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
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Third Party Imports
use tracing::{Instrument, Subscriber, instrument::WithSubscriber};
use tracing_subscriber::{Layer, layer::Context as TraceContext, prelude::*, registry::LookupSpan};

// Local Imports
use super::{
    Lanes,
    support::{self, Bank, Fact, Ledger, Noted, catalog},
};
use harmos_runtime::operations::{Context, Operation, OperationPolicy, RestartPolicy, RetryPolicy};
use harmos_runtime::{Error, Runtime, state::CausationId, state::Position};

/* === Constants === */

const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Note;
struct Descend;
struct Family;

#[derive(Clone, Default)]
struct TraceCapture(Arc<Mutex<Vec<TraceRecord>>>);

struct TraceRecord {
    id: u64,
    parent: Option<u64>,
    name: &'static str,
    fields: BTreeMap<String, String>,
    follows: Vec<u64>,
    closed: bool,
}

struct TraceFields(BTreeMap<String, String>);

/* === Enums === */

#[harmos_runtime::message]
#[harmos_runtime::operation(response = (), id = "causation.note", version = 1, policy = POLICY)]
struct NoteContract {
    label: String,
}

#[harmos_runtime::message]
#[harmos_runtime::operation(response = (), id = "causation.descend", version = 1, policy = POLICY)]
struct DescendContract {
    label: String,
}

#[harmos_runtime::message]
#[harmos_runtime::operation(response = (), id = "causation.family", version = 1, policy = POLICY)]
struct FamilyContract {
    cause: u64,
}

#[harmos_runtime::catalog]
enum Operations {
    NoteContract(NoteContract),
    DescendContract(DescendContract),
    FamilyContract(FamilyContract),
}

/* === Traits === */

/* === Implementations === */

impl tracing::field::Visit for TraceFields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name().to_owned(), format!("{value:?}"));
    }
}

impl<S> Layer<S> for TraceCapture
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attributes: &tracing::span::Attributes<'_>, id: &tracing::Id, context: TraceContext<'_, S>) {
        let parent = attributes
            .parent()
            .cloned()
            .or_else(|| attributes.is_contextual().then(|| context.lookup_current().map(|span| span.id())).flatten());
        let mut fields = TraceFields(BTreeMap::new());
        attributes.record(&mut fields);
        self.0.lock().unwrap().push(TraceRecord {
            id: id.into_u64(),
            parent: parent.map(|id| id.into_u64()),
            name: attributes.metadata().name(),
            fields: fields.0,
            follows: Vec::new(),
            closed: false,
        });
    }

    fn on_record(&self, id: &tracing::Id, values: &tracing::span::Record<'_>, _: TraceContext<'_, S>) {
        let mut fields = TraceFields(BTreeMap::new());
        values.record(&mut fields);
        let mut records = self.0.lock().unwrap();
        let record = records.iter_mut().find(|record| record.id == id.into_u64()).unwrap();
        record.fields.extend(fields.0);
    }

    fn on_close(&self, id: tracing::Id, _: TraceContext<'_, S>) {
        let mut records = self.0.lock().unwrap();
        records.iter_mut().find(|record| record.id == id.into_u64()).unwrap().closed = true;
    }

    fn on_follows_from(&self, id: &tracing::Id, predecessor: &tracing::Id, _: TraceContext<'_, S>) {
        let mut records = self.0.lock().unwrap();
        records.iter_mut().find(|record| record.id == id.into_u64()).unwrap().follows.push(predecessor.into_u64());
    }
}

impl Operation for Note {
    type Declaration = NoteContract;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(_resources: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, Error> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, request: &mut NoteContract, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Error> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            context.scope().records::<Bank>().publish(Noted(request.label.clone())).await?;
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Operation for Descend {
    type Declaration = DescendContract;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(_resources: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, Error> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, request: &mut DescendContract, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Error> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            context.scope().records::<Bank>().publish(Noted(request.label.clone())).await?;
            context.submit(NoteContract { label: format!("{}-child", request.label) })?.await?;
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Operation for Family {
    type Declaration = FamilyContract;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(_resources: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, Error> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, request: &mut FamilyContract, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Error> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            context.scope().records::<Bank>().publish(Noted("parent".to_owned())).await?;
            context.submit(DescendContract { label: "inherited".to_owned() })?.await?;
            context.submit_caused(DescendContract { label: "rerooted".to_owned() }, Position(request.cause))?.await?;
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> Runtime<Bank> {
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ()).register(catalog()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime.operations.register_operation::<Family>("app", Lanes::Serial, Arc::new(())).unwrap();
    runtime.operations.register_operation::<Descend>("app", Lanes::Concurrent, Arc::new(())).unwrap();
    runtime.operations.register_operation::<Note>("app", Lanes::Open, Arc::new(())).unwrap();
    runtime
}

/* === Tests === */

#[tokio::test]
async fn scoped_trace_handoffs() {
    let runtime = runtime().await;
    let capture = TraceCapture::default();
    let subscriber = tracing_subscriber::registry().with(capture.clone());
    async {
        let span = tracing::debug_span!("request");
        async {
            let invocation = runtime.operations.submit(NoteContract { label: "trace-secret-payload".to_owned() }).unwrap();
            invocation.await.unwrap();
            let cancelled = runtime.operations.submit(NoteContract { label: "cancelled".to_owned() }).unwrap();
            cancelled.cancel();
            assert!(cancelled.await.is_err());
        }
        .instrument(span)
        .await;
    }
    .with_subscriber(subscriber)
    .await;
    runtime.stop().await.unwrap();

    let records = capture.0.lock().unwrap();
    let root = records.iter().find(|record| record.name == "request").unwrap().id;
    for name in ["operations.instance", "operations.queue", "operations.dispatch", "operations.attempt", "state.queue", "state.write"] {
        let record = records.iter().find(|record| record.name == name).expect("the handoff is observable");
        assert!(record.closed, "{name} closes after physical settlement");
        let mut parent = record.parent;
        while let Some(id) = parent {
            if id == root {
                break;
            }
            parent = records.iter().find(|record| record.id == id).expect("the parent belongs to the scoped collector").parent;
        }
        assert_eq!(parent, Some(root), "{name} preserves the originating request");
    }
    let instance = records.iter().find(|record| record.name == "operations.instance").unwrap();
    assert_eq!(instance.fields.get("settled").map(String::as_str), Some("true"));
    assert_eq!(instance.fields.get("outcome").map(String::as_str), Some("Succeeded"));
    for (work, queue) in [("operations.dispatch", "operations.queue"), ("state.write", "state.queue")] {
        let work = records.iter().find(|record| record.name == work).unwrap();
        let queue = records.iter().find(|record| record.name == queue).unwrap();
        assert!(work.follows.contains(&queue.id));
    }
    assert!(records.iter().any(|record| record.name == "operations.instance"
        && record.closed
        && record.fields.get("outcome").map(String::as_str) == Some("Cancelled")));
    assert!(records.iter().all(|record| record.fields.values().all(|value| !value.contains("trace-secret-payload"))));
}

#[tokio::test]
async fn filtered_trace_behaves() {
    let runtime = runtime().await;
    let capture = TraceCapture::default();
    let subscriber = tracing_subscriber::registry().with(capture.clone()).with(tracing::level_filters::LevelFilter::OFF);
    async {
        runtime.operations.call(NoteContract { label: "filtered".to_owned() }).await.unwrap();
    }
    .with_subscriber(subscriber)
    .await;
    let records: Vec<_> = runtime.state.query::<Fact>(Position::ORIGIN).await.collect();
    assert_eq!(records.len(), 1);
    runtime.stop().await.unwrap();
    assert!(capture.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn dropped_trace_handoff() {
    let runtime = runtime().await;
    let capture = TraceCapture::default();
    let subscriber = tracing_subscriber::registry().with(capture.clone());
    async {
        let request = tracing::debug_span!("dropped.request");
        request.in_scope(|| {
            let invocation = runtime.operations.submit(NoteContract { label: "abandoned".to_owned() }).unwrap();
            drop(invocation);
        });
    }
    .with_subscriber(subscriber)
    .await;

    // The Dispatcher Drains The Queued Cancellation Outside The Caller's Scope.
    runtime.stop().await.unwrap();
    let records = capture.0.lock().unwrap();
    let instance = records.iter().find(|record| record.name == "operations.instance").unwrap();
    assert_eq!(instance.fields.get("outcome").map(String::as_str), Some("Cancelled"));
    assert!(records.iter().all(|record| record.closed));
}

#[tokio::test]
async fn children_inherit_cause() {
    let runtime = runtime().await;
    let causing = runtime.state.commit(support::deposit(5)).await.expect("the causing deposit lands");
    let other = runtime.state.commit(support::deposit(7)).await.expect("the second cause lands");

    runtime
        .operations
        .submit_caused(FamilyContract { cause: other.position.0 }, causing.position)
        .unwrap()
        .await
        .expect("every generation records its fact");

    let noted: BTreeMap<String, Option<CausationId>> = runtime
        .state
        .query::<Fact>(Position::ORIGIN)
        .await
        .map(|entry| entry.expect("the recorded fact is readable"))
        .map(|entry| {
            let Fact::Noted(Noted(label)) = entry.value;
            (label, entry.metadata.causation)
        })
        .collect();

    let root = Some(CausationId(causing.position));
    let other = Some(CausationId(other.position));
    assert_eq!(noted.get("parent"), Some(&root));
    assert_eq!(noted.get("inherited"), Some(&root), "a submitted child inherits the cause its parent's attempt carries");
    assert_eq!(noted.get("inherited-child"), Some(&root), "inheritance chains: a default-lane grandchild is in the same family");
    assert_eq!(noted.get("rerooted"), Some(&other), "a child that named its own cause re-roots itself");
    assert_eq!(noted.get("rerooted-child"), Some(&other), "and everything beneath it inherits the cause it re-rooted onto");
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn caused_submit_records() {
    let runtime = runtime().await;
    let receipt = runtime.state.commit(support::deposit(5)).await.expect("the causing deposit lands");

    runtime
        .operations
        .submit_caused(NoteContract { label: "answered".to_owned() }, receipt.position)
        .unwrap()
        .await
        .expect("the caused job records its fact");

    let noted: Vec<_> = runtime
        .state
        .query::<Fact>(Position::ORIGIN)
        .await
        .map(|entry| entry.expect("the recorded fact is readable"))
        .filter(|entry| matches!(entry.value, Fact::Noted(_)))
        .collect();
    assert_eq!(noted.len(), 1);
    assert_eq!(
        noted[0].metadata.causation,
        Some(CausationId(receipt.position)),
        "the recorded fact answers the position the submission named"
    );
    assert_eq!(noted[0].metadata.correlation.0, receipt.position);
    runtime.stop().await.unwrap();
}
