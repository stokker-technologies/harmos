/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Local Imports
use super::*;

/* === Tests === */

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn watch_delivery_accounts_for_rows_overruns_and_shutdown() {
    // Start One Host With an Active and a Stalled Watcher.
    let runtime = streaming().await;
    let mut live = runtime.streams.watch::<Pulse>();
    let mut stalled = runtime.streams.watch::<Pulse>();
    let held: Vec<At<Pulse>> = runtime.streams.query::<Pulse>(..).await.unwrap().collect();
    let streams = runtime.streams.clone();
    let window = Pulse::WINDOW as u64;
    let published = window * 3;

    // Publish Across Ring Wraps From an Ordinary Thread.
    let publisher = std::thread::spawn(move || {
        for sample in 1..=published {
            streams.publish(pulse(sample));
        }
    });
    let mut delivered = 0;
    let mut missed = 0;
    let mut last = 0;
    while delivered + missed < published {
        match next(&mut live).await.expect("the watch is live") {
            Item::Row(At { value, .. }) => {
                assert!(value.sample > last);
                last = value.sample;
                delivered += 1;
            }
            Item::Gap { missed: count } => missed += count,
        }
    }
    publisher.join().unwrap();
    assert_eq!(delivered + missed, published);
    assert_eq!(last, published);
    assert!(held.is_empty(), "a captured snapshot never changes");

    // A Stalled Watcher Reports the Exact Gap, Then the Retained Window.
    assert_eq!(next(&mut stalled).await, Some(Item::Gap { missed: published - window }));
    for sample in (published - window + 1)..=published {
        assert!(matches!(next(&mut stalled).await, Some(Item::Row(At { value, .. })) if value == pulse(sample)));
    }

    // Shutdown Drains the Pending Rows Before Closing Both Watches.
    runtime.streams.publish(pulse(published + 1));
    runtime.streams.publish(pulse(published + 2));
    crate::support::app::shutdown(&runtime).await.unwrap();
    for watching in [&mut live, &mut stalled] {
        for sample in (published + 1)..=(published + 2) {
            assert!(matches!(next(watching).await, Some(Item::Row(At { value, .. })) if value == pulse(sample)));
        }
        assert_eq!(next(watching).await, None);
    }
}
