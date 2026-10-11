/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

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
use harmos::operations::{Context, Operation, OperationPolicy, RetryPolicy};
use harmos::{CausationId, Error, Position, Runtime};

/* === Constants === */
const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Note;
struct Descend;
struct Family;

/* === Enums === */
#[harmos::catalog]
enum Operations {
    #[operation(request = String, response = (), id = "causation.note", policy = POLICY)]
    NoteContract,
    #[operation(request = String, response = (), id = "causation.descend", policy = POLICY)]
    DescendContract,
    #[operation(request = Position, response = (), id = "causation.family", policy = POLICY)]
    FamilyContract,
}

/* === Traits === */

/* === Implementations === */

impl Operation for Note {
    type Declaration = NoteContract;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    fn new(_resources: Arc<()>) -> Result<Self, Error> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut String,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<(), Error> {
        context
            .scope()
            .records::<Bank>()
            .publish(Noted(request.clone()))
            .await?;
        Ok(())
    }
}

impl Operation for Descend {
    type Declaration = DescendContract;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    fn new(_resources: Arc<()>) -> Result<Self, Error> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut String,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<(), Error> {
        context
            .scope()
            .records::<Bank>()
            .publish(Noted(request.clone()))
            .await?;
        context
            .submit::<NoteContract>(format!("{}-child", *request))?
            .await?;
        Ok(())
    }
}

impl Operation for Family {
    type Declaration = FamilyContract;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    fn new(_resources: Arc<()>) -> Result<Self, Error> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut Position,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<(), Error> {
        context
            .scope()
            .records::<Bank>()
            .publish(Noted("parent".to_owned()))
            .await?;
        context
            .submit::<DescendContract>("inherited".to_owned())?
            .await?;
        context
            .submit_caused::<DescendContract>("rerooted".to_owned(), *request)?
            .await?;
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn runtime() -> Runtime<Bank> {
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_operation::<Family>("app", Lanes::Serial, Arc::new(()))
        .unwrap();
    runtime
        .operations
        .register_operation::<Descend>("app", Lanes::Concurrent, Arc::new(()))
        .unwrap();
    runtime
        .operations
        .register_operation::<Note>("app", Lanes::Open, Arc::new(()))
        .unwrap();
    runtime
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("scope_hands_cause", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let causing = runtime
                        .state
                        .commit(support::deposit(5))
                        .await
                        .expect("the causing deposit lands");
                    let other = runtime
                        .state
                        .commit(support::deposit(7))
                        .await
                        .expect("the second cause lands");

                    runtime
                        .operations
                        .submit_caused::<FamilyContract>(other.position, causing.position)
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
                    assert_eq!(
                        noted.get("inherited"),
                        Some(&root),
                        "a submitted child inherits the cause its parent's attempt carries"
                    );
                    assert_eq!(
                        noted.get("inherited-child"),
                        Some(&root),
                        "inheritance chains: a default-lane grandchild is in the same family"
                    );
                    assert_eq!(
                        noted.get("rerooted"),
                        Some(&other),
                        "a child that named its own cause re-roots itself"
                    );
                    assert_eq!(
                        noted.get("rerooted-child"),
                        Some(&other),
                        "and everything beneath it inherits the cause it re-rooted onto"
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("scoped_attempt_records", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let receipt = runtime
                        .state
                        .commit(support::deposit(5))
                        .await
                        .expect("the causing deposit lands");

                    runtime
                        .operations
                        .submit_caused::<NoteContract>("answered".to_owned(), receipt.position)
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
                });
        });
    }
}
