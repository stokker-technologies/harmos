/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Connection-Owned gRPC Plugin Placement

/* === Modules === */

mod catalog;
pub(crate) mod clock;
mod config;
mod connection;
pub(crate) mod invoke;
mod managed;
mod publish;
mod rows;

/* === Re-Exports === */

pub use config::PlacementSettings;
pub use harmos_protocol::v1;
pub use invoke::CallError;
pub use managed::Managed;

/* === Imports === */

// Standard Library Imports
use std::{future::Future, sync::Arc};

// Third Party Imports
use harmos_protocol::v1::plugin_client::PluginClient;
use tokio::sync::watch;

// Local Imports
use crate::operations::Lane;
use crate::{Application, Runtime, clock::Clock};
use connection::Connection;
use managed::Process;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Outcome = Result<(), Arc<Error>>;

/* === Structs === */

#[derive(Clone)]
pub struct Activation {
    pub number: u64,
    pub description: v1::Description,
    pub previous_started_at: Option<prost_types::Timestamp>,
    pub executable_digest: Option<[u8; 32]>,
    connection: Arc<Connection>,
    clock: Clock,
    admission: Option<Arc<crate::operations::GrpcAdmission>>,
}

#[derive(Clone)]
pub struct Status {
    pub activation: Option<Activation>,
    pub failure: Option<Arc<Error>>,
}

pub struct Attached {
    status: watch::Receiver<Status>,
    stop: watch::Sender<Option<String>>,
    stopped: watch::Receiver<Option<Outcome>>,
}

struct Session<A: Application> {
    operations: crate::operations::Manager,
    streams: crate::streams::StreamManager<A>,
    lane: &'static str,
    clock: Clock,
    settings: PlacementSettings,
    process: Option<Process>,
    address: String,
    activation: Activation,
    publications: publish::Publications,
    stop: watch::Receiver<Option<String>>,
    participant: crate::runtime::StopParticipant,
}

/* === Enums === */

#[derive(Clone, Debug)]
pub enum Placement {
    Managed(Managed),
    Remote(String),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),

    #[error("plugin transport failed: {0}")]
    Transport(String),

    #[error("plugin connection closed")]
    Disconnected,

    #[error("plugin {0} exceeded its host-clock budget")]
    Timeout(&'static str),

    #[error("managed plugin exited with {0}")]
    Exit(std::process::ExitStatus),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/* === Traits === */

/* === Implementations === */

impl<A: Application> Runtime<A> {
    pub async fn attach_grpc(&self, placement: Placement, settings: PlacementSettings) -> Result<Attached, Error> {
        let participant = self.stop_participant();
        if participant.is_stopping() {
            return Err(Error::Disconnected);
        }
        Attached::start(
            self.operations.execution(),
            self.streams.clone(),
            <A::Lanes as crate::operations::Lane>::default_lane().name(),
            placement,
            settings,
            participant,
        )
        .await
    }
}

impl Activation {
    pub fn is_connected(&self) -> bool {
        !*self.connection.closed.borrow()
    }

    /// Inspect This Connection Only; An Old Handle Never Follows Reconnection.
    pub async fn describe(&self) -> Result<v1::Description, Error> {
        self.connected(describe(&self.connection)).await
    }

    async fn connected<T>(&self, future: impl Future<Output = Result<T, Error>>) -> Result<T, Error> {
        let _activity = self.clock.activity("plugin transport");
        let closed = self.connection.closed.subscribe();
        tokio::select! {
            biased;
            () = disconnected(closed) => Err(Error::Disconnected),
            result = future => result,
        }
    }
}

impl Attached {
    async fn start<A: Application>(
        operations: crate::operations::Manager, streams: crate::streams::StreamManager<A>, lane: &'static str, placement: Placement,
        settings: PlacementSettings, participant: crate::runtime::StopParticipant,
    ) -> Result<Self, Error> {
        let clock = operations.clock().clone();
        if settings.connect_timeout.is_zero() || settings.reconnect_delay.is_zero() || settings.stop_grace.is_zero() {
            return Err(Error::Invalid("Placement Durations Must Be Positive".into()));
        }
        let _activity = clock.activity("plugin startup");
        let setup = async {
            let (process, address) = match &placement {
                Placement::Managed(managed) => {
                    let (process, address) = Process::start(managed).await?;
                    (Some(process), address)
                }
                Placement::Remote(address) => (None, address.clone()),
            };
            let mut activation = connect(&clock, &address, 1, None, process.as_ref().map(|process| process.digest)).await?;
            let admission = operations.admit_grpc(&activation, lane).inspect_err(|_| {
                activation.connection.closed.send_replace(true);
            })?;
            activation.admission = Some(Arc::new(admission));
            let publications = publish::Publications::open(&streams, &activation, &clock, &settings).await?;
            Ok::<_, Error>((process, address, activation, publications))
        };
        let (process, address, activation, publications) =
            clock.timeout(settings.connect_timeout, setup).await.map_err(|_| Error::Timeout("startup"))??;
        let (status, receiver) = watch::channel(Status {
            activation: Some(activation.clone()),
            failure: None,
        });
        let (stop, stopping) = watch::channel(None);
        let (stopped, completed) = watch::channel(None);
        tokio::spawn(async move {
            let session = Session {
                operations,
                streams,
                lane,
                clock,
                settings,
                process,
                address,
                activation,
                publications,
                stop: stopping,
                participant,
            };
            let result = supervise(session, &status).await;
            status.send_modify(|status| {
                status.activation = None;
                if let Err(error) = &result {
                    status.failure = Some(error.clone());
                }
            });
            stopped.send_replace(Some(result));
        });
        Ok(Self {
            status: receiver,
            stop,
            stopped: completed,
        })
    }

    pub fn status(&self) -> watch::Receiver<Status> {
        self.status.clone()
    }

    pub fn activation(&self) -> Option<Activation> {
        self.status.borrow().activation.clone()
    }

    pub async fn stop(&self, reason: impl Into<String>) -> Outcome {
        self.stop.send_if_modified(|current| {
            if current.is_some() {
                return false;
            }
            *current = Some(reason.into());
            true
        });
        let mut stopped = self.stopped.clone();
        loop {
            if let Some(result) = stopped.borrow_and_update().clone() {
                return result;
            }
            if stopped.changed().await.is_err() {
                return Err(Arc::new(Error::Disconnected));
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn describe(connection: &Connection) -> Result<v1::Description, Error> {
    let mut client = PluginClient::new(connection.channel.clone());
    client
        .describe(v1::DescribeRequest {
            revision: 1,
            host: "harmos".into(),
            host_version: env!("CARGO_PKG_VERSION").into(),
        })
        .await
        .map(|reply| reply.into_inner())
        .map_err(|error| Error::Transport(error.to_string()))
}

async fn connect(
    clock: &Clock, address: &str, number: u64, previous_started_at: Option<prost_types::Timestamp>, executable_digest: Option<[u8; 32]>,
) -> Result<Activation, Error> {
    let connection = Arc::new(Connection::open(address).await?);
    let description = describe(&connection).await?;
    let valid_time = description
        .started_at
        .is_some_and(|time| (-62_135_596_800..=253_402_300_799).contains(&time.seconds) && (0..1_000_000_000).contains(&time.nanos));
    if description.revision == 0
        || description.id.is_empty()
        || description.sdk.is_empty()
        || description.sdk_version.is_empty()
        || semver::Version::parse(&description.version).is_err()
        || !valid_time
        || description.catalog.is_none()
    {
        return Err(Error::Invalid("Invalid Plugin Description".into()));
    }
    catalog::verify(description.catalog.as_ref().expect("description catalog was checked"))?;
    Ok(Activation {
        number,
        description,
        previous_started_at,
        executable_digest,
        connection,
        admission: None,
        clock: clock.clone(),
    })
}

async fn disconnected(mut closed: watch::Receiver<bool>) {
    while !*closed.borrow_and_update() {
        if closed.changed().await.is_err() {
            break;
        }
    }
}

async fn stopping(stop: &mut watch::Receiver<Option<String>>, participant: &mut crate::runtime::StopParticipant) -> String {
    loop {
        if let Some(reason) = stop.borrow_and_update().clone() {
            return reason;
        }
        tokio::select! {
            result = stop.changed() => if result.is_err() { return "Attachment Dropped".into(); },
            () = participant.stopping() => return "Host Shutting Down".into(),
        }
    }
}

async fn supervise<A: Application>(session: Session<A>, status: &watch::Sender<Status>) -> Outcome {
    let Session {
        operations,
        streams,
        lane,
        clock,
        settings,
        mut process,
        address,
        mut activation,
        mut publications,
        mut stop,
        mut participant,
    } = session;
    loop {
        let exited = async {
            match &mut process {
                Some(process) => process.exited().await,
                None => std::future::pending().await,
            }
        };
        let reason = tokio::select! {
            biased;
            reason = stopping(&mut stop, &mut participant) => Some(reason),
            result = exited => {
                activation.connection.closed.send_replace(true);
                activation.admission.as_ref().unwrap().remove();
                return Err(Arc::new(result.map_or_else(|error| error, Error::Exit)));
            },
            () = disconnected(activation.connection.closed.subscribe()) => None,
        };
        if let Some(reason) = reason {
            let _activity = clock.activity("plugin stop");
            let deadline = clock.now() + settings.stop_grace;
            let streams_closed = clock.timeout_at(deadline, publications.close()).await;
            let mut client = PluginClient::new(activation.connection.channel.clone());
            let result = clock
                .timeout_at(deadline, client.stop(v1::StopRequest { reason }))
                .await
                .map_err(|_| Error::Timeout("Stop"))
                .and_then(|result| result.map(|_| ()).map_err(|error| Error::Transport(error.to_string())));
            let result = streams_closed.map_err(|_| Error::Timeout("Publish closure")).and(result);
            if let Err(error) = &result {
                tracing::warn!(plugin = %activation.description.id, %error, "plugin Stop failed; closing its connection");
            }
            activation.connection.closed.send_replace(true);
            activation.admission.as_ref().unwrap().remove();
            if let Some(process) = &mut process {
                let exit = clock.timeout_at(deadline, process.exited()).await;
                match exit {
                    Ok(Ok(exit)) if exit.success() => {}
                    Ok(Ok(exit)) => return Err(Arc::new(Error::Exit(exit))),
                    Ok(Err(error)) => return Err(Arc::new(error)),
                    Err(_) => {
                        let _ = process.child.kill().await;
                        return Err(Arc::new(Error::Timeout("process exit")));
                    }
                }
            }
            return result.map_err(Arc::new);
        }
        publications.close().await;
        activation.admission.as_ref().unwrap().remove();
        status.send_replace(Status {
            activation: None,
            failure: Some(Arc::new(Error::Disconnected)),
        });
        if process.is_some() {
            return Err(Arc::new(Error::Disconnected));
        }
        let previous = activation.description.started_at;
        let number = activation.number + 1;
        loop {
            tokio::select! {
                biased;
                _ = stopping(&mut stop, &mut participant) => return Ok(()),
                () = clock.sleep(settings.reconnect_delay) => {},
            }
            let _activity = clock.activity("plugin reconnect");
            let reconnect = clock.timeout(settings.connect_timeout, connect(&clock, &address, number, previous, None));
            let result = tokio::select! {
                biased;
                _ = stopping(&mut stop, &mut participant) => return Ok(()),
                result = reconnect => result,
            };
            match result {
                Ok(Ok(mut next)) => {
                    if next.description.id != activation.description.id {
                        next.connection.closed.send_replace(true);
                        status.send_modify(|status| {
                            status.failure = Some(Arc::new(Error::Invalid("Reconnect Changed Plugin Identity".into())))
                        });
                        continue;
                    }
                    match operations.admit_grpc(&next, lane) {
                        Ok(admission) => next.admission = Some(Arc::new(admission)),
                        Err(error) => {
                            next.connection.closed.send_replace(true);
                            status.send_modify(|status| status.failure = Some(Arc::new(error)));
                            continue;
                        }
                    }
                    match publish::Publications::open(&streams, &next, &clock, &settings).await {
                        Ok(opened) => publications = opened,
                        Err(error) => {
                            next.connection.closed.send_replace(true);
                            next.admission.as_ref().unwrap().remove();
                            status.send_modify(|status| status.failure = Some(Arc::new(error)));
                            continue;
                        }
                    }
                    activation = next;
                    status.send_replace(Status {
                        activation: Some(activation.clone()),
                        failure: None,
                    });
                    break;
                }
                Ok(Err(error)) => status.send_modify(|status| status.failure = Some(Arc::new(error))),
                Err(_) => status.send_modify(|status| status.failure = Some(Arc::new(Error::Timeout("reconnect")))),
            }
        }
    }
}

/* === Tests === */
