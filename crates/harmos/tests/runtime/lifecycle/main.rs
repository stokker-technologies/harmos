/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

#[path = "../../support/runtime_mirror.rs"]
mod mirror;
#[path = "../../support/runtime_probe.rs"]
mod probe;
mod shutdown;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

const STOP_GRACE: Duration = Duration::from_millis(100);

use std::sync::atomic::Ordering;

use harmos::state::{Focus, Link, PrincipalId, Timestamp};
use harmos::{
    DefinitionId, Error, state::CorrelationId, state::Entry, state::Envelope, state::Kind,
    state::Recorded, storage::JournalConsumer,
};
use harmos::{Runtime, state::DecodeError, state::Position};
use harmos::{runtime::Catalog, runtime::Registration};
use harmos::{runtime::Incompatible, storage::Payload};
use harmos::{storage::Recovery, storage::Source};
use mirror::Mirror;
use probe::{Change, EXPORTED, Editor, Note, RENAME, Sheet, inverses, named, rename};
use std::sync::Arc;
use std::time::Duration;

fn alice() -> PrincipalId {
    PrincipalId::new("alice")
}

fn origin() -> Sheet {
    Sheet {
        title: "origin".to_owned(),
        sealed: false,
    }
}

fn catalog() -> impl Registration<Editor> {
    Definitions([named(RENAME), named(EXPORTED)].into())
}

#[derive(Clone)]
struct Definitions(Vec<(Kind, DefinitionId, u32)>);

impl Registration<Editor> for Definitions {
    fn register(self) -> Catalog<Editor> {
        fn decode(
            _id: &DefinitionId,
            _version: u32,
            _bytes: &[u8],
        ) -> Result<Option<Change>, DecodeError> {
            Ok(None)
        }

        Catalog::Transactions {
            definitions: self.0,
            decode,
            inverses: Some(inverses),
        }
    }
}

fn stored(
    position: u64,
    id: &str,
    link: Option<Link>,
    value: Recorded<Editor>,
) -> Envelope<Payload<Editor>> {
    Envelope {
        position: Position(position),
        kind: value.kind(),
        id: DefinitionId::new(id),
        version: 1,
        metadata: serde_json::from_value(serde_json::json!({
            "commit": null,
            "principal": alice(),
            "correlation": CorrelationId(Position(position)),
            "causation": null,
            "request": null,
            "link": link,
            "at": Timestamp::UNIX_EPOCH,
        }))
        .unwrap(),
        payload: Payload {
            prepared: None,
            id: DefinitionId::new(id),
            version: 1,
            value,
            apply: None,
        },
    }
}

fn renamed(position: u64, title: &str, link: Option<Link>) -> Envelope<Payload<Editor>> {
    stored(
        position,
        RENAME.1,
        link,
        Recorded::Transaction(Arc::new(rename(title))),
    )
}

struct Replaying {
    recorded: Vec<Envelope<Payload<Editor>>>,
    resume: Position,
}

impl JournalConsumer<Editor> for Replaying {
    fn resume(&self) -> Position {
        self.resume
    }

    async fn drain(
        &mut self,
        _entries: Vec<Arc<Entry<Recorded<Editor>>>>,
        applied: Position,
    ) -> Position {
        self.resume = applied;

        applied
    }
}

impl Source<Editor> for Replaying {
    async fn load(&mut self) -> Result<Recovery<Editor>, Incompatible> {
        let entries = std::mem::take(&mut self.recorded);
        self.resume = entries
            .last()
            .map_or(Position::ORIGIN, |entry| entry.position);

        Ok(Recovery {
            commits: Vec::new(),
            frontier: self.resume,
            entries,
            encoded: Vec::new(),
        })
    }
}

fn recovering(recorded: Vec<Envelope<Payload<Editor>>>) -> Replaying {
    Replaying {
        recorded,
        resume: Position::ORIGIN,
    }
}

async fn title(runtime: &Runtime<Editor>) -> String {
    runtime
        .state
        .state()
        .read(|sheet| sheet.title.clone())
        .await
        .expect("reads are open")
        .value
}

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("boot_preserves_origin", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = Runtime::<Editor>::builder(Sheet::default(), ())
                        .register(catalog())
                        .runtime()
                        .await
                        .expect("nothing was recovered");

                    assert_eq!(title(&runtime).await, "");
                    assert_eq!(runtime.state.applied(), Position::ORIGIN);
                    assert_eq!(
                        runtime.state.stored(),
                        runtime.state.applied(),
                        "with nothing attached, stored tracks applied"
                    );
                    assert_eq!(runtime.stop().await.unwrap(), Position::ORIGIN);
                });
        });
    }
    {
        scenario_runtime::check("read_stamped_position", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = Runtime::<Editor>::builder(Sheet::default(), ())
                        .register(catalog())
                        .runtime()
                        .await
                        .expect("nothing was recovered");

                    let receipt = runtime
                        .state
                        .commit(rename("draft"))
                        .await
                        .expect("accepted");
                    let read = runtime
                        .state
                        .state()
                        .read(|sheet| sheet.title.clone())
                        .await
                        .expect("reads are open");

                    assert_eq!(read.value, "draft");
                    assert_eq!(read.position, receipt.position);
                    assert!(read.position >= receipt.position);
                });
        });
    }
    {
        scenario_runtime::check("boot_folds_state", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    // The live path: two renames and one undo of the second.
                    let live = Runtime::<Editor>::builder(origin(), ())
                        .register(catalog())
                        .runtime()
                        .await
                        .expect("nothing was recovered");
                    let handle = live.state.as_principal(alice());
                    handle.commit(rename("first")).await.expect("accepted");
                    let second = handle.commit(rename("second")).await.expect("accepted");
                    handle
                        .undo(alice(), Focus::all())
                        .await
                        .expect("alice has an entry to undo");

                    // The recovered path: the very same entries, replayed.
                    let recovered = Runtime::<Editor>::builder(origin(), ())
                        .register(catalog())
                        .recover(recovering(vec![
                            renamed(1, "first", None),
                            renamed(2, "second", None),
                            renamed(3, "first", Some(Link::UndoOf(second.position))),
                            stored(4, EXPORTED.1, None, Recorded::Record(Note::Exported)),
                        ]))
                        .runtime()
                        .await
                        .expect("every definition is declared");

                    assert_eq!(title(&live).await, title(&recovered).await);
                    assert_eq!(recovered.state.applied(), Position(4));

                    // The rebuilt fold has to answer the next undo exactly as the live one
                    // does — which it can only do if replay re-derived the inverse of entry
                    // one against the state as it stood before entry one.
                    live.state
                        .undo(alice(), Focus::all())
                        .await
                        .expect("the first rename is still active");
                    recovered
                        .state
                        .undo(alice(), Focus::all())
                        .await
                        .expect("the rebuilt stack knows the same");

                    assert_eq!(title(&live).await, "origin");
                    assert_eq!(
                        title(&recovered).await,
                        "origin",
                        "the inverse was re-derived against the state before its own entry"
                    );
                });
        });
    }
    {
        scenario_runtime::check("boot_refuses_loudly", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let booted = Runtime::<Editor>::builder(Sheet::default(), ())
                        .register(catalog())
                        .recover(recovering(vec![stored(
                            1,
                            "sheet.rotate",
                            None,
                            Recorded::Transaction(Arc::new(Change::Seal)),
                        )]))
                        .runtime()
                        .await;

                    let Err(refused) = booted else {
                        panic!("no definition is declared under that name");
                    };
                    assert!(
                        refused.to_string().contains("sheet.rotate"),
                        "the refusal names what it could not honor: {refused}"
                    );
                });
        });
    }
    {
        scenario_runtime::check("shutdown_drains_attached", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (mirror, (checkpoint, _)) = Mirror::new();
                    let runtime = Runtime::<Editor>::builder(Sheet::default(), ())
                        .register(catalog())
                        .attach(mirror)
                        .runtime()
                        .await
                        .expect("nothing was recovered");

                    for title in ["one", "two", "three"] {
                        runtime.state.commit(rename(title)).await.expect("accepted");
                    }
                    let applied = runtime.state.applied();
                    let settled = runtime.stop().await.unwrap();

                    assert_eq!(applied, Position(3));
                    assert_eq!(settled, applied, "shutdown drains to the applied frontier");
                    assert_eq!(checkpoint.load(Ordering::Acquire), applied.0);
                });
        });
    }
    {
        scenario_runtime::check("poisoned_writer_evacuates", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (mirror, (checkpoint, _)) = Mirror::new();
                    let runtime = Runtime::<Editor>::builder(Sheet::default(), ())
                        .register(catalog())
                        .attach(mirror)
                        .runtime()
                        .await
                        .expect("nothing was recovered");

                    runtime
                        .state
                        .commit(rename("kept"))
                        .await
                        .expect("accepted");

                    let silence = std::panic::take_hook();
                    std::panic::set_hook(Box::new(|_| {}));
                    let poisoned = runtime.state.commit(Change::Explode).await;
                    std::panic::set_hook(silence);

                    assert!(matches!(poisoned, Err(Error::Unavailable)));
                    assert_eq!(
                        runtime.state.applied(),
                        Position(1),
                        "no entry was sealed for the change that panicked"
                    );
                    assert!(matches!(
                        runtime
                            .state
                            .state()
                            .read(|sheet| sheet.title.clone())
                            .await,
                        Err(Error::Unavailable)
                    ));

                    // The runtime evacuates without being asked: the writer is gone, so the
                    // supervisor folds the intact tail out and stops rather than restarting.
                    let settled = runtime.stop().await.unwrap();

                    assert_eq!(settled, Position(1));
                    assert_eq!(
                        checkpoint.load(Ordering::Acquire),
                        1,
                        "the entry committed before the panic reached storage"
                    );
                });
        });
    }
    {
        scenario_runtime::check("repeat_stop_frontier", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = Runtime::<Editor>::builder(Sheet::default(), ())
                        .register(catalog())
                        .runtime()
                        .await
                        .expect("nothing was recovered");

                    runtime
                        .state
                        .commit(rename("once"))
                        .await
                        .expect("accepted");
                    let settled = runtime.stop().await.unwrap();

                    assert_eq!(settled, Position(1));
                    assert_eq!(
                        runtime.stop().await.unwrap(),
                        settled,
                        "a repeat stop joins nothing and reports the frontier the first saw"
                    );
                });
        });
    }
    {
        scenario_runtime::check("handles_allow_shutdown", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = Runtime::<Editor>::builder(Sheet::default(), ())
                        .register(catalog())
                        .runtime()
                        .await
                        .expect("nothing was recovered");
                    // What a host-side plugin keeps for its whole life, and cannot drop
                    // before asking the runtime it holds to stop.
                    let retained = runtime.handle();

                    let settled = tokio::time::timeout(Duration::from_secs(5), runtime.stop())
                        .await
                        .expect("the stop does not wait on a live handle")
                        .unwrap();

                    assert_eq!(settled, Position::ORIGIN);
                    assert!(
                        matches!(
                            retained.state.commit(rename("after")).await,
                            Err(Error::Unavailable)
                        ),
                        "a sealed journal refuses a late write rather than leaving it hanging"
                    );
                });
        });
    }
    {
        scenario_runtime::check("slow_stop_says", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = Runtime::<Editor>::builder(Sheet::default(), ())
                        .register(catalog())
                        .attach(Slow(|| tokio::time::sleep(STOP_GRACE * 3)))
                        .runtime()
                        .await
                        .expect("nothing was recovered");
                    let mut notices = runtime.notices();

                    let settled = runtime.stop().await.unwrap();

                    assert_eq!(
                        settled,
                        Position::ORIGIN,
                        "the grace window is a diagnostic, never a deadline the drain is cut at"
                    );
                    let notice = notices
                        .borrow_and_update()
                        .clone()
                        .expect("a join past the grace window publishes its diagnostic");
                    assert!(notice.contains("StateManager clone"), "{notice}");
                });
        });
    }
    {
        scenario_runtime::check("abandoned_stop_leaves", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = Runtime::<Editor>::builder(Sheet::default(), ())
                        .register(catalog())
                        .attach(Slow(|| tokio::time::sleep(STOP_GRACE * 3)))
                        .runtime()
                        .await
                        .expect("nothing was recovered");

                    runtime
                        .state
                        .commit(rename("once"))
                        .await
                        .expect("accepted");
                    // Abandoned mid-join, which is what a host-side timeout around a stop
                    // does: the frontier must not go out with the dropped future.
                    assert!(
                        tokio::time::timeout(STOP_GRACE, runtime.stop())
                            .await
                            .is_err(),
                        "the slow fold's flush outlasts the grace window"
                    );

                    assert_eq!(
                        runtime.stop().await.unwrap(),
                        Position(1),
                        "the next stop resumes the join rather than panicking on a spent handle"
                    );
                });
        });
    }
}

struct Slow<F>(F);

impl<F, Fut> JournalConsumer<Editor> for Slow<F>
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    fn resume(&self) -> Position {
        Position::ORIGIN
    }

    async fn drain(
        &mut self,
        _entries: Vec<Arc<Entry<Recorded<Editor>>>>,
        applied: Position,
    ) -> Position {
        applied
    }

    async fn flush(&mut self) {
        (self.0)().await;
    }
}
