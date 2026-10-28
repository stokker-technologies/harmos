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
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::operations::{Backoff, FailureKind, Jitter, OperationDeclaration, OperationInstanceId, OperationPolicy, RetryPolicy};
use harmos_runtime::{Message, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Resources {
    events: Mutex<Vec<&'static str>>,
    ids: Mutex<Vec<OperationInstanceId>>,
}

struct AttemptGuard(Arc<Resources>);

#[harmos_runtime::message]
#[derive(Debug)]
struct Text {
    value: String,
}

/* === Enums === */

#[harmos_runtime::operation(resources = [Resources, harmos_runtime::operations::HostContext], response = Text,
        id = "owned.read",
        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::OnTransient {
                max_retries: 1,
                backoff: Backoff::Fixed { delay: Duration::from_millis(1), jitter: Jitter::None },
            },
        },
    )]
struct ReadContractDeclaration {
    value: String,
}

#[harmos_runtime::catalog]
enum Operations {
    ReadContractDeclaration(ReadContractDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Operation for ReadContractDeclaration {
    type Error = std::io::Error;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Text>, Self::Error> {
        let resources = ctx.resource::<Resources>();
        let _guard = AttemptGuard(resources.clone());
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        resources.events.lock().unwrap().push("run");
        resources.ids.lock().unwrap().push(context.id());
        Ok(Text {
            value: format!("{}:{}", self.value, context.attempt() + 1),
        }
        .into())
    }
    fn classify(_: &Self::Error) -> FailureKind {
        FailureKind::Transient
    }
}
impl Drop for AttemptGuard {
    fn drop(&mut self) {
        self.0.events.lock().unwrap().push("drop");
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> (Runtime<App>, Arc<Resources>) {
    let resources = Arc::new(Resources::default());
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Resources>(resources.clone());
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(16 * 1024).unwrap()),
            ..Default::default()
        });
        setup.operations::<crate::operations::Single<ReadContractDeclaration>>(Lanes::Open);
    })
    .await
    .unwrap();
    assert!(resources.events.lock().unwrap().is_empty());
    (runtime, resources)
}

#[tokio::test]
async fn wire_bindings_execute() {
    {
        let resources = Arc::new(Resources::default());
        let runtime = Runtime::<App>::launch_with((), (), |setup| {
            setup.resource::<Resources>(resources.clone());
            setup.settings().operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(std::num::NonZeroUsize::new(16 * 1024).unwrap()),
                ..Default::default()
            });
            setup.operations::<crate::operations::Single<ReadContractDeclaration>>(Lanes::Open);
        })
        .await
        .unwrap();
        assert!(resources.events.lock().unwrap().is_empty());
        let invocation = runtime
            .operations
            .submit_encoded(&format!("host.{}", ReadContractDeclaration::ID), Text { value: "hello".into() }.encode())
            .unwrap();
        let response = invocation.await.unwrap();
        assert_eq!(Text::decode(&response).unwrap().value, "hello:1");
        assert_eq!(resources.events.lock().unwrap().iter().filter(|event| **event == "run").count(), 1);
        runtime.shutdown().await.unwrap();
    }
    {
        let (runtime, resources) = runtime().await;
        let encoded = runtime
            .operations
            .submit_encoded(&format!("host.{}", ReadContractDeclaration::ID), payload("complete"))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(Text::decode(&encoded).unwrap().value, "complete:1");
        assert_eq!(*resources.events.lock().unwrap(), ["run", "drop"]);
        let answer = runtime.operation(ReadContractDeclaration { value: "local".to_owned() }).run().await.unwrap();
        assert_eq!(answer.value, "local:1");
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn contracts_guard_identity() {
    let (runtime, resources) = runtime().await;
    assert!(runtime.operations.register::<crate::operations::Single<ReadContractDeclaration>>(Lanes::Open).await.is_err());
    assert!(resources.events.lock().unwrap().is_empty());
    runtime.shutdown().await.unwrap();
}

#[cfg(feature = "plugin")]
#[tokio::test]
async fn owner_assignments_isolate_instances() {
    let (runtime, resources) = runtime().await;
    let mut event_count = 0;
    for owner in ["camera", "simulator"] {
        runtime
            .plugin
            .register_embedded::<crate::operations::Single<ReadContractDeclaration>, Lanes>(
                harmos_runtime::plugin::PluginDefinition {
                    id: owner.into(),
                    version: "1.0.0".parse().unwrap(),
                    artifact: harmos_runtime::plugin::PluginArtifact::Embedded { revision: "fixture".into() },
                    requires: vec![],
                    configuration: b"null".to_vec(),
                    contributions: Default::default(),
                },
                Default::default(),
                Lanes::Open,
            )
            .await
            .unwrap();
        runtime.plugin.initialize().await.unwrap();
        assert_eq!(resources.events.lock().unwrap().len(), event_count, "activation registration executes no requests");
        let response = runtime
            .operations
            .submit_from::<ReadContractDeclaration>(owner, ReadContractDeclaration { value: owner.into() })
            .unwrap()
            .await
            .unwrap();
        assert_eq!(response.value, format!("{owner}:1"));
        event_count += 2;
        assert_eq!(&resources.events.lock().unwrap()[event_count - 2..], ["run", "drop"]);
    }
    assert_eq!(*resources.events.lock().unwrap(), ["run", "drop", "run", "drop"]);
    {
        let ids = resources.ids.lock().unwrap();
        assert_ne!(ids[0], ids[1]);
    }
    runtime.shutdown().await.unwrap();
}

fn payload(value: &str) -> Vec<u8> {
    Text { value: value.to_owned() }.encode()
}

/* === Tests === */

#[tokio::test]
async fn wire_registration_uses_supplied_schema_snapshot() {
    use harmos_runtime::operations::{Contract, WireDeclaration};
    let declaration = WireDeclaration::of::<ReadContractDeclaration>().contract().unwrap();
    let tiny = harmos_runtime::config::SchemaSettings {
        preparation_bytes: std::num::NonZeroUsize::MIN,
        ..Default::default()
    };
    assert!(WireDeclaration::of::<ReadContractDeclaration>().contract_with_settings(&tiny).is_err());
    assert!(
        Contract::new_with_settings(
            declaration.operation().clone(),
            declaration.fingerprint(),
            declaration.codec(),
            declaration.schema().clone(),
            &tiny
        )
        .is_err()
    );
    assert!(declaration.compatible(&declaration));
    assert!(!declaration.compatible_with_settings(&declaration, &tiny));

    for initial in [false, true] {
        for accepted in [true, false] {
            let settings = if accepted { harmos_runtime::config::SchemaSettings::default() } else { tiny };
            let resources = Arc::new(Resources::default());
            let runtime = Runtime::<App>::launch_with((), (), |setup| {
                setup.resource::<Resources>(resources.clone());
                setup.settings().schema_settings(settings);
                if initial {
                    setup.operations::<crate::operations::Single<ReadContractDeclaration>>(Lanes::Open);
                }
            })
            .await;
            if initial {
                assert_eq!(runtime.is_ok(), accepted);
                if let Ok(runtime) = runtime {
                    assert!(runtime.operations.definition(ReadContractDeclaration::ID).is_ok());
                    runtime.shutdown().await.unwrap();
                }
            } else {
                let runtime = runtime.unwrap();
                let result = runtime.operations.register::<crate::operations::Single<ReadContractDeclaration>>(Lanes::Open).await;
                assert_eq!(result.is_ok(), accepted);
                assert_eq!(runtime.operations.definition(ReadContractDeclaration::ID).is_ok(), accepted);
                runtime.shutdown().await.unwrap();
            }
        }
    }
}
