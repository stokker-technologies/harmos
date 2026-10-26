/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod hygiene {
    mod harmos {}

    #[::harmos_runtime::lanes]
    pub enum Lanes {
        #[default]
        Open,

        #[lane(concurrent(2))]
        Probe,
    }
}

/* === Re-Exports === */

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
use harmos_runtime::operations::{Operation, RetryPolicy};
use harmos_runtime::{Runtime, operations::Capacity, operations::Lane};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Gauge {
    active: AtomicUsize,
    maximum: AtomicUsize,
    ran: AtomicUsize,
    entered: mpsc::Sender<()>,
    release: Semaphore,
}

struct Measure(Arc<Gauge>);

/* === Enums === */

use super::support::Lanes;

#[harmos_runtime::operation(request = MeasureContract, infer_request,
        response = (),
        id = "lanes_measure",

        policy = harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::Never,
        },
    )]
struct MeasureContractDeclaration;

#[harmos_runtime::message]

struct MeasureContract {}

#[harmos_runtime::catalog]
enum Operations {
    MeasureContract(MeasureContractDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl Operation for Measure {
    type Request = MeasureContract;
    type Response = <MeasureContractDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = std::io::Error;
    type Endpoints = ();

    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Self::Error> {
        let resources = context.dependency::<Gauge>().expect("fixture registers its dependency");
        Ok(Self(resources))
    }

    async fn run(
        &mut self, _context: harmos_runtime::OperationContext, _request: &mut MeasureContract, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Self::Error> {
        let host = _context.dependency::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let _context = host.borrow();
        let result: ::core::result::Result<(), Self::Error> = async move {
            self.0.ran.fetch_add(1, Ordering::SeqCst);
            let active = self.0.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.0.maximum.fetch_max(active, Ordering::SeqCst);
            self.0.entered.send(()).await.unwrap();
            self.0.release.acquire().await.unwrap().forget();
            self.0.active.fetch_sub(1, Ordering::SeqCst);
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn assert_admission(lane: Lanes, count: usize, admitted: usize) {
    let (entered, mut receiver) = mpsc::channel(count);
    let gauge = Arc::new(Gauge {
        active: AtomicUsize::new(0),
        maximum: AtomicUsize::new(0),
        ran: AtomicUsize::new(0),
        entered,
        release: Semaphore::new(0),
    });
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(gauge.clone()).unwrap();
                dependencies
            })
            .implementation::<MeasureContractDeclaration, Measure>(),
            lane,
        ))
        .await
        .unwrap();
    let mut instances = Vec::new();
    for _ in 0..count {
        instances.push(runtime.operation(MeasureContract {}).start().await.unwrap());
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
    runtime.stop().await.unwrap();
}

/* === Tests === */

#[tokio::test]
async fn capacity_bounds_admission() {
    assert_admission(Lanes::Serial, 3, 1).await;
    assert_admission(Lanes::Concurrent, 3, 2).await;
}

#[tokio::test]
async fn unbounded_lanes_admit() {
    // The default lane is unbounded like an explicit unbounded lane.
    for lane in [Lanes::Unbounded, Lanes::default()] {
        assert_admission(lane, 8, 8).await;
    }
}

#[test]
fn catalog_resolves_crate() {
    assert_eq!(hygiene::Lanes::Probe.name(), "Probe");
    assert_eq!(hygiene::Lanes::Probe.capacity(), Capacity::concurrent(2));
    assert!(format!("{:?}", hygiene::Lanes::catalog()).contains("Probe"));
    assert_eq!(hygiene::Lanes::default().name(), "Open");
}

#[test]
fn capacities_match_variants() {
    assert_eq!(Lanes::Serial.name(), "Serial");
    assert_eq!(Lanes::Serial.capacity(), Capacity::serial());
    assert_eq!(Lanes::Concurrent.capacity(), Capacity::concurrent(2));
    assert_eq!(Lanes::Unbounded.capacity(), Capacity::unbounded());
    assert_eq!(Lanes::default().name(), "Open");
    assert_eq!(Lanes::Open.capacity(), Capacity::unbounded());
}
