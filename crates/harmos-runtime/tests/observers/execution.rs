/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::{Arc, Mutex};
// Third Party Imports
use tokio::sync::{Notify, mpsc};
// Local Imports
use super::{Observing, pending};
use harmos_runtime::sidecar::{ActivationId, Instance, ObserverContext, ObserverError};

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

struct Host {
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
#[harmos_runtime::observer(id = "execution.watch", interval_ms = 5)]
impl Watch {
    async fn initialize(resources: &Resources) -> Result<Self, std::io::Error> {
        let constructed: Result<Self, std::io::Error> =
            async move { Ok(Self(resources.clone())) }.await;
        let mut observer = constructed?;
        let started: Result<(), std::io::Error> = async {
            let observer = &mut observer;
            observer.0.events.send(Event::Opened).unwrap();
            Ok(())
        }
        .await;
        started?;
        Ok(observer)
    }

    async fn observe(
        &mut self,
        ctx: &ObserverContext<'_, Resources, Host>,
    ) -> Result<(), std::io::Error> {
        let _callback = Callback(self.0.clone());
        if let Some(identities) = &self.0.identities {
            identities
                .send((ctx.observer_id(), ctx.activation_id(), ctx.tick().ordinal()))
                .unwrap();
        }
        assert_eq!(ctx.extension::<u64>(), Some(&ctx.host().token));
        ctx.resources()
            .events
            .send(Event::Tick(ctx.tick().ordinal(), ctx.host().token))
            .unwrap();

        match ctx.host().behavior {
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

#[harmos_runtime::observer(interval_ms = 5)]
impl NeverStarts {
    async fn initialize(resources: &Resources) -> Result<Self, std::io::Error> {
        let constructed: Result<Self, std::io::Error> =
            async move { Ok(Self(resources.clone())) }.await;
        let mut observer = constructed?;
        let started: Result<(), std::io::Error> = async {
            let observer = &mut observer;
            let _initializing = Callback(observer.0.clone());
            observer.0.events.send(Event::Opened).unwrap();
            std::future::pending().await
        }
        .await;
        started?;
        Ok(observer)
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
fn declaration() -> Instance<Resources, Host> {
    Instance::<Resources, Host>::of::<Watch>().with_context(|host| host.token)
}

fn start(
    behavior: Behavior,
    stop_gate: Option<Arc<Notify>>,
) -> (Observing, mpsc::UnboundedReceiver<Event>) {
    let (events, received) = mpsc::unbounded_channel();
    let resources = Arc::new(Resources {
        events,
        stop_gate,
        identities: None,
    });
    let activity = declaration()
        .activate(
            resources,
            Host {
                token: 73,
                behavior,
            },
        )
        .unwrap();
    (Observing::start(activity), received)
}

async fn next(events: &mut mpsc::UnboundedReceiver<Event>) -> Event {
    events
        .recv()
        .await
        .expect("observer reports its lifecycle")
}

async fn entered(events: &mut mpsc::UnboundedReceiver<Event>) {
    assert_eq!(next(events).await, Event::Opened);
    assert_eq!(next(events).await, Event::Tick(1, 73));
}

#[tokio::test]
async fn shared_execution_clock() {
    let (mut task, mut events) = start(Behavior::FailFirst, None);
    entered(&mut events).await;
    assert_eq!(next(&mut events).await, Event::Dropped);
    assert_eq!(next(&mut events).await, Event::Tick(2, 73));
    assert!(task.is_observing());

    task.shutdown().await.unwrap();
    assert!(!task.is_observing());
    assert!(!task.activity().is_initialized());
    task.shutdown().await.unwrap();
}

#[tokio::test]
async fn callback_panic_isolated() {
    let (mut failed, mut failure) = start(Behavior::Panic, None);
    let (mut sibling, mut events) = start(Behavior::FailFirst, None);
    entered(&mut failure).await;
    assert_eq!(next(&mut failure).await, Event::Dropped);
    let ended = failed.ended().await;
    assert!(matches!(ended, Some(ObserverError::CallbackPanicked)));
    assert!(!failed.is_observing());
    let activity = failed.activity();
    activity.stop().await.unwrap();
    assert_eq!(next(&mut failure).await, Event::Stopping);
    assert_eq!(next(&mut failure).await, Event::Stopped);
    activity.cleanup().await.unwrap();
    drop(failed);
    assert_eq!(failure.recv().await, None);

    entered(&mut events).await;
    assert_eq!(next(&mut events).await, Event::Dropped);
    assert_eq!(next(&mut events).await, Event::Tick(2, 73));
    sibling.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancelled_stop_rejoins() {
    let gate = Arc::new(Notify::new());
    let (mut task, mut events) = start(Behavior::Hang, Some(Arc::clone(&gate)));
    entered(&mut events).await;
    assert!(task.interrupt().await.is_none());
    assert_eq!(next(&mut events).await, Event::Dropped);
    let activity = task.activity();
    pending::parked(std::pin::pin!(activity.stop())).await;
    assert_eq!(next(&mut events).await, Event::Stopping);
    assert!(activity.is_initialized());

    // An abandoned stop leaves the observer unstopped; the next
    // attempt enters the stop callback again.
    gate.notify_one();
    activity.stop().await.unwrap();
    assert_eq!(next(&mut events).await, Event::Stopping);
    assert_eq!(next(&mut events).await, Event::Stopped);
    activity.cleanup().await.unwrap();
    assert!(!activity.is_initialized());
    drop(task);
    assert_eq!(events.recv().await, None);
}

#[tokio::test]
async fn drop_cleans_callback() {
    // A host-run activation stops its observer when the last lease drops.
    let runtime = super::runtime().await;
    let manager = &runtime.operations;
    manager
        .register_native_observer(declaration(), |_| Host {
            token: 73,
            behavior: Behavior::Hang,
        })
        .unwrap();
    let (events, mut received) = mpsc::unbounded_channel();
    let resources = Arc::new(Resources {
        events,
        identities: None,
        stop_gate: None,
    });
    let lease = manager
        .submit_with_resources_at("execution.watch", resources.clone())
        .await
        .unwrap();
    entered(&mut received).await;
    drop(lease);
    assert_eq!(next(&mut received).await, Event::Dropped);
    assert_eq!(next(&mut received).await, Event::Stopping);
    assert_eq!(next(&mut received).await, Event::Stopped);
    manager.retire_resources(&resources).await.unwrap();
    drop(resources);
    assert_eq!(received.recv().await, None);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cancellation_drops_init() {
    let (events, mut received) = mpsc::unbounded_channel();
    let resources = Arc::new(Resources {
        events,
        identities: None,
        stop_gate: None,
    });
    let mut task = Observing::start(
        Instance::<Resources, ()>::of::<NeverStarts>()
            .activate(resources, ())
            .unwrap(),
    );
    assert_eq!(next(&mut received).await, Event::Opened);
    task.shutdown().await.unwrap();
    assert_eq!(next(&mut received).await, Event::Dropped);
    assert!(!task.activity().is_initialized());
    drop(task);
    assert_eq!(received.recv().await, None);
}

#[tokio::test]
async fn diagnostics_preserve_scope() {
    for maximum in [tracing::Level::INFO, tracing::Level::WARN] {
        let diagnostics = Diagnostics::default();
        let writer = diagnostics.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(maximum)
            .without_time()
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        let (mut task, mut events) = tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("execution", token = 73);
            let _entered = span.enter();
            start(Behavior::FailFirst, None)
        });
        entered(&mut events).await;
        assert_eq!(next(&mut events).await, Event::Dropped);
        assert_eq!(next(&mut events).await, Event::Tick(2, 73));
        task.shutdown().await.unwrap();

        let output = String::from_utf8(diagnostics.0.lock().unwrap().clone()).unwrap();
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
            output.contains(&format!("activation_id={}", task.activation)),
            "{output}"
        );
    }
}

#[tokio::test]
async fn restart_fresh_identity() {
    let declaration = declaration();
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
        let activity = declaration
            .activate(
                resources,
                Host {
                    token: 73,
                    behavior: Behavior::FailFirst,
                },
            )
            .unwrap();
        let activation = activity.activation_id();
        assert_eq!(activity.observer_id(), declaration.id());
        assert_ne!(previous, Some(activation));
        let mut task = Observing::start(activity);
        for ordinal in [1, 2] {
            let identity = observed.recv().await.unwrap();
            assert_eq!(identity, (declaration.id(), activation, ordinal));
        }
        task.shutdown().await.unwrap();
        previous = Some(activation);
    }
    assert_eq!(
        Instance::<Resources, ()>::of::<NeverStarts>().id(),
        concat!(module_path!(), "::NeverStarts")
    );
}

#[tokio::test]
async fn concurrent_activation_ids() {
    let (mut left, mut left_events) = start(Behavior::Hang, None);
    let (mut right, mut right_events) = start(Behavior::Hang, None);
    entered(&mut left_events).await;
    entered(&mut right_events).await;
    assert_eq!(left.observer, right.observer);
    assert_ne!(left.activation, right.activation);
    left.shutdown().await.unwrap();
    right.shutdown().await.unwrap();
}

/* === Tests === */
