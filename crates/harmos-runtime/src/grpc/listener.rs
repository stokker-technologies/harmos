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
use std::{
    collections::BTreeMap,
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    task::{Context, Poll},
};

// Third Party Imports
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::{OwnedSemaphorePermit, Semaphore, watch},
    task::JoinHandle,
};
use tonic::transport::server::{Connected, TcpConnectInfo};

// Local Imports
use super::{inbound::Inbound, manager::Settings, middleware::Middleware};
use crate::operations::resident::Activation;
use crate::operations::{Manager, RestartPolicy, Running};
use crate::services::{Resident, ServiceContext, ServiceDefinition};
use crate::{Application, Error, Result};

/* === Constants === */

const SERVICE: &str = "harmos.grpc";

/* === Macros === */

/* === Type Aliases === */

type Pending<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/* === Structs === */

pub(super) struct Resources {
    settings: Settings,
    routes: tonic::service::Routes,
    middleware: Arc<[Arc<dyn Middleware>]>,
    stopping: watch::Sender<bool>,
    address: Mutex<Option<SocketAddr>>,
}

pub(super) struct Owner<A: Application> {
    operations: Manager,
    application: std::marker::PhantomData<fn() -> A>,
    closing: AtomicBool,
    generation: AtomicU64,
    current: Mutex<BTreeMap<u64, Current>>,
    transitions: tokio::sync::Mutex<()>,
}

#[derive(Clone)]
struct Current {
    service: String,
    resources: Arc<Resources>,
    lease: Option<Arc<Running<Activation>>>,
}

struct Starting<'a, A: Application> {
    owner: &'a Owner<A>,
    generation: u64,
    ready: bool,
}

struct Listener {
    resources: Arc<Resources>,
    task: Option<JoinHandle<Result<()>>>,
}

struct Connection {
    stream: TcpStream,
    stopping: Pin<Box<dyn Future<Output = ()> + Send>>,
    stopped: bool,
    _permit: OwnedSemaphorePermit,
}

/* === Enums === */

/* === Traits === */

pub(super) trait Host: Send + Sync {
    fn attach(&self, resources: Arc<Resources>) -> Pending<'_, (u64, SocketAddr)>;
    fn detach(&self, generation: u64) -> Pending<'_, ()>;
    fn cancel(&self, generation: u64);
    fn attached(&self) -> bool;
    fn close(&self);
    fn shutdown(&self) -> Pending<'_, ()>;
    fn local_addr(&self) -> Option<SocketAddr>;
}

/* === Implementations === */

impl Resources {
    pub(super) fn new(settings: Settings, routes: tonic::service::Routes, middleware: Vec<Arc<dyn Middleware>>) -> Self {
        Self {
            settings,
            routes,
            middleware: middleware.into(),
            stopping: watch::channel(false).0,
            address: Mutex::new(None),
        }
    }
}

impl<A: Application> Owner<A> {
    pub(super) fn new(operations: Manager) -> Self {
        Self {
            operations,
            application: std::marker::PhantomData,
            closing: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            current: Mutex::new(BTreeMap::new()),
            transitions: tokio::sync::Mutex::new(()),
        }
    }

    async fn retire(&self, generation: u64) -> Result<()> {
        let Some(current) = self.current().get(&generation).cloned() else {
            return Ok(());
        };
        current.resources.stopping.send_replace(true);
        if let Some(lease) = &current.lease {
            lease.cancel();
        }
        drop(current.lease);
        match self.operations.unregister_resident(&current.service).await {
            Ok(()) | Err(Error::NotRegistered { .. }) => {
                self.current().remove(&generation);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn current(&self) -> std::sync::MutexGuard<'_, BTreeMap<u64, Current>> {
        self.current.lock().expect("gRPC listener lock is healthy")
    }
}

impl<A: Application> Drop for Starting<'_, A> {
    fn drop(&mut self) {
        if !self.ready {
            self.owner.cancel(self.generation);
        }
    }
}

impl<A: Application> Host for Owner<A> {
    fn attach(&self, resources: Arc<Resources>) -> Pending<'_, (u64, SocketAddr)> {
        Box::pin(async move {
            let _transition = self.transitions.lock().await;
            if self.closing.load(Ordering::Acquire) {
                return Err(Error::Closing { domain: "gRPC listener" });
            }
            // Reap Dropped Or Cancelled Attachments Without Touching Live Siblings.
            let retiring = {
                let current = self.current();
                current.iter().filter(|(_, current)| *current.resources.stopping.borrow()).map(|(id, _)| *id).collect::<Vec<_>>()
            };
            for generation in retiring {
                self.retire(generation).await?;
            }
            let generation = self
                .generation
                .try_update(Ordering::AcqRel, Ordering::Acquire, |current| current.checked_add(1))
                .map_err(|_| Error::ResourceExhausted { resource: "gRPC listener generations" })?
                + 1;
            let service = format!("{SERVICE}.{generation}");
            self.operations.register_service::<A, _, _, _>(
                ServiceDefinition {
                    id: service.clone(),
                    policy: crate::ServicePolicy {
                        stop_timeout: resources.settings.shutdown_timeout,
                        restart: RestartPolicy::Never,
                    },
                },
                |resources: Arc<Resources>| Listener { resources, task: None },
            )?;
            self.current().insert(
                generation,
                Current {
                    service: service.clone(),
                    resources: resources.clone(),
                    lease: None,
                },
            );
            let mut starting = Starting { owner: self, generation, ready: false };
            if self.closing.load(Ordering::Acquire) {
                self.retire(generation).await?;
                return Err(Error::Closing { domain: "gRPC listener" });
            }
            let started = async {
                let lease = self.operations.submit_with_resources_at(&service, resources.clone()).await?;
                let lease = Arc::new(lease);
                if let Some(current) = self.current().get_mut(&generation) {
                    current.lease = Some(lease.clone());
                }
                if *resources.stopping.borrow() {
                    lease.cancel();
                }
                lease.ready().await?;
                resources.address.lock().expect("gRPC address lock is healthy").ok_or(Error::NotReady { domain: "gRPC listener" })
            };
            match started.await {
                Ok(address) => {
                    starting.ready = true;
                    Ok((generation, address))
                }
                Err(error) => {
                    self.retire(generation).await?;
                    Err(error)
                }
            }
        })
    }

    fn detach(&self, generation: u64) -> Pending<'_, ()> {
        Box::pin(async move {
            let _transition = self.transitions.lock().await;
            self.retire(generation).await
        })
    }

    fn cancel(&self, generation: u64) {
        let current = self.current();
        let Some(current) = current.get(&generation) else {
            return;
        };
        current.resources.stopping.send_replace(true);
        if let Some(lease) = &current.lease {
            lease.cancel();
        }
    }

    fn attached(&self) -> bool {
        self.current().values().any(|current| !*current.resources.stopping.borrow())
    }

    fn close(&self) {
        self.closing.store(true, Ordering::Release);
        for current in self.current().values() {
            current.resources.stopping.send_replace(true);
            if let Some(lease) = &current.lease {
                lease.cancel();
            }
        }
    }

    fn shutdown(&self) -> Pending<'_, ()> {
        Box::pin(async move {
            self.close();
            let _transition = self.transitions.lock().await;
            let retiring = self.current().keys().copied().collect::<Vec<_>>();
            let mut failure = None;
            for generation in retiring {
                if let Err(error) = self.retire(generation).await {
                    failure.get_or_insert(error);
                }
            }
            failure.map_or(Ok(()), Err)
        })
    }

    fn local_addr(&self) -> Option<SocketAddr> {
        let current = self.current();
        let mut attached = current.values().filter(|current| !*current.resources.stopping.borrow());
        let first = attached.next()?;
        if attached.next().is_some() {
            return None;
        }
        *first.resources.address.lock().expect("gRPC address lock is healthy")
    }
}

impl<A: Application> Resident<A> for Listener {
    type Error = Error;

    async fn run(&mut self, mut context: ServiceContext<A>) -> std::result::Result<(), Error> {
        if *self.resources.stopping.borrow() {
            return Ok(());
        }
        let listener = TcpListener::bind(self.resources.settings.address).await.map_err(failure)?;
        let address = listener.local_addr().map_err(failure)?;
        let slots = Arc::new(Semaphore::new(self.resources.settings.connections.get()));
        let stopping = self.resources.stopping.subscribe();
        // Healthy Connections Drain Gracefully; Stalled Ones Are Aborted Before The Stop Deadline.
        let grace = self.resources.settings.shutdown_timeout / 2;
        let incoming = futures_util::stream::unfold((listener, slots, stopping), move |(listener, slots, stopping)| async move {
            let permit = slots.clone().acquire_owned().await.ok()?;
            let connection = listener.accept().await.and_then(|(stream, _)| {
                stream.set_nodelay(true)?;
                let mut cancellation = stopping.clone();
                Ok(Connection {
                    stream,
                    stopping: Box::pin(async move {
                        let _ = cancellation.wait_for(|stopping| *stopping).await;
                        tokio::time::sleep(grace).await;
                    }),
                    stopped: false,
                    _permit: permit,
                })
            });
            Some((connection, (listener, slots, stopping)))
        });
        let inbound = Inbound {
            routes: self.resources.routes.clone(),
            middleware: self.resources.middleware.clone(),
            timeout: self.resources.settings.request_timeout,
            stopping: self.resources.stopping.subscribe(),
        };
        let mut stopping = self.resources.stopping.subscribe();
        let mut server = tonic::transport::Server::builder();
        server = server
            .concurrency_limit_per_connection(self.resources.settings.calls_per_connection.get())
            .max_concurrent_streams(Some(self.resources.settings.calls_per_connection.get() as u32));
        if let Some(tls) = &self.resources.settings.tls {
            server = server.tls_config(tls.clone()).map_err(failure)?;
        }
        let shutdown = async move {
            while !*stopping.borrow_and_update() {
                if stopping.changed().await.is_err() {
                    break;
                }
            }
        };
        // Cross-Origin Preflight Wraps gRPC-Web; Actual Calls Keep Ingress Authentication.
        let web = self.resources.settings.web;
        let origins = super::origin::CrossOrigin::new(&self.resources.settings.allowed_origins);
        self.task = Some(tokio::spawn(async move {
            if web {
                server
                    .accept_http1(true)
                    .layer(tower::util::option_layer(origins))
                    .layer(tonic_web::GrpcWebLayer::new())
                    .serve_with_incoming_shutdown(inbound, incoming, shutdown)
                    .await
                    .map_err(failure)
            } else {
                server.serve_with_incoming_shutdown(inbound, incoming, shutdown).await.map_err(failure)
            }
        }));
        *self.resources.address.lock().expect("gRPC address lock is healthy") = Some(address);
        tracing::info!(%address, "gRPC listener ready");
        context.ready();
        let completed = context.until_stopping(self.task.as_mut().expect("listener owns serving task")).await;
        if let Some(result) = completed {
            self.task.take();
            result.map_err(failure)??;
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        tracing::debug!("stopping gRPC listener");
        self.resources.stopping.send_replace(true);
        if let Some(task) = &mut self.task {
            let joined = tokio::time::timeout(self.resources.settings.shutdown_timeout, task)
                .await
                .map_err(|_| Error::NotReady { domain: "gRPC connection settlement" })?;
            self.task.take();
            joined.map_err(failure)??;
        }
        *self.resources.address.lock().expect("gRPC address lock is healthy") = None;
        tracing::info!("gRPC listener stopped");
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Connected for Connection {
    type ConnectInfo = TcpConnectInfo;
    fn connect_info(&self) -> TcpConnectInfo {
        self.stream.connect_info()
    }
}

impl Connection {
    // Abort Socket I/O After The Grace So Tonic Can Settle Stalled Connection Tasks.
    fn poll_stopping(&mut self, context: &mut Context<'_>) -> std::io::Result<()> {
        if !self.stopped {
            self.stopped = self.stopping.as_mut().poll(context).is_ready();
        }
        if self.stopped {
            return Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, "gRPC listener is stopping"));
        }
        Ok(())
    }
}

impl AsyncRead for Connection {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buffer: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        self.poll_stopping(cx)?;
        Pin::new(&mut self.stream).poll_read(cx, buffer)
    }
}
impl AsyncWrite for Connection {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, bytes: &[u8]) -> Poll<std::io::Result<usize>> {
        self.poll_stopping(cx)?;
        Pin::new(&mut self.stream).poll_write(cx, bytes)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.poll_stopping(cx)?;
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.poll_stopping(cx)?;
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn failure(source: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::TaskFailed {
        domain: "gRPC listener",
        source: Arc::new(source),
    }
}

/* === Tests === */
