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

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::{sync::mpsc, thread};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
// Third Party Imports
use tokio::sync::oneshot;
// Local Imports
use super::observer::spawn;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Dropped(Arc<AtomicBool>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("blocking_preserves_runtimes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (release, blocked) = mpsc::channel::<()>();
                    let (entered, started) = oneshot::channel();
                    let blocking = spawn("blocking-observer".into(), async move {
                        let _ = entered.send(thread::current().id());
                        // A bounded fallback makes a regression fail instead of hanging CI.
                        let _ = blocked.recv_timeout(Duration::from_secs(3));
                    })
                    .unwrap();
                    let blocking_thread = started.await.unwrap();
                    let before = Instant::now();
                    let (measured, measurement) = oneshot::channel();
                    let healthy = spawn("healthy-observer".into(), async move {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        let _ = measured.send(thread::current().id());
                    })
                    .unwrap();
                    let healthy_thread = tokio::time::timeout(Duration::from_secs(1), measurement)
                        .await
                        .unwrap()
                        .unwrap();
                    assert_ne!(healthy_thread, blocking_thread);
                    assert_ne!(healthy_thread, thread::current().id());
                    assert!(before.elapsed() < Duration::from_secs(1));
                    release.send(()).unwrap();
                    blocking.await.unwrap();
                    healthy.await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("abort_drops_observer", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let dropped = Arc::new(AtomicBool::new(false));
                    let guard = Dropped(Arc::clone(&dropped));
                    let (entered, started) = oneshot::channel();
                    let task = spawn("pending-observer".into(), async move {
                        let _guard = guard;
                        let _ = entered.send(());
                        std::future::pending::<()>().await;
                    })
                    .unwrap();
                    started.await.unwrap();
                    task.abort();
                    assert!(task.await.unwrap_err().is_cancelled());
                    assert!(dropped.load(Ordering::SeqCst));
                });
        });
    }
    {
        scenario_runtime::check("panic_preserves_sibling", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let failed = spawn("panicked-observer".into(), async {
                        panic!("observer failed")
                    })
                    .unwrap();
                    let healthy = spawn("healthy-observer".into(), async {}).unwrap();
                    assert!(failed.await.unwrap_err().is_panic());
                    healthy.await.unwrap();
                });
        });
    }
}

/* === Tests === */
