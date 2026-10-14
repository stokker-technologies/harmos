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
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::{HostLanes, assembly, operations, service_status};
use crate::core_journal::OriginalJournal;
use crate::support::{Bank, DEPOSIT, Deposit, HOUSE};
use harmos::operations::Host as _;
use harmos::operations::{OperationPolicy, RestartPolicy, RetryPolicy};
use harmos::sidecar::{Notice, Refusal, Supervised, Supervisor};
use harmos::{services::Service, services::ServiceStatus};

/* === Constants === */
const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(10),
    attempt_timeout: Duration::from_secs(10),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};
/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Invoker {
    resources: Arc<()>,
}

/* === Enums === */
/* === Traits === */

/* === Implementations === */

impl harmos::operations::lifecycle::Policy for Invoker {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos::plugin(id = "journal-invoker", provides = (), requires(harmos::operations::host::Invoke))]
impl harmos::Plugin for Invoker {
    type Context = Supervisor;
    type Config = ();
    type Error = Refusal;
    type Resources = ();
    fn prepare(_: ()) -> Result<Self, Refusal> {
        Ok(Self {
            resources: Arc::new(()),
        })
    }
    fn resources(&self) -> Arc<()> {
        self.resources.clone()
    }
    fn bind_operations(
        &self,
        _: &mut harmos::operations::WireTable<Supervisor>,
    ) -> Result<(), harmos::operations::ServingError> {
        Ok(())
    }
    async fn initialize(&mut self, context: &Supervisor) -> Result<(), Refusal> {
        let core = context;
        let payload = serde_json::to_vec(&Deposit {
            account: HOUSE,
            amount: 7,
        })
        .map_err(Refusal::of)?;
        let request = harmos::operations::host::InvokeRequest {
            grant: DEPOSIT.1.into(),
            key: "same-original-commit".into(),
            payload,
        };
        let mut denied = request.clone();
        denied.grant = "ledger.withdraw".into();
        if core
            .call_as::<harmos::operations::host::Invoke>(denied.clone())
            .await
            .is_ok()
        {
            return Err(Refusal::of("unselected transaction reached the journal"));
        }
        let first = core
            .call_as::<harmos::operations::host::Invoke>(request.clone())
            .await
            .map_err(Refusal::of)?;
        let second = core
            .call_as::<harmos::operations::host::Invoke>(request)
            .await
            .map_err(Refusal::of)?;
        if first != second || first.position == 0 {
            return Err(Refusal::of(
                "keyed calls did not retain the same journal receipt",
            ));
        }
        let malformed = harmos::operations::host::InvokeRequest {
            grant: DEPOSIT.1.into(),
            key: "malformed".into(),
            payload: b"invalid-json".to_vec(),
        };
        if core
            .call_as::<harmos::operations::host::Invoke>(malformed)
            .await
            .is_ok()
        {
            return Err(Refusal::of("malformed transaction payload was admitted"));
        }
        if core
            .call_as::<harmos::operations::host::Invoke>(denied)
            .await
            .is_ok()
        {
            return Err(Refusal::of(
                "later unselected transaction reached the journal",
            ));
        }
        Ok(())
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<(), Refusal> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn notices(_: &Service<Bank>, _: Notice) {}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("provider_transaction_receipt", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly()
                        .operation_settings(harmos::operations::Settings {
                            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                            ..Default::default()
                        })
                        .runtime()
                        .await
                        .unwrap();
                    let original = Arc::new(OriginalJournal::new(runtime.state.clone()));
                    runtime
                        .operations
                        .register_wire_operation::<crate::core_journal::Invoke>(
                            "bank-journal",
                            HostLanes::Open,
                            original.clone(),
                        )
                        .unwrap();
                    let capacity = NonZeroUsize::new(2).unwrap();
                    let source = harmos_sidecar::PluginNative::<Invoker>::new(capacity).unwrap();
                    let definition =
                        operations::definition(&source, serde_json::to_vec(&()).unwrap());
                    let resident = Supervised::new(source, notices as fn(&Service<Bank>, Notice));
                    runtime
                        .operations
                        .register_service(
                            harmos::services::ServiceDefinition {
                                id: "journal-carrier".into(),
                                policy: harmos::operations::OperationPolicy {
                                    restart: harmos::operations::RestartPolicy::Never,
                                    ..harmos::services::POLICY
                                },
                            },
                            move |_: Arc<()>| {
                                harmos::sidecar::Managed::new(
                                    resident.clone(),
                                    definition.clone(),
                                    HostLanes::Open,
                                    capacity,
                                    NonZeroUsize::new(256).unwrap(),
                                )
                                .unwrap()
                            },
                        )
                        .unwrap();
                    let lease = runtime
                        .operations
                        .submit_resident("journal-carrier")
                        .await
                        .unwrap();
                    service_status(
                        runtime.operations.watch_residents(),
                        "journal-carrier",
                        |status| matches!(status, ServiceStatus::Running { .. }),
                    )
                    .await;
                    // No Grant Names Transactions: The Provider Refuses The Two Withdrawals Itself
                    assert_eq!(original.prepared.load(Ordering::SeqCst), 5);
                    assert_eq!(original.handled.load(Ordering::SeqCst), 5);
                    assert_eq!(original.finalized.load(Ordering::SeqCst), 5);
                    let state = runtime
                        .state
                        .state()
                        .read(|state| (state.balances.get(&HOUSE).copied(), state.applications))
                        .await
                        .unwrap();
                    assert_eq!(state.value, (Some(7), 1));
                    drop(lease);
                    runtime.stop().await.unwrap();
                });
        });
    }
}

/* === Tests === */
