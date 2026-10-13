/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

mod hygiene {
    mod harmos {}

    #[::harmos::lanes]
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
use harmos::operations::{Context, Operation, OperationPolicy, RetryPolicy};
use harmos::{Runtime, operations::Capacity, operations::Lane};

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
#[harmos::lanes]
enum Lanes {
    #[default]
    Open,
    #[lane(serial)]
    Serial,
    #[lane(concurrent(2))]
    Concurrent,
    #[lane(unbounded)]
    Unbounded,
}

#[harmos::operation(
        request = (),
        response = (),
        id = "lanes.measure",
        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::Never,
        },
    )]
struct MeasureContract;

#[harmos::catalog]
enum Operations {
    MeasureContract(MeasureContract),
}

/* === Traits === */

/* === Implementations === */
impl Operation for Measure {
    type Declaration = MeasureContract;
    type Resources = Gauge;
    type Error = std::io::Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    fn new(resources: Arc<Gauge>) -> Result<Self, Self::Error> {
        Ok(Self(resources))
    }

    async fn run<'ctx>(
        &mut self,
        _context: Self::Context<'ctx>,
        _request: &mut (),
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<()>, Self::Error> {
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
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_operation::<Measure>("app", lane, gauge.clone())
        .unwrap();
    let instances: Vec<_> = (0..count)
        .map(|_| runtime.operations.submit::<MeasureContract>(()).unwrap())
        .collect();

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

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("capacity_bounds_admission", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    assert_admission(Lanes::Serial, 3, 1).await;
                    assert_admission(Lanes::Concurrent, 3, 2).await;
                });
        });
    }
    {
        scenario_runtime::check("unbounded_lane_admits", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    assert_admission(Lanes::Unbounded, 8, 8).await;
                });
        });
    }
    {
        scenario_runtime::check("lane_admits_instances", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    assert_admission(Lanes::default(), 8, 8).await;
                });
        });
    }
    {
        scenario_runtime::check("catalog_resolves_crate", || {
            assert_eq!(hygiene::Lanes::Probe.name(), "Probe");
            assert_eq!(hygiene::Lanes::Probe.capacity(), Capacity::concurrent(2));
            assert!(format!("{:?}", hygiene::Lanes::catalog()).contains("Probe"));
            assert_eq!(hygiene::Lanes::default().name(), "Open");
        });
    }
    {
        scenario_runtime::check("capacities_match_variants", || {
            assert_eq!(Lanes::Serial.name(), "Serial");
            assert_eq!(Lanes::Serial.capacity(), Capacity::serial());
            assert_eq!(Lanes::Concurrent.capacity(), Capacity::concurrent(2));
            assert_eq!(Lanes::Unbounded.capacity(), Capacity::unbounded());
            assert_eq!(Lanes::default().name(), "Open");
            assert_eq!(Lanes::Open.capacity(), Capacity::unbounded());
        });
    }
}
