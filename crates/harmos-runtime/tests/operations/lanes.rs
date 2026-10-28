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

/* === Enums === */

use super::support::Lanes;

#[harmos_runtime::operation(resources = [Gauge, harmos_runtime::operations::HostContext], response = (),
        id = "lanes.measure",
        policy = harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::Never,
        },
    )]
struct MeasureContractDeclaration {}

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

async fn assert_admission(lane: Lanes, count: usize, admitted: usize) {
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
