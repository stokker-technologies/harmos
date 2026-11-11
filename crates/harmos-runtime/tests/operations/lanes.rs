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

#[harmos_runtime::operation(resources = [Gauge, harmos_runtime::operations::HostContext], response = (),
        id = "lanes.measure",
        policy = harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    )]
struct MeasureContractDeclaration {}

/* === Enums === */

#[harmos_runtime::catalog]
enum Operations {
    MeasureContractDeclaration(MeasureContractDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Operation for MeasureContractDeclaration {
    type Error = std::io::Error;

    async fn run(self, _context: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Self::Error> {
        let resources = _context.resource::<Gauge>();
        let host = _context.resource::<harmos_runtime::operations::HostContext>();
        let _context = host.borrow();
        let result: ::core::result::Result<(), Self::Error> = async move {
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
}

/* === Public Functions === */

/* === Private Functions === */

async fn assert_admission(lane: Lane, count: usize, admitted: usize) {
    let (entered, mut receiver) = mpsc::channel(count);
    let gauge = Arc::new(Gauge {
        active: AtomicUsize::new(0),
        maximum: AtomicUsize::new(0),
        ran: AtomicUsize::new(0),
        entered,
        release: Semaphore::new(0),
    });
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Gauge>(gauge.clone());
    })
    .await
    .unwrap();
    runtime.operations.register::<Operations>(lane).await.unwrap();
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
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn capacity_bounds_admission() {
    assert_admission(crate::support::SERIAL, 3, 1).await;
    assert_admission(crate::support::CONCURRENT, 3, 2).await;
}

#[tokio::test]
async fn unbounded_lanes_admit() {
    // The default lane is unbounded like an explicit unbounded lane.
    for lane in [crate::support::UNBOUNDED, harmos_runtime::operations::Lane::default()] {
        assert_admission(lane, 8, 8).await;
    }
}

/* === Tests === */

#[test]
fn lane_values_refuse_duplicates_and_invalid_capacity() {
    let lane = harmos_runtime::operations::Lane::new("worker", Capacity::serial());
    let duplicate = harmos_runtime::operations::LaneSet::new([lane, lane]);
    assert!(matches!(duplicate, Err(harmos_runtime::Error::InvalidLane { name: "worker", .. })));
    let invalid = harmos_runtime::operations::Lane::new("worker", Capacity::concurrent(0));
    assert!(matches!(harmos_runtime::operations::LaneSet::new([invalid]), Err(harmos_runtime::Error::InvalidLane { name: "worker", .. })));
    assert!(harmos_runtime::operations::LaneSet::new([lane]).is_ok());
}
