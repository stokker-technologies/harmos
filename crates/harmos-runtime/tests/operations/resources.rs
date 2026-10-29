/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use std::time::Duration;

use super::{Lanes, app::App};
use harmos_runtime::operations::RetryPolicy;
use harmos_runtime::{Runtime, state::Position};

struct Bench {
    device: String,
    reads: AtomicUsize,
}

#[harmos_runtime::operation(resources = [Bench], operations = [ReadContractDeclaration], response = Reading,
        id = "resources.read", grant = |_: &ReadContractDeclaration| Some(harmos_domain::plugin::Grant::of("resources.read", "device")),
        policy = harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::Never,
        },
    )]
struct ReadContractDeclaration {
    nested: bool,
}

// The Device A Read Saw And How Many Reads Preceded It.
#[harmos_runtime::message]
struct Reading {
    device: String,
    seen: u64,
}

#[harmos_runtime::catalog]
enum Operations {
    ReadContractDeclaration(ReadContractDeclaration),
}

impl harmos_runtime::Operation for ReadContractDeclaration {
    type Error = harmos_runtime::operations::InvocationError;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Reading>, Self::Error> {
        let resources = ctx.resource::<Bench>();
        let seen = u64::try_from(resources.reads.fetch_add(1, Ordering::SeqCst)).expect("read counts fit in u64");
        if self.nested {
            let child = ctx.operation(ReadContractDeclaration { nested: false }).await?;
            assert_eq!(child.device, resources.device);
            assert_eq!(child.seen, seen + 1);
        }
        Ok(Reading { device: resources.device.clone(), seen }.into())
    }
}

#[tokio::test]
async fn runtime_selected_ids() {
    use harmos_runtime::Message;
    use harmos_runtime::operations::{OperationDeclaration, OperationDefinition, Registration};

    #[harmos_runtime::operation(id = "resources.other", response = Reading, policy = ReadContractDeclaration::POLICY)]
    struct OtherRead {
        nested: bool,
    }

    let resources = Arc::new(Bench {
        device: "configured".into(),
        reads: AtomicUsize::new(0),
    });
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Bench>(resources.clone());
        setup.operations::<Operations>(Lanes::Open);
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: std::num::NonZeroUsize::new(4096),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    let id = format!("devices.rig_{}.read", 7);
    let registration = Registration::new(Lanes::Open).operation::<ReadContractDeclaration>(&id).unwrap();
    runtime.register_batch(registration.clone()).await.unwrap();

    let definition = runtime.operations.definition(&id).unwrap();
    assert_eq!(definition, OperationDefinition::named::<ReadContractDeclaration>(&id).unwrap());
    assert_eq!(definition.policy, ReadContractDeclaration::POLICY);
    assert_ne!(definition.fingerprint, ReadContractDeclaration::definition().fingerprint);
    assert!(runtime.operations.definitions().contains(&definition));
    assert!(matches!(
        runtime.operation(OtherRead { nested: false }).id(&id).run().await,
        Err(harmos_runtime::Error::InvalidInput { field: "operation", .. })
    ));
    let typed = runtime.handle().operation(ReadContractDeclaration { nested: false }).id(&id).run().await.unwrap();
    assert_eq!((typed.device.as_str(), typed.seen), ("configured", 0));
    let qualified = format!("host.{id}");
    let bytes = runtime.operations.call_encoded(&qualified, ReadContractDeclaration { nested: false }.encode()).await.unwrap();
    let encoded = Reading::decode(&bytes).unwrap();
    assert_eq!((encoded.device.as_str(), encoded.seen), ("configured", 1));

    assert!(matches!(runtime.register_batch(registration).await, Err(harmos_runtime::Error::AlreadyRegistered { .. })));
    let collision = Registration::new(Lanes::Open).operation::<ReadContractDeclaration>(ReadContractDeclaration::ID).unwrap();
    assert!(matches!(runtime.register_batch(collision).await, Err(harmos_runtime::Error::AlreadyRegistered { .. })));
    let duplicate = Registration::new(Lanes::Open)
        .operation::<ReadContractDeclaration>("devices.duplicate")
        .unwrap()
        .operation::<ReadContractDeclaration>("devices.duplicate")
        .unwrap();
    assert!(matches!(runtime.register_batch(duplicate).await, Err(harmos_runtime::Error::AlreadyRegistered { .. })));
    assert!(runtime.operations.definition("devices.duplicate").is_err());
    assert_eq!(resources.reads.load(Ordering::SeqCst), 2);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn needs_lacks_registration() {
    let runtime = Runtime::<App>::launch((), ()).await.unwrap();
    assert_eq!(runtime.shutdown().await.unwrap(), Position::ORIGIN);
}

#[tokio::test]
async fn nested_resources_shared() {
    let resources = Arc::new(Bench {
        device: "sim://rig-a".to_owned(),
        reads: AtomicUsize::new(0),
    });
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Bench>(resources.clone());
    })
    .await
    .unwrap();
    runtime.operations.register::<Operations>(Lanes::Open).await.unwrap();

    let parent = runtime.operation(ReadContractDeclaration { nested: true }).start().await.unwrap().await.unwrap();
    assert_eq!(parent.device, "sim://rig-a");
    assert_eq!(parent.seen, 0);
    let following = runtime.operation(ReadContractDeclaration { nested: false }).start().await.unwrap().await.unwrap();
    assert_eq!(following.seen, 2, "parent, child and following attempt share one value");
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn private_grpc_refused() {
    use harmos_runtime::operations::{Grpc, NoStream, OperationDeclaration, Registration, Resource, WireDeclaration};

    #[harmos_runtime::message]
    #[derive(Clone)]
    struct Private;

    impl OperationDeclaration for Private {
        type Request = Self;
        type Response = ();
        type Input = NoStream;
        type Output = NoStream;
        const ID: &'static str = "resources.private_grpc";
        const POLICY: harmos_runtime::OperationPolicy = ReadContractDeclaration::POLICY;
        const GRPC: Option<Grpc> = Some(Grpc::new("resources.v1", "Private", "Call"));
    }

    impl harmos_runtime::Dependent for Private {
        const RESOURCES: &'static [Resource] = &[];
    }

    impl harmos_runtime::Operation for Private {
        type Error = std::convert::Infallible;
        async fn run(self, _: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<()>, Self::Error> {
            unreachable!("Private gRPC operations must never activate")
        }
    }

    assert!(matches!(WireDeclaration::of::<Private>().contract(), Err(harmos_runtime::operations::ContractError::PrivateGrpc { .. })));
    let runtime = Runtime::<App>::launch((), ()).await.unwrap();
    let registration = Registration::new(Lanes::Open).operation::<Private>(Private::ID).unwrap();
    let error = runtime.register_batch(registration).await.unwrap_err();
    let harmos_runtime::Error::OperationFailed { source, .. } = error else { panic!("unexpected error: {error:?}") };
    assert!(matches!(
        source.downcast_ref::<harmos_runtime::operations::ContractError>(),
        Some(harmos_runtime::operations::ContractError::PrivateGrpc { .. })
    ));
    assert!(runtime.operations.definition(Private::ID).is_err());
    runtime.shutdown().await.unwrap();
}
