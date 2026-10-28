/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod application;
mod partial;
mod shutdown;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::Ordering;

// Third Party Imports
use tokio::sync::{Notify, Semaphore};

// Local Imports
use crate::runtime::boot::catalog;
use crate::runtime::mirror::Mirror;
use crate::runtime::probe::{Change, Editor, Sheet, rename};
use harmos_runtime::state::{Entry, Position, Recorded};
use harmos_runtime::storage::JournalConsumer;
use harmos_runtime::{Error, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// A flush held open until the story releases it, announcing when it is entered.
#[derive(Clone)]
struct Held {
    entered: Arc<Notify>,
    release: Arc<Semaphore>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Default for Held {
    fn default() -> Self {
        Self {
            entered: Arc::default(),
            release: Arc::new(Semaphore::new(0)),
        }
    }
}

impl Held {
    // Close the gate for good, so every later flush passes straight through.
    fn open(&self) {
        self.release.close();
    }
}

impl JournalConsumer<Editor> for Held {
    fn resume(&self) -> Position {
        Position::ORIGIN
    }

    async fn drain(&mut self, _entries: Vec<Arc<Entry<Recorded<Editor>>>>, applied: Position) -> Position {
        applied
    }

    async fn flush(&mut self) {
        self.entered.notify_one();
        let _ = self.release.acquire().await;
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn shutdown_drains_attached() {
    let (mirror, (checkpoint, _)) = Mirror::new();
    let runtime = Runtime::<Editor>::launch_with(Sheet::default(), (), |setup| {
        setup.settings().register(catalog());
        setup.settings().attach(mirror);
    })
    .await
    .expect("nothing was recovered");

    for title in ["one", "two", "three"] {
        runtime.state.commit(rename(title)).await.expect("accepted");
    }
    let applied = runtime.state.applied();
    let settled = runtime.shutdown().await.unwrap();

    assert_eq!(applied, Position(3));
    assert_eq!(settled, applied, "shutdown drains to the applied frontier");
    assert_eq!(checkpoint.load(Ordering::Acquire), applied.0);
}

#[tokio::test]
async fn poisoned_writer_evacuates() {
    let (mirror, (checkpoint, _)) = Mirror::new();
    let runtime = Runtime::<Editor>::launch_with(Sheet::default(), (), |setup| {
        setup.settings().register(catalog());
        setup.settings().attach(mirror);
    })
    .await
    .expect("nothing was recovered");

    runtime.state.commit(rename("kept")).await.expect("accepted");

    let silence = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let poisoned = runtime.state.commit(Change::Explode).await;
    std::panic::set_hook(silence);

    assert!(matches!(poisoned, Err(Error::Unavailable)));
    assert_eq!(runtime.state.applied(), Position(1), "no entry was sealed for the change that panicked");
    assert!(matches!(runtime.state.state().read(|sheet| sheet.title.clone()).await, Err(Error::Unavailable)));

    // The runtime evacuates without being asked: the writer is gone, so the
    // supervisor folds the intact tail out and stops rather than restarting.
    let settled = runtime.shutdown().await.unwrap();

    assert_eq!(settled, Position(1));
    assert_eq!(checkpoint.load(Ordering::Acquire), 1, "the entry committed before the panic reached storage");
}

#[tokio::test]
async fn repeat_stop_frontier() {
    let runtime = Runtime::<Editor>::launch_with(Sheet::default(), (), |setup| {
        setup.settings().register(catalog());
    })
    .await
    .expect("nothing was recovered");

    runtime.state.commit(rename("once")).await.expect("accepted");
    let settled = runtime.shutdown().await.unwrap();

    assert_eq!(settled, Position(1));
    assert_eq!(runtime.shutdown().await.unwrap(), settled, "a repeat stop joins nothing and reports the frontier the first saw");
}

#[tokio::test]
async fn handles_allow_shutdown() {
    let runtime = Runtime::<Editor>::launch_with(Sheet::default(), (), |setup| {
        setup.settings().register(catalog());
    })
    .await
    .expect("nothing was recovered");
    // What a host-side plugin keeps for its whole life, and cannot drop
    // before asking the runtime it holds to stop.
    let retained = runtime.handle();

    // The stop does not wait on a live handle.
    let settled = runtime.shutdown().await.unwrap();

    assert_eq!(settled, Position::ORIGIN);
    assert!(
        matches!(retained.state.commit(rename("after")).await, Err(Error::Unavailable)),
        "a sealed journal refuses a late write rather than leaving it hanging"
    );
}

#[tokio::test(start_paused = true)]
async fn slow_flush_notices() {
    let held = Held::default();
    let runtime = Runtime::<Editor>::launch_with(Sheet::default(), (), |setup| {
        setup.settings().register(catalog());
        setup.settings().attach(held.clone());
    })
    .await
    .expect("nothing was recovered");
    let mut notices = runtime.notices();

    // The held flush outlasts the grace window, so the paused clock reaches it.
    let mut stopping = pin!(runtime.shutdown());
    tokio::select! {
        _ = &mut stopping => panic!("the held flush keeps the stop joining"),
        changed = notices.changed() => changed.expect("the runtime publishes notices"),
    }
    let notice = notices.borrow_and_update().clone().expect("a join past the grace window publishes its diagnostic");
    assert!(notice.contains("StateManager clone"), "{notice}");

    held.open();
    assert_eq!(stopping.await.unwrap(), Position::ORIGIN, "the grace window is a diagnostic, never a deadline the drain is cut at");
}

#[tokio::test]
async fn abandoned_stop_resumes() {
    let held = Held::default();
    let runtime = Runtime::<Editor>::launch_with(Sheet::default(), (), |setup| {
        setup.settings().register(catalog());
        setup.settings().attach(held.clone());
    })
    .await
    .expect("nothing was recovered");

    runtime.state.commit(rename("once")).await.expect("accepted");
    // Abandoned mid-join, which is what a host-side timeout around a stop
    // does: the frontier must not go out with the dropped future.
    {
        let mut stopping = pin!(runtime.shutdown());
        tokio::select! {
            _ = &mut stopping => panic!("the held flush keeps the stop joining"),
            () = held.entered.notified() => {}
        }
    }

    held.open();
    assert_eq!(runtime.shutdown().await.unwrap(), Position(1), "the next stop resumes the join rather than panicking on a spent handle");
}
