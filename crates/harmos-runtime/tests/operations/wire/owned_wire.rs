/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::app::App;
use harmos_runtime::Runtime;
use harmos_runtime::operations::{Backoff, FailureKind, Jitter, OperationInstanceId, OperationPolicy, RetryPolicy};

/* === Structs === */

#[derive(Default)]
struct Resources {
    events: Mutex<Vec<&'static str>>,
    ids: Mutex<Vec<OperationInstanceId>>,
}

struct AttemptGuard(Arc<Resources>);

#[derive(harmos_runtime::Message, Clone, Default, Debug)]
struct Text {
    value: String,
}

/* === Enums === */

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Text)]
#[derive(Clone)]
struct ReadContractDeclaration {
    value: String,
}

static READ_CONTRACT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<ReadContractDeclaration>(
        "owned.read",
        OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::OnTransient {
                total_attempts: 2,
                backoff: Backoff::Fixed {
                    delay: Duration::from_millis(1),
                    jitter: Jitter::None,
                },
            },
        },
    )
    .replay(harmos_runtime::operations::ReplaySafety::Idempotent);

const READ_CONTRACT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<ReadContractDeclaration> =
    harmos_runtime::operations::OperationSpec::<ReadContractDeclaration>::new(&READ_CONTRACT_DECLARATION_DESCRIPTION);

/* === Implementations === */

async fn read_contract_declaration_handler(
    request: ReadContractDeclaration, ctx: harmos_runtime::operations::FunctionContext<ReadContractDeclaration>,
) -> Result<harmos_runtime::Response<Text>, std::io::Error> {
    let resources = ctx.resource::<Resources>().expect("declared resource is registered");
    let _guard = AttemptGuard(resources.clone());
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    resources.events.lock().unwrap().push("run");
    resources.ids.lock().unwrap().push(context.id());
    Ok(Text {
        value: format!("{}:{}", request.value, context.attempt() + 1),
    }
    .into())
}

fn read_contract_declaration_classification(_: &std::io::Error) -> FailureKind {
    FailureKind::Transient
}

fn read_contract_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        READ_CONTRACT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Resources>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        read_contract_declaration_handler,
        read_contract_declaration_classification,
    )
}
impl Drop for AttemptGuard {
    fn drop(&mut self) {
        self.0.events.lock().unwrap().push("drop");
    }
}

/* === Private Functions === */

async fn runtime() -> (Runtime<App>, Arc<Resources>) {
    let resources = Arc::new(Resources::default());
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<Resources>(resources.clone());
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(16 * 1024).unwrap()),
            ..Default::default()
        });
        setup.operation(
            read_contract_declaration_operation().expect("representable operation"),
            harmos_runtime::operations::Lane::default(),
        );
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
        let runtime = crate::support::app::launch::<App>((), (), |setup| {
            setup.resource::<Resources>(resources.clone());
            setup.settings().operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(std::num::NonZeroUsize::new(16 * 1024).unwrap()),
                ..Default::default()
            });
            setup.operation(
                read_contract_declaration_operation().expect("representable operation"),
                harmos_runtime::operations::Lane::default(),
            );
        })
        .await
        .unwrap();
        assert!(resources.events.lock().unwrap().is_empty());
        let invocation = runtime
            .operations
            .submit_encoded(&format!("host.{}", READ_CONTRACT_DECLARATION_OPERATION.entry().id), protobuf(&Text { value: "hello".into() }))
            .unwrap();
        let response = invocation.await.unwrap();
        assert_eq!(decode_protobuf::<Text>(&response).value, "hello:1");
        assert_eq!(resources.events.lock().unwrap().iter().filter(|event| **event == "run").count(), 1);
        crate::support::app::shutdown(&runtime).await.unwrap();
    }
    {
        let (runtime, resources) = runtime().await;
        let encoded = runtime
            .operations
            .submit_encoded(&format!("host.{}", READ_CONTRACT_DECLARATION_OPERATION.entry().id), payload("complete"))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(decode_protobuf::<Text>(&encoded).value, "complete:1");
        assert_eq!(*resources.events.lock().unwrap(), ["run", "drop"]);
        let answer = runtime.operation(ReadContractDeclaration { value: "local".to_owned() }).await.unwrap();
        assert_eq!(answer.value, "local:1");
        crate::support::app::shutdown(&runtime).await.unwrap();
    }
}

#[tokio::test]
async fn contracts_guard_identity() {
    let (runtime, resources) = runtime().await;
    assert!(
        runtime
            .operations
            .register_operation(
                read_contract_declaration_operation().expect("representable operation"),
                harmos_runtime::operations::Lane::default()
            )
            .await
            .is_err()
    );
    assert!(resources.events.lock().unwrap().is_empty());
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[cfg(feature = "plugin-guest")]
#[tokio::test]
async fn owner_assignments_isolate_instances() {
    let (runtime, resources) = runtime().await;
    let mut event_count = 0;
    for owner in ["camera", "simulator"] {
        runtime
            .plugin
            .register_embedded_batch(
                harmos_runtime::guest::PluginDefinition {
                    id: owner.into(),
                    version: "1.0.0".parse().unwrap(),
                    artifact: harmos_runtime::guest::PluginArtifact::Embedded { revision: "fixture".into() },
                    requires: vec![],
                    configuration: b"null".to_vec(),
                    contributions: Default::default(),
                },
                Default::default(),
                harmos_runtime::operations::Registration::new(harmos_runtime::operations::Lane::default())
                    .entry(read_contract_declaration_operation().unwrap()),
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
    crate::support::app::shutdown(&runtime).await.unwrap();
}

fn payload(value: &str) -> Vec<u8> {
    protobuf(&Text { value: value.to_owned() })
}

fn protobuf<T: harmos_runtime::Portable>(value: &T) -> Vec<u8> {
    let mut bytes = Vec::new();
    harmos_operations::execution::grpc::encode(value, &mut bytes).unwrap();
    bytes
}

fn decode_protobuf<T: harmos_runtime::Portable>(bytes: &[u8]) -> T {
    harmos_operations::execution::grpc::decode(harmos_operations::execution::grpc::DecodeInput::new(bytes)).unwrap()
}

/* === Tests === */

#[tokio::test]
async fn wire_registration_uses_supplied_schema_snapshot() {
    use harmos_runtime::operations::Contract;
    let declaration = READ_CONTRACT_DECLARATION_OPERATION.wire().contract().unwrap();
    let tiny = harmos_runtime::config::SchemaSettings {
        preparation_bytes: std::num::NonZeroUsize::MIN,
        ..Default::default()
    };
    assert!(READ_CONTRACT_DECLARATION_OPERATION.wire().contract_with_settings(&tiny).is_err());
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
            let runtime = crate::support::app::launch::<App>((), (), |setup| {
                setup.resource::<Resources>(resources.clone());
                setup.settings().schema_settings(settings);
                if initial {
                    setup.operation(
                        read_contract_declaration_operation().expect("representable operation"),
                        harmos_runtime::operations::Lane::default(),
                    );
                }
            })
            .await;
            if initial || !accepted {
                assert_eq!(runtime.is_ok(), accepted);
                if let Ok(runtime) = runtime {
                    assert!(runtime.operations.definition(READ_CONTRACT_DECLARATION_OPERATION.entry().id).is_ok());
                    crate::support::app::shutdown(&runtime).await.unwrap();
                }
            } else {
                let runtime = runtime.unwrap();
                let result = runtime
                    .operations
                    .register_operation(
                        read_contract_declaration_operation().expect("representable operation"),
                        harmos_runtime::operations::Lane::default(),
                    )
                    .await;
                assert_eq!(result.is_ok(), accepted);
                assert_eq!(runtime.operations.definition(READ_CONTRACT_DECLARATION_OPERATION.entry().id).is_ok(), accepted);
                crate::support::app::shutdown(&runtime).await.unwrap();
            }
        }
    }
}
