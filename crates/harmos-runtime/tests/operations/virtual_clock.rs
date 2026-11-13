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
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

// Third Party Imports

// Local Imports
use harmos_runtime::clock::Clock;

/* === Tests === */

#[tokio::test]
async fn host_work_wait_and_interval_interleave_under_one_controller() {
    // Compose One Host Clock and Register Work Before Scheduling It
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let mut operation = clock.activity("operation call");
    let mut observer = clock.activity("observer stream");
    let events = Arc::new(Mutex::new(Vec::new()));
    let completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut interval = clock.interval(Duration::from_secs(2));

    // Answer Ticks While an Operation Waits and Performs Host Work
    let work = async {
        tokio::join!(
            async {
                operation.wait(clock.sleep(Duration::from_secs(3))).await;
                events.lock().unwrap().push(("operation", clock.date()));
                tokio::task::yield_now().await;
                operation.wait(clock.sleep(Duration::from_secs(2))).await;
                events.lock().unwrap().push(("completed", clock.date()));
                completed.store(true, std::sync::atomic::Ordering::SeqCst);
                drop(operation);
            },
            async {
                for _ in 0..2 {
                    let tick = observer.wait(interval.tick()).await;
                    events.lock().unwrap().push(("tick", tick.date));
                    // The Tick Remains Unanswered During Host Work
                    tokio::task::yield_now().await;
                    assert_eq!(clock.date(), tick.date);
                }
                drop(observer);
            }
        );
    };

    // Stop at Completion Rather than Following Periodic Timers Forever
    let (_, result) =
        tokio::join!(work, controller.advance_until(Duration::from_secs(20), || completed.load(std::sync::atomic::Ordering::SeqCst)));
    result.unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        [
            ("tick", SystemTime::UNIX_EPOCH + Duration::from_secs(2)),
            ("operation", SystemTime::UNIX_EPOCH + Duration::from_secs(3)),
            ("tick", SystemTime::UNIX_EPOCH + Duration::from_secs(4)),
            ("completed", SystemTime::UNIX_EPOCH + Duration::from_secs(5)),
        ]
    );
    assert_eq!(controller.read().date, SystemTime::UNIX_EPOCH + Duration::from_secs(5));
}
