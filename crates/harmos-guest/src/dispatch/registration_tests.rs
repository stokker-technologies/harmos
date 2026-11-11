/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use super::Table as WireTable;
use harmos_operations::ServicePolicy;
use harmos_operations::execution::__private::{Catalog, ResourceSet, ServiceFuture};
use harmos_operations::execution::operations::*;
use harmos_operations::execution::services::Service;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Worker(Arc<AtomicUsize>);

pub const READ_CATALOG_CATALOG: CatalogDescription = CatalogDescription::new("ReadCatalog", &[], &[], Some(read_catalog_catalog_batch));

pub fn read_catalog_catalog_batch() -> Catalog {
    Catalog::new().entry(read_operation().expect("representable operation"))
}

#[derive(harmos_operations::execution::Message, Debug, Default)]
#[harmos_path(harmos_operations::execution)]
#[derive(harmos_operations::execution::Request)]
#[request(response = u64)]
#[derive(Clone)]
struct Read;

static READ_DESCRIPTION: harmos_operations::execution::operations::WireDeclaration =
    harmos_operations::execution::operations::WireDeclaration::request::<Read>(
        "fixture.read",
        harmos_operations::execution::OperationPolicy {
            timeout: std::time::Duration::from_secs(1),
            attempt_timeout: std::time::Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: harmos_operations::execution::RetryPolicy::Never,
        },
    );

const READ_OPERATION: harmos_operations::execution::operations::OperationSpec<Read> =
    harmos_operations::execution::operations::OperationSpec::<Read>::new(&READ_DESCRIPTION);

async fn read_handler(
    _request: Read, ctx: harmos_operations::execution::operations::FunctionContext<Read>,
) -> Result<harmos_operations::execution::Response<u64>, std::convert::Infallible> {
    Ok(harmos_operations::execution::Response::from_owner(ctx.resource::<u64>().expect("declared resource is registered")))
}

fn read_operation() -> ::core::result::Result<
    harmos_operations::execution::operations::OperationFactory,
    harmos_operations::execution::operations::ServingError,
> {
    harmos_operations::execution::operations::OperationFactory::function(
        READ_OPERATION,
        const { &[harmos_operations::execution::Resource::of::<u64>()] },
        read_handler,
        |_| harmos_operations::execution::operations::FailureKind::Permanent,
    )
}

fn ready<T>(mut future: ServiceFuture<'_, T>) -> T {
    let mut task = std::task::Context::from_waker(std::task::Waker::noop());
    match future.as_mut().poll(&mut task) {
        std::task::Poll::Ready(Ok(value)) => value,
        _ => panic!("synchronous fixture must complete successfully"),
    }
}

#[test]
fn named_factory_contract() {
    let id = format!("fixture.device_{}", 7);
    let batch = Catalog::new().entry_named(read_operation().expect("representable operation"), &id).unwrap();
    let factory = &batch.operations[0];
    let definition = READ_OPERATION.named(&id).unwrap();
    assert_eq!(factory.definition, definition);
    assert_ne!(definition.fingerprint, READ_OPERATION.definition().expect("valid operation").fingerprint);
    let contract = factory.contract().unwrap();
    assert_eq!(contract.operation(), &definition);
    assert_eq!(contract.schema(), READ_OPERATION.wire().contract().unwrap().schema());
    assert_eq!(contract.operation().policy, READ_OPERATION.entry().policy);
    let granted = factory.grant(Some(harmos_operations::plugin::Grant::of(READ_OPERATION.entry().id, "device"))).unwrap();
    assert_eq!((granted.operation(), granted.scope()), (id.as_str(), Some("device")));
    let foreign = harmos_operations::plugin::Grant::of("foreign.operation", "device");
    assert_eq!(factory.grant(Some(foreign.clone())), Some(foreign));

    let mut table = WireTable::new("fixture").unwrap();
    table.provide_resource(std::sync::Arc::new(7_u64)).unwrap();
    table.register(batch.clone()).unwrap();
    table.validate_binding().unwrap();
    table.validate(std::slice::from_ref(&definition)).unwrap();
    assert!(table.register(batch.clone()).is_err());

    let mut changed = factory.clone();
    changed.definition.id = harmos_operations::execution::Identifier::new("fixture.changed").unwrap();
    assert!(changed.validate().is_err());
    let mut changed = factory.clone();
    changed.definition.fingerprint.0[0] ^= 1;
    assert!(changed.validate().is_err());
    let mut changed = factory.clone();
    changed.definition.policy.timeout += READ_OPERATION.entry().policy.attempt_timeout;
    assert!(changed.validate().is_err());

    for id in ["", "bad-name", "harmos.initialize"] {
        assert!(Catalog::new().entry_named(read_operation().expect("representable operation"), id).is_err());
    }
}

#[test]
fn carrier_assignments_are_immutable() {
    use harmos_operations::execution::operations::{AssignedOperation, OperationAssignments};
    use prost::Message as _;
    let first = AssignedOperation {
        operation: format!("fixture.{}", READ_OPERATION.entry().id),
        fingerprint: READ_OPERATION.definition().expect("valid operation").fingerprint.0.to_vec(),
        generation: 1,
    };
    let assignments = OperationAssignments::default();
    let bytes = harmos_protocol::host::OperationAssignments { operations: vec![first.clone()] }.encode_to_vec();
    assignments.install(&bytes).unwrap();
    assignments.clone().install(&bytes).unwrap();
    assert_eq!(assignments.resolve::<Read>().unwrap(), &first);
    let mut replacement = first.clone();
    replacement.generation = 2;
    assert!(assignments.install(&harmos_protocol::host::OperationAssignments { operations: vec![replacement] }.encode_to_vec()).is_err());
    assert_eq!(assignments.resolve::<Read>().unwrap().generation, 1);
    let mut wrong = first.clone();
    wrong.fingerprint[0] ^= 1;
    let incompatible = OperationAssignments::default();
    incompatible.install(&harmos_protocol::host::OperationAssignments { operations: vec![wrong] }.encode_to_vec()).unwrap();
    assert!(incompatible.resolve::<Read>().is_err());
    let duplicate = AssignedOperation {
        operation: format!("other.{}", READ_OPERATION.entry().id),
        ..first.clone()
    };
    assert!(
        OperationAssignments::default()
            .install(&harmos_protocol::host::OperationAssignments { operations: vec![first, duplicate] }.encode_to_vec())
            .is_err()
    );
}

#[test]
fn encoded_factory_preserves_response() {
    use harmos_operations::execution::Message;

    let mut dependencies = ResourceSet::new();
    dependencies.insert(Arc::new(42u64)).unwrap();
    let batch = Catalog::new().entry(read_operation().expect("representable operation"));
    let mut prepared = batch.operations[0].prepare_encoded(dependencies, &Read.encode()).unwrap();
    let context = ExecutionContext::new(ResourceSet::new(), harmos_operations::execution::operations::CallContext::default());
    ready(prepared.instance.initialize(context.clone()));
    let response = ready(prepared.instance.run(context)).restore::<Vec<u8>>().unwrap();
    assert_eq!(u64::decode(&response).unwrap(), 42);
    ready(prepared.instance.stop());
    ready(prepared.instance.cleanup());
}

impl Service for Worker {
    type Error = harmos_operations::execution::ResourceError;

    async fn initialize(context: ExecutionContext) -> Result<Self, Self::Error> {
        let counter = context.resource::<AtomicUsize>()?;
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(Self(counter))
    }

    async fn run(&mut self, _: ExecutionContext) -> Result<(), Self::Error> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn factory_rejects_a_substituted_declaration() {
    let mut batch = Catalog::new().entry(read_operation().expect("representable operation"));
    batch.operations[0].definition.id = harmos_operations::execution::Identifier::new("different").unwrap();
    assert!(matches!(batch.operations[0].validate(), Err(ServingError::Catalog)));
}

#[test]
fn wire_identity_fences_replacement_before_decoding() {
    use harmos_operations::execution::operations::ServingError;
    use harmos_operations::execution::{Message, QualifiedId};
    let batch = Catalog::new().entry(read_operation().expect("representable operation"));
    let fingerprint = READ_OPERATION.definition().unwrap().fingerprint.0;
    let mut table = WireTable::new("fixture").unwrap();
    table.resource(42u64);
    table.capacity(std::num::NonZeroUsize::MIN);
    table.register(batch.clone()).unwrap();
    table.validate_binding().unwrap();
    assert!(matches!(table.prepare(1, "spoof.fixture.read", fingerprint, 1, Read.encode()), Err(ServingError::Unknown { .. })));
    assert!(matches!(table.prepare(1, "fixture.fixture.read", [0; 32], 1, vec![255]), Err(ServingError::Identity { .. })));
    assert!(matches!(table.prepare(1, "fixture.fixture.read", fingerprint, 0, vec![255]), Err(ServingError::Identity { .. })));
    table.prepare(1, "fixture.fixture.read", fingerprint, 7, Read.encode()).unwrap();
    ready(Box::pin(table.unregister(&QualifiedId::qualify("fixture", "fixture.read").unwrap())));
    table.register(batch).unwrap();
    assert!(matches!(table.prepare(2, "fixture.fixture.read", fingerprint, 7, vec![255]), Err(ServingError::Identity { .. })));
    table.prepare(2, "fixture.fixture.read", fingerprint, 8, Read.encode()).unwrap();
    assert!(matches!(table.prepare(3, "fixture.fixture.read", fingerprint, 9, vec![255]), Err(ServingError::Identity { .. })));
    ready(Box::pin(table.settle()));
}

#[test]
fn service_factory_reconstructs_from_its_snapshot() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut dependencies = ResourceSet::new();
    dependencies.insert(counter.clone()).unwrap();
    let policy = ServicePolicy {
        stop_timeout: std::time::Duration::from_secs(1),
        restart: harmos_operations::execution::RestartPolicy::Never,
    };
    let batch = Catalog::new().service::<Worker>("worker", policy);
    let factory = &batch.services[0];
    let context = ExecutionContext::new(dependencies, harmos_operations::execution::operations::CallContext::default());
    let mut first = factory.initialize(context.clone());
    let mut second = factory.initialize(context);
    let mut task = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(matches!(first.as_mut().poll(&mut task), std::task::Poll::Ready(Ok(_))));
    assert!(matches!(second.as_mut().poll(&mut task), std::task::Poll::Ready(Ok(_))));
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn binding_errors_aggregate() {
    use harmos_operations::execution::StartupError;
    let mut table = WireTable::new("fixture").unwrap();
    table.resource(7u32).resource(8u32);
    table.catalog(&READ_CATALOG_CATALOG).catalog(&READ_CATALOG_CATALOG);
    assert!(table.definitions().is_empty());

    let Err(ServingError::Startup(error)) = table.validate_binding() else {
        panic!("invalid binding must be refused");
    };
    let StartupError::Multiple { errors } = *error else {
        panic!("independent binding errors must be aggregated");
    };
    assert_eq!(errors.len(), 3);
    assert!(errors.iter().any(|error| matches!(error, StartupError::DuplicateResource { .. })));
    assert!(errors.iter().any(|error| matches!(error, StartupError::DuplicateCatalog { .. })));
    assert!(errors.iter().any(|error| matches!(error, StartupError::MissingResource { .. })));
    assert!(table.definitions().is_empty());
    assert_eq!(*table.context().resource::<u32>().unwrap(), 7);
}

#[test]
fn frozen_dynamic_resources() {
    let mut table = WireTable::new("fixture").unwrap();
    let owner = Arc::new(41u32);
    table.resource::<u32>(owner.clone());
    table.validate_binding().unwrap();
    assert!(Arc::ptr_eq(&owner, &table.context().resource::<u32>().unwrap()));

    let error = table.register(read_catalog_catalog_batch()).unwrap_err();
    assert!(matches!(error, ServingError::Resource { .. }));
    table.resource(42u64);
    assert!(table.validate_binding().is_err());
    assert!(table.context().resource::<u64>().is_err());
    assert!(table.definitions().is_empty());
}

#[test]
fn resource_order_independence() {
    let mut table = WireTable::new("fixture").unwrap();
    table.catalog(&READ_CATALOG_CATALOG).resource(42u64);
    assert!(table.definitions().is_empty());
    table.validate_binding().unwrap();
    assert_eq!(table.definitions().len(), 1);
    assert_eq!(*table.context().resource::<u64>().unwrap(), 42);
}
