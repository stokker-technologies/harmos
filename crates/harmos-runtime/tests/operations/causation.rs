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
use harmos_runtime::operations::{OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Runtime, state::CausationId, state::Position};

/* === Constants === */

const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

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

#[harmos_runtime::operation(resources = [harmos_runtime::operations::HostContext], response = (), id = "causation.note",  policy = POLICY)]
struct NoteContractDeclaration {
    label: String,
}

#[harmos_runtime::operation(resources = [harmos_runtime::operations::HostContext], response = (), id = "causation.descend",  policy = POLICY)]
struct DescendContractDeclaration {
    label: String,
}

#[harmos_runtime::operation(resources = [harmos_runtime::operations::HostContext], response = (), id = "causation.family",  policy = POLICY)]
struct FamilyContractDeclaration {
    cause: u64,
}

#[harmos_runtime::catalog]
enum Operations {
    NoteContractDeclaration(NoteContractDeclaration),
    DescendContractDeclaration(DescendContractDeclaration),
    FamilyContractDeclaration(FamilyContractDeclaration),
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

impl harmos_runtime::operations::Operation for NoteContractDeclaration {
    type Error = Error;

    async fn run(self, context: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Error> {
        let host = context.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let result: ::core::result::Result<(), Self::Error> = async move {
            context.scope().records::<Bank>().publish(Noted(self.label.clone())).await?;
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::operations::Operation for DescendContractDeclaration {
    type Error = Error;

    async fn run(self, context: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Error> {
        let host = context.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let result: ::core::result::Result<(), Self::Error> = async move {
            context.scope().records::<Bank>().publish(Noted(self.label.clone())).await?;
            context.submit::<NoteContractDeclaration>(NoteContractDeclaration { label: format!("{}-child", self.label) })?.await?;
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::operations::Operation for FamilyContractDeclaration {
    type Error = Error;

    async fn run(self, context: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Error> {
        let host = context.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let result: ::core::result::Result<(), Self::Error> = async move {
            context.scope().records::<Bank>().publish(Noted("parent".to_owned())).await?;
            context.submit::<DescendContractDeclaration>(DescendContractDeclaration { label: "inherited".to_owned() })?.await?;
            context
                .submit_caused::<DescendContractDeclaration>(
                    DescendContractDeclaration { label: "rerooted".to_owned() },
                    Position(self.cause),
                )?
                .await?;
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
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(Arc::new(())).unwrap();
                dependencies
            })
            .operation::<FamilyContractDeclaration>(),
            Lanes::Serial,
        ))
        .await
        .unwrap();
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(Arc::new(())).unwrap();
                dependencies
            })
            .operation::<DescendContractDeclaration>(),
            Lanes::Concurrent,
        ))
        .await
        .unwrap();
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(Arc::new(())).unwrap();
                dependencies
            })
            .operation::<NoteContractDeclaration>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
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
            let invocation = runtime.operation(NoteContractDeclaration { label: "trace-secret-payload".to_owned() }).start().await.unwrap();
            invocation.await.unwrap();
            let cancelled = runtime.operation(NoteContractDeclaration { label: "cancelled".to_owned() }).start().await.unwrap();
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
        let record = records.iter().find(|record| record.name == name).unwrap_or_else(|| panic!("missing handoff {name}"));
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
        runtime.operation(NoteContractDeclaration { label: "filtered".to_owned() }).run().await.unwrap();
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
        async {
            let invocation = runtime.operation(NoteContractDeclaration { label: "abandoned".to_owned() }).start().await.unwrap();
            drop(invocation);
        }
        .instrument(request)
        .await;
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
        .submit_caused::<FamilyContractDeclaration>(FamilyContractDeclaration { cause: other.position.0 }, causing.position)
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
        .submit_caused::<NoteContractDeclaration>(NoteContractDeclaration { label: "answered".to_owned() }, receipt.position)
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
