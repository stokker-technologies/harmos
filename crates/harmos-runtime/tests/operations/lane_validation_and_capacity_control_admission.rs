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
use tokio::sync::{Semaphore, mpsc};

// Local Imports
use super::app::App;
use harmos_runtime::operations::RetryPolicy;
use harmos_runtime::{operations::Capacity, operations::Lane};

/* === Structs === */

struct Gauge {
    active: AtomicUsize,
    maximum: AtomicUsize,
    ran: AtomicUsize,
    entered: mpsc::Sender<()>,
    release: Semaphore,
}

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = ())]
#[derive(Clone)]
struct MeasureContractDeclaration {}

static MEASURE_CONTRACT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<MeasureContractDeclaration>(
        "lanes.measure",
        harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    );

const MEASURE_CONTRACT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<MeasureContractDeclaration> =
    harmos_runtime::operations::OperationSpec::<MeasureContractDeclaration>::new(&MEASURE_CONTRACT_DECLARATION_DESCRIPTION);

/* === Enums === */

fn operations_catalog() -> harmos_runtime::operations::Catalog {
    operations_catalog_batch().named(concat!(module_path!(), "::Operations"))
}

fn operations_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(measure_contract_declaration_operation().expect("representable operation"));
    batch
}

/* === Implementations === */

async fn measure_contract_declaration_handler(
    _request: MeasureContractDeclaration, _context: harmos_runtime::operations::FunctionContext<MeasureContractDeclaration>,
) -> Result<harmos_runtime::operations::Response<()>, std::io::Error> {
    let resources = _context.resource::<Gauge>().expect("declared resource is registered");
    let host = _context.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let _context = host.borrow();
    let result: ::core::result::Result<(), std::io::Error> = async move {
        resources.ran.fetch_add(1, Ordering::SeqCst);
        let active = resources.active.fetch_add(1, Ordering::SeqCst) + 1;
        resources.maximum.fetch_max(active, Ordering::SeqCst);
        resources.entered.send(()).await.unwrap();
        resources.release.acquire().await.unwrap().forget();
        resources.active.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn measure_contract_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        MEASURE_CONTRACT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Gauge>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        measure_contract_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

/* === Tests === */

#[tokio::test]
async fn lane_validation_and_capacity_control_admission() {
    let lane = harmos_runtime::operations::Lane::new("worker", Capacity::serial());
    let duplicate = harmos_runtime::operations::LaneSet::new([lane, lane]);
    assert!(matches!(duplicate, Err(harmos_runtime::Error::InvalidLane { name: "worker", .. })));
    let invalid = harmos_runtime::operations::Lane::new("worker", Capacity::concurrent(0));
    assert!(matches!(harmos_runtime::operations::LaneSet::new([invalid]), Err(harmos_runtime::Error::InvalidLane { name: "worker", .. })));
    assert!(harmos_runtime::operations::LaneSet::new([lane]).is_ok());

    // Admit Only The Capacity Of Each Lane, Then Drain Every Queued Request.
    for (lane, count, admitted) in
        [(crate::support::SERIAL, 3, 1), (crate::support::CONCURRENT, 3, 2), (crate::support::UNBOUNDED, 8, 8), (Lane::default(), 8, 8)]
    {
        let (entered, mut receiver) = mpsc::channel(count);
        let gauge = Arc::new(Gauge {
            active: AtomicUsize::new(0),
            maximum: AtomicUsize::new(0),
            ran: AtomicUsize::new(0),
            entered,
            release: Semaphore::new(0),
        });
        let runtime = crate::support::app::launch::<App>((), (), |setup| {
            setup.resource::<Gauge>(gauge.clone());
        })
        .await
        .unwrap();
        crate::support::app::register(&runtime, operations_catalog(), lane).await.unwrap();
        let mut instances = Vec::new();
        for _ in 0..count {
            instances.push(runtime.operation(MeasureContractDeclaration {}).start().await.unwrap());
        }

        for _ in 0..admitted {
            receiver.recv().await.unwrap();
        }
        assert_eq!(gauge.active.load(Ordering::SeqCst), admitted);
        gauge.release.add_permits(count);
        for instance in instances {
            instance.await.unwrap();
        }
        assert_eq!(gauge.maximum.load(Ordering::SeqCst), admitted);
        assert_eq!(gauge.ran.load(Ordering::SeqCst), count);
        assert_eq!(gauge.active.load(Ordering::SeqCst), 0);
        crate::support::app::shutdown(&runtime).await.unwrap();
    }
}
