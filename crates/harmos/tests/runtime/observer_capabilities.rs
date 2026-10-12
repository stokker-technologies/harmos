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
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
// Third Party Imports
use tokio::sync::mpsc;
// Local Imports
use super::support::Pulse;
use harmos::sidecar::inventory::publications;
use harmos::sidecar::{
    ActivationId, Instance, ObserverCaller, ObserverContext, ObserverPublisher,
    ObserverStreamPublisher, PublicationError, StreamSpec,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Context<'a> = ObserverContext<'a, u64, Host>;
type Identity = (&'static str, ActivationId, u64);

/* === Structs === */
struct Host {
    events: mpsc::Sender<Event>,
    hang: bool,
    dropped: Arc<AtomicBool>,
}

struct Measure;

struct Recorded(u64);

struct Denied;

struct WrongVersion;

struct Dropped(Arc<AtomicBool>);

#[derive(Default)]
struct Watch;

#[derive(Default)]
struct CheckGrants;

/* === Enums === */
#[derive(Debug, PartialEq, Eq)]
enum Event {
    Call(Identity, u64),
    Record(Identity, u64),
    Stream(Identity, u64),
    GrantsChecked,
}

#[derive(Debug, thiserror::Error)]
enum Failure {
    #[error(transparent)]
    Publication(#[from] PublicationError),
    #[error("first tick refused")]
    Refused,
}

/* === Traits === */

/* === Implementations === */
impl StreamSpec for WrongVersion {
    type Row = Pulse;
    const ID: &'static str = Pulse::ID;
    const VERSION: u32 = Pulse::VERSION + 1;
    const WINDOW: usize = Pulse::WINDOW;
}

impl ObserverCaller<u64, Measure> for Host {
    type Request = u64;
    type Response = u64;
    type Error = Failure;

    async fn call(&self, ctx: &Context<'_>, request: u64) -> Result<u64, Failure> {
        let _guard = Dropped(self.dropped.clone());
        self.events
            .send(Event::Call(identity(ctx), request))
            .await
            .unwrap();
        if self.hang {
            return std::future::pending().await;
        }
        if ctx.tick().ordinal() == 1 {
            return Err(Failure::Refused);
        }
        Ok(request + ctx.resources())
    }
}

impl ObserverPublisher<u64, Recorded> for Host {
    type Error = Failure;

    async fn publish(&self, ctx: &Context<'_>, value: Recorded) -> Result<(), Failure> {
        self.events
            .send(Event::Record(identity(ctx), value.0))
            .await
            .unwrap();
        Ok(())
    }
}

impl ObserverPublisher<u64, Pulse> for Host {
    type Error = Failure;
    const STREAM: Option<publications::Entry> = Some(publications::Entry::of::<Pulse>());

    async fn publish(&self, ctx: &Context<'_>, value: Pulse) -> Result<(), Failure> {
        self.events
            .send(Event::Stream(identity(ctx), value.sample))
            .await
            .unwrap();
        Ok(())
    }
}

impl ObserverPublisher<u64, Denied> for Host {
    type Error = Failure;
    const STREAM: Option<publications::Entry> = Some(publications::Entry::of::<WrongVersion>());

    async fn publish(&self, _: &Context<'_>, _: Denied) -> Result<(), Failure> {
        panic!("an undeclared stream must not reach its backend");
    }
}

impl<S: StreamSpec<Row = Pulse>> ObserverStreamPublisher<u64, S> for Host {
    type Error = Failure;

    async fn publish_on(&self, ctx: &Context<'_>, row: &Pulse) -> Result<(), Failure> {
        self.events
            .send(Event::Stream(identity(ctx), row.sample))
            .await
            .unwrap();
        Ok(())
    }
}

#[harmos::observer(id = "host.watch", interval_ms = 5, streams(Pulse))]
impl Watch {
    async fn observe(&mut self, ctx: &ObserverContext<'_, u64, Host>) -> Result<(), Failure> {
        let value = ctx.call::<Measure>(7).await?;
        ctx.publish(Recorded(value)).await?;
        ctx.publish(Pulse { sample: value }).await?;
        ctx.publish_on::<Pulse>(&Pulse { sample: value + 1 }).await
    }
}

#[harmos::observer(id = "host.grants", interval_ms = 5, streams(Pulse))]
impl CheckGrants {
    async fn observe(&mut self, ctx: &ObserverContext<'_, u64, Host>) -> Result<(), Failure> {
        assert!(matches!(
            ctx.publish(Denied).await,
            Err(Failure::Publication(PublicationError::Undeclared { .. }))
        ));
        assert!(matches!(
            ctx.publish_on::<WrongVersion>(&Pulse { sample: 0 }).await,
            Err(Failure::Publication(PublicationError::Undeclared { .. }))
        ));
        ctx.capabilities()
            .events
            .send(Event::GrantsChecked)
            .await
            .unwrap();
        Ok(())
    }
}

impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn identity(ctx: &Context<'_>) -> Identity {
    (ctx.observer_id(), ctx.activation_id(), ctx.tick().ordinal())
}

fn host(hang: bool) -> (Host, mpsc::Receiver<Event>, Arc<AtomicBool>) {
    let (events, received) = mpsc::channel(16);
    let dropped = Arc::new(AtomicBool::new(false));
    (
        Host {
            events,
            hang,
            dropped: dropped.clone(),
        },
        received,
        dropped,
    )
}

async fn next(events: &mut mpsc::Receiver<Event>) -> Event {
    tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .unwrap()
        .unwrap()
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("typed_context_dispatch", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (host, mut events, _) = host(false);
                    let mut task = Instance::<u64, Host>::of::<Watch>()
                        .start(Arc::new(35), host)
                        .unwrap();
                    let activation = task.activation_id();
                    assert_eq!(
                        next(&mut events).await,
                        Event::Call(("host.watch", activation, 1), 7)
                    );
                    assert_eq!(
                        next(&mut events).await,
                        Event::Call(("host.watch", activation, 2), 7)
                    );
                    assert_eq!(
                        next(&mut events).await,
                        Event::Record(("host.watch", activation, 2), 42)
                    );
                    assert_eq!(
                        next(&mut events).await,
                        Event::Stream(("host.watch", activation, 2), 42)
                    );
                    assert_eq!(
                        next(&mut events).await,
                        Event::Stream(("host.watch", activation, 2), 43)
                    );
                    task.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("version_grants_checked", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (host, mut events, _) = host(false);
                    let mut task = Instance::<u64, Host>::of::<CheckGrants>()
                        .start(Arc::new(0), host)
                        .unwrap();
                    assert_eq!(next(&mut events).await, Event::GrantsChecked);
                    task.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("stop_drops_call", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (host, mut events, dropped) = host(true);
                    let mut task = Instance::<u64, Host>::of::<Watch>()
                        .start(Arc::new(0), host)
                        .unwrap();
                    assert!(matches!(next(&mut events).await, Event::Call(..)));
                    assert!(!dropped.load(Ordering::SeqCst));
                    task.stop().await.unwrap();
                    assert!(dropped.load(Ordering::SeqCst));
                    assert_eq!(events.recv().await, None);
                });
        });
    }
}

/* === Tests === */
