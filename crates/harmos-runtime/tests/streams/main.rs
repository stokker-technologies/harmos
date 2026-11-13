/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../streams/allocation.rs"]
mod allocation;

#[path = "retention.rs"]
mod retention;

#[cfg(feature = "plugin-guest")]
mod revisions;

#[cfg(feature = "plugin-guest")]
#[path = "../streams/listeners.rs"]
mod listeners;

#[path = "streams_macros.rs"]
mod streams_macros;

mod watch_delivery_accounts_for_rows_overruns_and_shutdown;

/* === Imports === */

// Standard Library Imports
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

// Third Party Imports
use serde::{Deserialize, Serialize};
use support::{Bank, Ledger, Pulse, Reading, deposit, pulse, streaming, streams};

// Local Imports
use crate::support;
use harmos_runtime::streams::StreamRow as _;
use harmos_runtime::{
    state::At, state::Position, storage::Batch, storage::Cursor, storage::StreamConsumer, streams::BeneathWindow, streams::Definition,
    streams::Item, streams::StreamManager,
};

/* === Structs === */

struct Selecting;

/* === Implementations === */

impl StreamConsumer<Bank> for Selecting {
    fn resume(&self) -> Cursor {
        Cursor::ORIGIN
    }

    fn selections(&self) -> Vec<Definition> {
        vec![Pulse::definition()]
    }

    async fn drain(&mut self, batch: Batch<Bank>) -> Cursor {
        batch.reaches()
    }
}

/* === Private Functions === */

async fn next<S: Clone + 'static>(watching: &mut (impl futures_core::Stream<Item = Item<S>> + Unpin)) -> Option<Item<S>> {
    std::future::poll_fn(|context| Pin::new(&mut *watching).poll_next(context)).await
}

#[tokio::test]
async fn history_respects_cuts() {
    {
        let runtime = streaming().await;

        // Advance the journal between publishes so stamps genuinely move.
        for sample in 1..=3u64 {
            runtime.state.commit(deposit(1)).await.expect("accepted");
            runtime.streams.publish(pulse(sample));
        }

        let complete: Vec<u64> = runtime
            .streams
            .query::<Pulse>(..)
            .await
            .expect("a window that never evicted answers any range completely")
            .map(|At { value, .. }| value.sample)
            .collect();
        assert_eq!(complete, vec![1, 2, 3]);

        // Nine more pushes the first rows out of the window of eight.
        for sample in 4..=12u64 {
            runtime.state.commit(deposit(1)).await.expect("accepted");
            runtime.streams.publish(pulse(sample));
        }

        assert_eq!(
            runtime.streams.query::<Pulse>(..).await.expect_err("an unbounded start reaches beneath the window"),
            BeneathWindow::Floor { floor: Position(4) },
            "the window cannot answer beneath its retained floor"
        );
        assert_eq!(
            runtime.streams.query::<Pulse>(Position::ORIGIN..).await.expect_err("the origin lies beneath the floor"),
            BeneathWindow::Floor { floor: Position(4) },
        );

        // A start the caller actually holds — the stamp of a row still retained —
        // is answerable, and completely so.
        let retained: Vec<u64> = runtime
            .streams
            .query::<Pulse>(Position(5)..)
            .await
            .expect("the range starts at a retained stamp above the floor")
            .map(|At { value, .. }| value.sample)
            .collect();
        assert_eq!(retained, vec![5, 6, 7, 8, 9, 10, 11, 12]);

        crate::support::app::shutdown(&runtime).await.unwrap();
    }
    {
        let runtime = streaming().await;

        let first = runtime.state.commit(deposit(10)).await.expect("accepted");
        // The journal now sits idle while rows keep flowing: two publishes at
        // different wall-clock moments carry the same unchanged stamp.
        runtime.streams.publish(pulse(1));
        runtime.streams.publish(pulse(2));

        let second = runtime.state.commit(deposit(20)).await.expect("accepted");
        runtime.streams.publish(pulse(3));

        let cut: Vec<u64> = runtime
            .streams
            .query::<Pulse>(..)
            .as_of(first.position)
            .await
            .expect("nothing was evicted")
            .map(|At { value, .. }| value.sample)
            .collect();
        assert_eq!(
            cut,
            vec![1, 2],
            "later wall-clock rows under the unchanged stamp witnessed state no \
             newer than the cutoff, so both are included and the later row is not"
        );

        let all: Vec<u64> = runtime
            .streams
            .query::<Pulse>(..)
            .as_of(second.position)
            .await
            .expect("nothing was evicted")
            .map(|At { value, .. }| value.sample)
            .collect();
        assert_eq!(all, vec![1, 2, 3]);

        crate::support::app::shutdown(&runtime).await.unwrap();
    }
}

#[tokio::test]
async fn demand_tracks_consumers() {
    {
        let runtime = streaming().await;
        let demand = runtime.streams.demand::<Pulse>();

        assert_eq!(*demand.borrow(), 0, "an unobserved stream has zero demand");

        let first = runtime.streams.watch::<Pulse>();
        let second = runtime.streams.watch::<Pulse>();
        let other = runtime.streams.watch::<Reading>();

        assert_eq!(*demand.borrow(), 2);
        assert_eq!(*runtime.streams.demand::<Reading>().borrow(), 1, "demand is per stream: the other feed's watcher is not this one's");

        drop(first);
        assert_eq!(*demand.borrow(), 1);
        drop(second);
        drop(other);
        assert_eq!(*demand.borrow(), 0, "demand falls back to zero on drop");

        crate::support::app::shutdown(&runtime).await.unwrap();
    }
    {
        let runtime = crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
            setup.settings().register(support::catalog());
            setup.settings().register(streams());
            setup.settings().attach_streams(Selecting);
        })
        .await
        .expect("the catalog and attachment are honorable");
        let pulse_demand = runtime.streams.demand::<Pulse>();
        let reading_demand = runtime.streams.demand::<Reading>();

        assert_eq!(*pulse_demand.borrow(), 1, "the attached consumer counts for the stream it selected");
        assert_eq!(*reading_demand.borrow(), 0, "selection is per definition, not per facility");

        crate::support::app::shutdown(&runtime).await.unwrap();

        assert_eq!(*pulse_demand.borrow(), 0, "the consumer leaves demand when its supervisor joins");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn demand_drives_capture() {
    let runtime = streaming().await;
    let capture = Arc::new(AtomicBool::new(false));

    // The capture loop from the spec, verbatim in shape: an application task
    // that turns its source on while anyone is looking and off when nobody is.
    // The acknowledgement channel stands in for the hardware call, so the test
    // observes transitions instead of sleeping.
    let (acknowledged, mut acknowledgements) = tokio::sync::watch::channel(false);
    let mut demand = runtime.streams.demand::<Pulse>();
    let looped = {
        let capture = Arc::clone(&capture);
        tokio::spawn(async move {
            while demand.changed().await.is_ok() {
                if *demand.borrow() > 0 {
                    capture.store(true, Ordering::Release);
                    acknowledged.send_replace(true);
                } else {
                    capture.store(false, Ordering::Release);
                    acknowledged.send_replace(false);
                }
            }
        })
    };

    let watching = runtime.streams.watch::<Pulse>();
    while !*acknowledgements.borrow_and_update() {
        acknowledgements.changed().await.expect("the loop is alive");
    }
    assert!(capture.load(Ordering::Acquire), "a watcher appearing turns capture on");

    drop(watching);
    while *acknowledgements.borrow_and_update() {
        acknowledgements.changed().await.expect("the loop is alive");
    }
    assert!(!capture.load(Ordering::Acquire), "the last watcher leaving turns capture off");

    // The loop ends when the facility is gone entirely — no lifecycle control
    // was ever exercised by the substrate itself. Stopping ends delivery; it is
    // dropping the last `Streams` handle that closes the demand channel, and
    // the runtime holds one for as long as it is alive.
    runtime.shutdown().await.unwrap();
    drop(runtime);
    looped.await.expect("the capture loop ends without panic");
}

#[tokio::test]
async fn definitions_refuse_invalid() {
    {
        let refused = crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
            setup.settings().register(support::catalog());
            setup.settings().register(streams());
            setup.settings().register(colliding::stream_definitions());
        })
        .await;

        let Err(refused) = refused else {
            panic!("two definitions share an id; the boot cannot honor both");
        };
        assert!(refused.to_string().contains("pulse"), "the refusal names the colliding id: {refused}");
    }
    {
        #[derive(harmos_runtime::Described, Clone, Copy, Debug, Serialize, Deserialize)]
        struct Hollow;

        let refused = crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
            setup.settings().register(support::catalog());
            setup.settings().register([Definition::native::<Hollow>("hollow", 0)]);
        })
        .await;

        let Err(refused) = refused else {
            panic!("a window of zero rows retains nothing and cannot be honored");
        };
        assert!(refused.to_string().contains("hollow"), "the refusal names the stream it cannot honor: {refused}");
    }
}

#[allow(dead_code)]
fn registration_takes_definitions(catalog: [Definition; 2], streams: &StreamManager<Bank>) {
    let _ = catalog;
    let _ = streams;
}

mod colliding {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports
    use serde::{Deserialize, Serialize};

    // Local Imports

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    #[derive(harmos_runtime::Row, Clone, Copy, Debug, Serialize, Deserialize)]
    pub struct Imposter;

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl harmos_runtime::streams::StreamRow for Imposter {
        const ENTRY: harmos_runtime::StreamEntry<Self> = harmos_runtime::StreamEntry::new("pulse", 1).label("Imposter");
    }

    /* === Public Functions === */

    pub fn stream_definitions() -> Vec<harmos_runtime::streams::Definition> {
        vec![<Imposter as harmos_runtime::streams::StreamRow>::ENTRY.build().unwrap()]
    }

    /* === Private Functions === */

    /* === Tests === */
}
