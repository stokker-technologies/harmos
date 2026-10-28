/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;

use tokio::sync::oneshot;

use super::observer::spawn;

struct Dropped(Arc<AtomicBool>);

impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn blocking_preserves_runtimes() {
    let (release, blocked) = mpsc::channel::<()>();
    let (entered, started) = oneshot::channel();
    let blocking = spawn("blocking-observer".into(), async move {
        let _ = entered.send(thread::current().id());
        // Block The Observer Thread Until The Test Releases It.
        let _ = blocked.recv();
    })
    .unwrap();
    let blocking_thread = started.await.unwrap();

    // A Shared Runtime Would Starve This Observer Behind The Blocked One.
    let (measured, measurement) = oneshot::channel();
    let healthy = spawn("healthy-observer".into(), async move {
        let _ = measured.send(thread::current().id());
    })
    .unwrap();
    let healthy_thread = measurement.await.unwrap();
    assert_ne!(healthy_thread, blocking_thread);
    assert_ne!(healthy_thread, thread::current().id());

    release.send(()).unwrap();
    blocking.await.unwrap();
    healthy.await.unwrap();
}

#[tokio::test]
async fn abort_drops_observer() {
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
}

#[tokio::test]
async fn panic_preserves_sibling() {
    let failed = spawn("panicked-observer".into(), async { panic!("observer failed") }).unwrap();
    let healthy = spawn("healthy-observer".into(), async {}).unwrap();
    assert!(failed.await.unwrap_err().is_panic());
    healthy.await.unwrap();
}
