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
use std::sync::{Arc, Mutex};
use std::time::Duration;
// Third Party Imports
use tokio::sync::{Notify, mpsc};
// Local Imports
use harmos::plugin::sidecar::{ActivationId, Instance, ObserverContext};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type ObservedIdentity = (&'static str, ActivationId, u64);

/* === Structs === */
#[derive(Clone)]
struct Resources {
    events: mpsc::UnboundedSender<Event>,
    identities: Option<mpsc::UnboundedSender<ObservedIdentity>>,
    stop_gate: Option<Arc<Notify>>,
}

struct Capabilities {
    token: u64,
    behavior: Behavior,
}

#[derive(Clone, Default)]
struct Diagnostics(Arc<Mutex<Vec<u8>>>);

struct NeverStarts(Resources);

struct Watch(Resources);

struct Callback(Resources);

/* === Enums === */
#[derive(Clone, Copy)]
enum Behavior {
    FailFirst,
    Panic,
    Hang,
}

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Opened,
    Tick(u64, u64),
    Dropped,
    Stopping,
    Stopped,
}

/* === Traits === */

/* === Implementations === */
#[harmos::observer(id = "execution.watch", interval_ms = 5)]
impl Watch {
    fn new(resources: &Resources) -> Result<Self, std::io::Error> {
        Ok(Self(resources.clone()))
    }

    async fn initialize(&mut self) -> Result<(), std::io::Error> {
        self.0.events.send(Event::Opened).unwrap();
        Ok(())
    }

    async fn observe(
        &mut self,
        ctx: &ObserverContext<'_, Resources, Capabilities>,
    ) -> Result<(), std::io::Error> {
        let _callback = Callback(self.0.clone());
        if let Some(identities) = &self.0.identities {
            identities
                .send((ctx.observer_id(), ctx.activation_id(), ctx.tick().ordinal()))
                .unwrap();
        }
        assert!(ctx.tick().elapsed() >= Duration::from_millis(5));
        assert_eq!(ctx.extension::<u64>(), Some(&ctx.capabilities().token));
        ctx.resources()
            .events
            .send(Event::Tick(ctx.tick().ordinal(), ctx.capabilities().token))
            .unwrap();

        match ctx.capabilities().behavior {
            Behavior::FailFirst if ctx.tick().ordinal() == 1 => {
                Err(std::io::Error::other("tick refused"))
            }
            Behavior::Panic => panic!("callback panic is isolated"),
            Behavior::Hang => std::future::pending().await,
            Behavior::FailFirst => Ok(()),
        }
    }

    async fn stop(&mut self) -> Result<(), std::io::Error> {
        self.0.events.send(Event::Stopping).unwrap();
        if let Some(gate) = &self.0.stop_gate {
            gate.notified().await;
        }
        self.0.events.send(Event::Stopped).unwrap();

        Ok(())
    }
}

impl Drop for Callback {
    fn drop(&mut self) {
        let _ = self.0.events.send(Event::Dropped);
    }
}

impl std::io::Write for Diagnostics {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[harmos::observer(interval_ms = 5)]
impl NeverStarts {
    fn new(resources: &Resources) -> Result<Self, std::io::Error> {
        Ok(Self(resources.clone()))
    }

    async fn initialize(&mut self) -> Result<(), std::io::Error> {
        let _initializing = Callback(self.0.clone());
        self.0.events.send(Event::Opened).unwrap();
        std::future::pending().await
    }

    async fn observe(
        &mut self,
        _ctx: &ObserverContext<'_, Resources, ()>,
    ) -> Result<(), std::io::Error> {
        panic!("an uninitialized observer cannot run");
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn start(
    behavior: Behavior,
    stop_gate: Option<Arc<Notify>>,
) -> (
    harmos::plugin::sidecar::ObserverTask,
    mpsc::UnboundedReceiver<Event>,
) {
    let (events, received) = mpsc::unbounded_channel();
    let resources = Arc::new(Resources {
        events,
        stop_gate,
        identities: None,
    });
    let instance = Instance::<Resources, Capabilities>::of::<Watch>()
        .with_context(|capabilities| capabilities.token);
    let task = instance
        .start(
            resources,
            Capabilities {
                token: 73,
                behavior,
            },
        )
        .unwrap();
    (task, received)
}

async fn next(events: &mut mpsc::UnboundedReceiver<Event>) -> Event {
    tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .expect("observer makes bounded progress")
        .expect("observer reports its lifecycle")
}

async fn entered(events: &mut mpsc::UnboundedReceiver<Event>) {
    assert_eq!(next(events).await, Event::Opened);
    assert_eq!(next(events).await, Event::Tick(1, 73));
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("shared_execution_clock", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (mut task, mut events) = start(Behavior::FailFirst, None);
                    entered(&mut events).await;
                    assert_eq!(next(&mut events).await, Event::Dropped);
                    assert_eq!(next(&mut events).await, Event::Tick(2, 73));
                    assert!(!task.is_finished());

                    task.stop().await.unwrap();
                    assert!(task.is_finished());
                    task.join().await.unwrap();
                    task.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("callback_panic_isolated", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (mut failed, mut failure) = start(Behavior::Panic, None);
                    let (mut sibling, mut events) = start(Behavior::FailFirst, None);
                    entered(&mut failure).await;
                    assert_eq!(next(&mut failure).await, Event::Dropped);
                    assert_eq!(next(&mut failure).await, Event::Stopping);
                    assert_eq!(next(&mut failure).await, Event::Stopped);
                    tokio::time::timeout(Duration::from_secs(2), failed.join())
                        .await
                        .unwrap()
                        .unwrap();
                    assert!(failed.is_finished());
                    assert_eq!(failure.recv().await, None);

                    entered(&mut events).await;
                    assert_eq!(next(&mut events).await, Event::Dropped);
                    assert_eq!(next(&mut events).await, Event::Tick(2, 73));
                    sibling.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancelled_stop_rejoins", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let gate = Arc::new(Notify::new());
                    let (mut task, mut events) = start(Behavior::Hang, Some(Arc::clone(&gate)));
                    entered(&mut events).await;
                    assert!(
                        tokio::time::timeout(Duration::from_millis(20), task.stop())
                            .await
                            .is_err()
                    );
                    assert_eq!(next(&mut events).await, Event::Dropped);
                    assert_eq!(next(&mut events).await, Event::Stopping);
                    assert!(!task.is_finished());

                    gate.notify_one();
                    task.stop().await.unwrap();
                    assert_eq!(next(&mut events).await, Event::Stopped);
                    assert!(task.is_finished());
                    assert_eq!(events.recv().await, None);
                });
        });
    }
    {
        scenario_runtime::check("drop_cleans_callback", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (task, mut events) = start(Behavior::Hang, None);
                    entered(&mut events).await;
                    drop(task);
                    assert_eq!(next(&mut events).await, Event::Dropped);
                    assert_eq!(next(&mut events).await, Event::Stopping);
                    assert_eq!(next(&mut events).await, Event::Stopped);
                    assert_eq!(events.recv().await, None);
                });
        });
    }
    {
        scenario_runtime::check("cancellation_drops_init", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (events, mut received) = mpsc::unbounded_channel();
                    let resources = Arc::new(Resources {
                        events,
                        identities: None,
                        stop_gate: None,
                    });
                    let mut task = Instance::<Resources, ()>::of::<NeverStarts>()
                        .start(resources, ())
                        .unwrap();
                    assert_eq!(next(&mut received).await, Event::Opened);
                    tokio::time::timeout(Duration::from_secs(2), task.stop())
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(next(&mut received).await, Event::Dropped);
                    assert_eq!(received.recv().await, None);
                });
        });
    }
    {
        scenario_runtime::check("diagnostics_preserve_scope", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for maximum in [tracing::Level::INFO, tracing::Level::WARN] {
                        let diagnostics = Diagnostics::default();
                        let writer = diagnostics.clone();
                        let subscriber = tracing_subscriber::fmt()
                            .with_max_level(maximum)
                            .without_time()
                            .with_ansi(false)
                            .with_writer(move || writer.clone())
                            .finish();
                        let (mut task, mut events) =
                            tracing::subscriber::with_default(subscriber, || {
                                let span = tracing::info_span!("execution", token = 73);
                                let _entered = span.enter();
                                start(Behavior::FailFirst, None)
                            });
                        entered(&mut events).await;
                        assert_eq!(next(&mut events).await, Event::Dropped);
                        assert_eq!(next(&mut events).await, Event::Tick(2, 73));
                        task.stop().await.unwrap();

                        let output =
                            String::from_utf8(diagnostics.0.lock().unwrap().clone()).unwrap();
                        if maximum == tracing::Level::INFO {
                            assert!(output.contains("execution{token=73}"), "{output}");
                        } else {
                            assert!(!output.contains("execution{token=73}"), "{output}");
                        }
                        assert!(output.contains("observer callback refused"), "{output}");
                        assert!(output.contains("tick=1"), "{output}");
                        assert!(output.contains("tick refused"), "{output}");
                        assert!(output.contains(r#"observer="execution.watch""#), "{output}");
                        assert!(
                            output.contains(&format!("activation_id={}", task.activation_id())),
                            "{output}"
                        );
                    }
                });
        });
    }
    {
        scenario_runtime::check("hanging_cleanup_retained", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let gate = Arc::new(Notify::new());
                    let (mut task, mut events) = start(Behavior::Hang, Some(gate.clone()));
                    entered(&mut events).await;
                    assert!(
                        tokio::time::timeout(Duration::from_millis(1200), task.stop())
                            .await
                            .is_err()
                    );
                    assert_eq!(next(&mut events).await, Event::Dropped);
                    assert_eq!(next(&mut events).await, Event::Stopping);
                    assert!(!task.is_finished());
                    assert!(matches!(
                        events.try_recv(),
                        Err(mpsc::error::TryRecvError::Empty)
                    ));
                    gate.notify_one();
                    task.join().await.unwrap();
                    assert_eq!(next(&mut events).await, Event::Stopped);
                    assert_eq!(events.recv().await, None);
                    assert!(task.is_finished());
                });
        });
    }
    {
        scenario_runtime::check("restart_fresh_identity", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let declaration = Instance::<Resources, Capabilities>::of::<Watch>()
                        .with_context(|capabilities| capabilities.token);
                    assert_eq!(declaration.id(), "execution.watch");
                    let mut previous = None;
                    for _ in 0..2 {
                        let (events, _received) = mpsc::unbounded_channel();
                        let (identities, mut observed) = mpsc::unbounded_channel();
                        let resources = Arc::new(Resources {
                            events,
                            identities: Some(identities),
                            stop_gate: None,
                        });
                        let mut task = declaration
                            .start(
                                resources,
                                Capabilities {
                                    token: 73,
                                    behavior: Behavior::FailFirst,
                                },
                            )
                            .unwrap();
                        let activation = task.activation_id();
                        assert_eq!(task.observer_id(), declaration.id());
                        assert_ne!(previous, Some(activation));
                        for ordinal in [1, 2] {
                            let identity =
                                tokio::time::timeout(Duration::from_secs(2), observed.recv())
                                    .await
                                    .unwrap()
                                    .unwrap();
                            assert_eq!(identity, (declaration.id(), activation, ordinal));
                        }
                        task.stop().await.unwrap();
                        previous = Some(activation);
                    }
                    assert_eq!(
                        Instance::<Resources, ()>::of::<NeverStarts>().id(),
                        concat!(module_path!(), "::NeverStarts")
                    );
                });
        });
    }
    {
        scenario_runtime::check("concurrent_activation_ids", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (mut left, mut left_events) = start(Behavior::Hang, None);
                    let (mut right, mut right_events) = start(Behavior::Hang, None);
                    entered(&mut left_events).await;
                    entered(&mut right_events).await;
                    assert_eq!(left.observer_id(), right.observer_id());
                    assert_ne!(left.activation_id(), right.activation_id());
                    left.cancel();
                    right.cancel();
                    left.join().await.unwrap();
                    right.join().await.unwrap();
                });
        });
    }
}

/* === Tests === */
