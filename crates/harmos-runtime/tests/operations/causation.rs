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
use std::sync::Arc;
use std::time::Duration;

// Third Party Imports

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
