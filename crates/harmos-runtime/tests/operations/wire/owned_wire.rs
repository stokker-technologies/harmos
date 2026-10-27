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
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(16 * 1024).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let resources = Arc::new(Resources::default());
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(resources.clone()).unwrap();
                dependencies
            })
            .operation::<ReadContractDeclaration>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    assert!(resources.events.lock().unwrap().is_empty());
    (runtime, resources)
}

#[tokio::test]
async fn wire_bindings_execute() {
    {
        let resources = Arc::new(Resources::default());
        let runtime = Runtime::<App>::builder((), ())
            .register(Lanes::catalog())
            .operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(std::num::NonZeroUsize::new(16 * 1024).unwrap()),
                ..Default::default()
            })
            .with_registration(harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new({
                    let mut dependencies = harmos_runtime::Dependencies::new();
                    dependencies.insert(resources.clone()).unwrap();
                    dependencies
                })
                .operation::<ReadContractDeclaration>(),
                Lanes::Open,
            ))
            .runtime()
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
        runtime.stop().await.unwrap();
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
        runtime.stop().await.unwrap();
    }
}

#[tokio::test]
async fn contracts_guard_identity() {
    let (runtime, resources) = runtime().await;
    for change_policy in [false, true] {
        let mut batch = harmos_runtime::RegistrationBatch::new({
            let mut dependencies = harmos_runtime::Dependencies::new();
            dependencies.insert(resources.clone()).unwrap();
            dependencies
        })
        .operation::<ReadContractDeclaration>();
        if change_policy {
            batch.operations[0].definition.policy.timeout += Duration::from_secs(1);
        } else {
            batch.operations[0].definition.id = harmos_runtime::Identifier::new("camera_read").unwrap();
        }
        assert!(runtime.operations.register(harmos_runtime::RegistrationExt::lane(batch, Lanes::Open)).await.is_err());
    }
    assert!(resources.events.lock().unwrap().is_empty());
    runtime.stop().await.unwrap();
}

#[cfg(feature = "plugin")]
#[tokio::test]
async fn owner_assignments_isolate_instances() {
    let (runtime, _) = runtime().await;
    let mut owners = Vec::new();
    for owner in ["camera", "simulator"] {
        let resources = Arc::new(Resources::default());
        runtime
            .plugin
            .register_embedded(
                harmos_runtime::plugin::PluginDefinition {
                    id: owner.into(),
                    version: "1.0.0".parse().unwrap(),
                    artifact: harmos_runtime::plugin::PluginArtifact::Embedded { revision: "fixture".into() },
                    requires: vec![],
                    configuration: b"null".to_vec(),
                    contributions: Default::default(),
                },
                Default::default(),
                harmos_runtime::RegistrationExt::lane(
                    harmos_runtime::RegistrationBatch::new({
                        let mut dependencies = harmos_runtime::Dependencies::new();
                        dependencies.insert(resources.clone()).unwrap();
                        dependencies
                    })
                    .operation::<ReadContractDeclaration>(),
                    Lanes::Open,
                ),
            )
            .await
            .unwrap();
        runtime.plugin.initialize().await.unwrap();
        assert!(resources.events.lock().unwrap().is_empty());
        let response = runtime
            .operations
            .submit_encoded(&format!("{owner}.{}", ReadContractDeclaration::ID), Text { value: owner.into() }.encode())
            .unwrap()
            .await
            .unwrap();
        assert_eq!(Text::decode(&response).unwrap().value, format!("{owner}:1"));
        assert_eq!(*resources.events.lock().unwrap(), ["run", "drop"]);
        owners.push(resources);
    }
    assert_ne!(owners[0].ids.lock().unwrap()[0], owners[1].ids.lock().unwrap()[0]);
    runtime.stop().await.unwrap();
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
            let builder = Runtime::<App>::builder((), ()).schema_settings(settings).register(Lanes::catalog());
            let mut dependencies = harmos_runtime::Dependencies::new();
            dependencies.insert(Arc::new(Resources::default())).unwrap();
            let batch = harmos_runtime::RegistrationExt::lane(
                harmos_runtime::RegistrationBatch::new(dependencies).operation::<ReadContractDeclaration>(),
                Lanes::Open,
            );
            if initial {
                let result = builder.with_registration(batch).runtime().await;
                assert_eq!(result.is_ok(), accepted);
                if let Ok(runtime) = result {
                    assert!(runtime.operations.definition(ReadContractDeclaration::ID).is_ok());
                    runtime.stop().await.unwrap();
                }
            } else {
                let runtime = builder.runtime().await.unwrap();
                let result = runtime.operations.register(batch).await;
                assert_eq!(result.is_ok(), accepted);
                assert_eq!(runtime.operations.definition(ReadContractDeclaration::ID).is_ok(), accepted);
                runtime.stop().await.unwrap();
            }
        }
    }
}
