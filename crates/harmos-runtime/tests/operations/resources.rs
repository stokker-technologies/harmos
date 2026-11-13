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
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use std::time::Duration;

// Third Party Imports

// Local Imports
use super::app::App;
use harmos_runtime::operations::RetryPolicy;
use harmos_runtime::{Runtime, state::Position};

/* === Structs === */

struct Bench {
    device: String,
    reads: AtomicUsize,
}

/* === Enums === */

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Reading)]
#[derive(Clone)]
struct ReadContractDeclaration {
    nested: bool,
}

static READ_CONTRACT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<ReadContractDeclaration>(
        "resources.read",
        harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    );

static READ_CONTRACT_DECLARATION_CALLEES: [harmos_runtime::operations::WireDeclaration; 1] =
    [READ_CONTRACT_DECLARATION_DESCRIPTION.copied()];
const READ_CONTRACT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<ReadContractDeclaration> =
    harmos_runtime::operations::OperationSpec::<ReadContractDeclaration>::new(&READ_CONTRACT_DECLARATION_DESCRIPTION)
        .grant(|_: &ReadContractDeclaration| Some(harmos_operations::plugin::Grant::of("resources.read", "device")))
        .calls(&READ_CONTRACT_DECLARATION_CALLEES);

// The Device A Read Saw And How Many Reads Preceded It.
#[derive(harmos_runtime::Message, Clone, Debug, Default)]
struct Reading {
    device: String,
    seen: u64,
}

fn operations_catalog() -> harmos_runtime::operations::Catalog {
    operations_catalog_batch().named(concat!(module_path!(), "::Operations"))
}

fn operations_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(read_contract_declaration_operation().expect("representable operation"));
    batch
}

/* === Implementations === */

async fn read_contract_declaration_handler(
    request: ReadContractDeclaration, ctx: harmos_runtime::operations::FunctionContext<ReadContractDeclaration>,
) -> Result<harmos_runtime::Response<Reading>, harmos_runtime::operations::InvocationError> {
    let resources = ctx.resource::<Bench>().expect("declared resource is registered");
    let seen = u64::try_from(resources.reads.fetch_add(1, Ordering::SeqCst)).expect("read counts fit in u64");
    if request.nested {
        let child = ctx.operation(READ_CONTRACT_DECLARATION_OPERATION, ReadContractDeclaration { nested: false }).await?;
        assert_eq!(child.device, resources.device);
        assert_eq!(child.seen, seen + 1);
    }
    Ok(Reading { device: resources.device.clone(), seen }.into())
}

fn read_contract_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        READ_CONTRACT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Bench>()] },
        read_contract_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

/* === Tests === */

#[tokio::test]
async fn runtime_selected_ids() {
    use harmos_runtime::operations::Registration;

    #[derive(harmos_runtime::Message, Debug, Default)]
    #[harmos_path(harmos_runtime)]
    #[derive(harmos_runtime::Request)]
    #[request(response = Reading)]
    #[derive(Clone)]
    struct OtherRead {
        nested: bool,
    }

    let resources = Arc::new(Bench {
        device: "configured".into(),
        reads: AtomicUsize::new(0),
    });
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<Bench>(resources.clone());
        setup.catalog(operations_catalog(), harmos_runtime::operations::Lane::default());
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: std::num::NonZeroUsize::new(4096),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    let id = format!("devices.rig_{}.read", 7);
    let registration = Registration::new(harmos_runtime::operations::Lane::default())
        .operation(read_contract_declaration_operation().expect("representable operation"), &id)
        .unwrap();
    runtime.register_batch(registration.clone()).await.unwrap();

    let definition = runtime.operations.definition(&id).unwrap();
    assert_eq!(definition, READ_CONTRACT_DECLARATION_OPERATION.named(&id).unwrap());
    assert_eq!(definition.policy, READ_CONTRACT_DECLARATION_OPERATION.entry().policy);
    assert_ne!(definition.fingerprint, READ_CONTRACT_DECLARATION_OPERATION.definition().expect("valid operation").fingerprint);
    assert!(runtime.operations.definitions().contains(&definition));
    assert!(matches!(
        runtime.operations.submit_at::<OtherRead>(&id, OtherRead { nested: false }),
        Err(harmos_runtime::Error::InvalidInput { field: "operation", .. })
    ));
    let typed =
        runtime.operations.submit_at::<ReadContractDeclaration>(&id, ReadContractDeclaration { nested: false }).unwrap().await.unwrap();
    assert_eq!((typed.device.as_str(), typed.seen), ("configured", 0));
    let qualified = format!("host.{id}");
    let mut payload = Vec::new();
    harmos_operations::execution::grpc::encode(&ReadContractDeclaration { nested: false }, &mut payload).unwrap();
    let bytes = runtime.operations.call_encoded(&qualified, payload).await.unwrap();
    let encoded: Reading =
        harmos_operations::execution::grpc::decode(harmos_operations::execution::grpc::DecodeInput::new(&bytes)).unwrap();
    assert_eq!((encoded.device.as_str(), encoded.seen), ("configured", 1));

    assert!(matches!(runtime.register_batch(registration).await, Err(harmos_runtime::Error::AlreadyRegistered { .. })));
    let collision = Registration::new(harmos_runtime::operations::Lane::default())
        .operation(read_contract_declaration_operation().expect("representable operation"), READ_CONTRACT_DECLARATION_OPERATION.entry().id)
        .unwrap();
    assert!(matches!(runtime.register_batch(collision).await, Err(harmos_runtime::Error::AlreadyRegistered { .. })));
    let duplicate = Registration::new(harmos_runtime::operations::Lane::default())
        .operation(read_contract_declaration_operation().expect("representable operation"), "devices.duplicate")
        .unwrap()
        .operation(read_contract_declaration_operation().expect("representable operation"), "devices.duplicate")
        .unwrap();
    assert!(matches!(runtime.register_batch(duplicate).await, Err(harmos_runtime::Error::AlreadyRegistered { .. })));
    assert!(runtime.operations.definition("devices.duplicate").is_err());
    assert_eq!(resources.reads.load(Ordering::SeqCst), 2);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn needs_lacks_registration() {
    let runtime = Runtime::<App>::launch((), ()).await.unwrap();
    assert_eq!(crate::support::app::shutdown(&runtime).await.unwrap(), Position::ORIGIN);
}

#[tokio::test]
async fn nested_resources_shared() {
    let resources = Arc::new(Bench {
        device: "sim://rig-a".to_owned(),
        reads: AtomicUsize::new(0),
    });
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<Bench>(resources.clone());
    })
    .await
    .unwrap();
    crate::support::app::register(&runtime, operations_catalog(), harmos_runtime::operations::Lane::default()).await.unwrap();

    let parent = runtime.operation(ReadContractDeclaration { nested: true }).start().await.unwrap().await.unwrap();
    assert_eq!(parent.device, "sim://rig-a");
    assert_eq!(parent.seen, 0);
    let following = runtime.operation(ReadContractDeclaration { nested: false }).start().await.unwrap().await.unwrap();
    assert_eq!(following.seen, 2, "parent, child and following attempt share one value");
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn private_grpc_refused() {
    use harmos_runtime::operations::{Grpc, NoStream, Registration};

    #[derive(harmos_runtime::Message, Debug, Default, Clone)]
    struct Private;

    impl harmos_operations::operations::RequestShape for Private {
        type Request = Self;
        type Response = ();
        type Input = NoStream;
        type Output = NoStream;
    }
    static PRIVATE_DESCRIPTION: harmos_operations::operations::WireDeclaration =
        harmos_operations::operations::WireDeclaration::request::<Private>(
            "resources.private_grpc",
            READ_CONTRACT_DECLARATION_OPERATION.entry().policy,
        )
        .grpc(Grpc::new("resources.v1", "Private", "Call"));

    const PRIVATE_OPERATION: harmos_operations::operations::OperationSpec<Private> =
        harmos_operations::operations::OperationSpec::<Private>::new(&PRIVATE_DESCRIPTION);

    async fn private_handler(
        _request: Private, _: harmos_runtime::operations::FunctionContext<Private>,
    ) -> Result<harmos_runtime::Response<()>, std::convert::Infallible> {
        unreachable!("Private gRPC operations must never activate")
    }

    fn private_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError>
    {
        harmos_runtime::operations::OperationFactory::function(PRIVATE_OPERATION, &[], private_handler, |_| {
            harmos_runtime::operations::FailureKind::Permanent
        })
    }

    assert!(matches!(PRIVATE_OPERATION.wire().contract(), Err(harmos_runtime::operations::ContractError::RestrictedGrpc { .. })));
    let runtime = Runtime::<App>::launch((), ()).await.unwrap();
    let registration = Registration::new(harmos_runtime::operations::Lane::default())
        .operation(private_operation().expect("representable operation"), PRIVATE_OPERATION.entry().id)
        .unwrap();
    let error = runtime.register_batch(registration).await.unwrap_err();
    let harmos_runtime::Error::OperationFailed { source, .. } = error else { panic!("unexpected error: {error:?}") };
    assert!(matches!(
        source.downcast_ref::<harmos_runtime::operations::ContractError>(),
        Some(harmos_runtime::operations::ContractError::RestrictedGrpc { .. })
    ));
    assert!(runtime.operations.definition(PRIVATE_OPERATION.entry().id).is_err());
    crate::support::app::shutdown(&runtime).await.unwrap();
}
