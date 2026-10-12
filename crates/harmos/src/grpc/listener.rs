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
use super::{ingress::Ingress, manager::Settings, middleware::Middleware};
use crate::services::{
    Fault, Resident, Service, ServiceDefinition, ServiceLease, ServiceManager, ServicePolicy,
};
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
    services: ServiceManager<A>,
    closing: AtomicBool,
    generation: AtomicU64,
    current: Mutex<Option<Current<A>>>,
    transitions: tokio::sync::Mutex<()>,
}

struct Current<A: Application> {
    generation: u64,
    resources: Arc<Resources>,
    lease: Option<Arc<ServiceLease<A>>>,
}

struct Listener {
    resources: Arc<Resources>,
    task: Option<JoinHandle<Result<()>>>,
}

struct Connection {
    stream: TcpStream,
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
    pub(super) fn new(
        settings: Settings,
        routes: tonic::service::Routes,
        middleware: Vec<Arc<dyn Middleware>>,
    ) -> Self {
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
    pub(super) fn new(services: ServiceManager<A>) -> Self {
        Self {
            services,
            closing: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            current: Mutex::new(None),
            transitions: tokio::sync::Mutex::new(()),
        }
    }

    async fn retire(&self, current: Current<A>) -> Result<()> {
        current.resources.stopping.send_replace(true);
        if let Some(lease) = &current.lease {
            lease.instance().cancel();
        }
        drop(current.lease);
        match self.services.unregister(SERVICE).await {
            Ok(()) | Err(Error::NotRegistered { .. }) => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn current(&self) -> std::sync::MutexGuard<'_, Option<Current<A>>> {
        self.current.lock().expect("gRPC listener lock is healthy")
    }
}

impl<A: Application> Host for Owner<A> {
    fn attach(&self, resources: Arc<Resources>) -> Pending<'_, (u64, SocketAddr)> {
        Box::pin(async move {
            let _transition = self.transitions.lock().await;
            if self.closing.load(Ordering::Acquire) {
                return Err(Error::Closing {
                    domain: "gRPC listener",
                });
            }
            // A Dropped Attachment Is Cancelled But Still Owns Its Service
            let retiring = {
                let mut current = self.current();
                match current.as_ref() {
                    Some(attached) if !*attached.resources.stopping.borrow() => {
                        return Err(Error::AlreadyRegistered {
                            domain: "gRPC listener",
                            name: SERVICE.into(),
                        });
                    }
                    Some(_) => current.take(),
                    None => None,
                }
            };
            if let Some(retiring) = retiring {
                self.retire(retiring).await?;
            }
            let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
            self.services.register(
                ServiceDefinition {
                    id: SERVICE.into(),
                    policy: ServicePolicy {
                        credit: 0,
                        stop_timeout: resources.settings.shutdown_timeout,
                        ..ServicePolicy::default()
                    },
                },
                |resources: Arc<Resources>| Listener {
                    resources,
                    task: None,
                },
            )?;
            *self.current() = Some(Current {
                generation,
                resources: resources.clone(),
                lease: None,
            });
            let started = async {
                let scope = self.services.scope(resources.clone())?;
                let lease = Arc::new(self.services.activate(SERVICE, &scope).await?);
                if let Some(current) = self.current().as_mut() {
                    current.lease = Some(lease.clone());
                }
                if *resources.stopping.borrow() {
                    lease.instance().cancel();
                }
                lease.ready().await?;
                resources
                    .address
                    .lock()
                    .expect("gRPC address lock is healthy")
                    .ok_or(Error::NotReady {
                        domain: "gRPC listener",
                    })
            };
            match started.await {
                Ok(address) => Ok((generation, address)),
                Err(error) => {
                    let failed = self.current().take();
                    if let Some(failed) = failed {
                        self.retire(failed).await?;
                    }
                    Err(error)
                }
            }
        })
    }

    fn detach(&self, generation: u64) -> Pending<'_, ()> {
        Box::pin(async move {
            let _transition = self.transitions.lock().await;
            let retiring = {
                let mut current = self.current();
                if current
                    .as_ref()
                    .is_some_and(|current| current.generation == generation)
                {
                    current.take()
                } else {
                    None
                }
            };
            match retiring {
                Some(retiring) => self.retire(retiring).await,
                None => Ok(()),
            }
        })
    }

    fn cancel(&self, generation: u64) {
        let current = self.current();
        let Some(current) = current
            .as_ref()
            .filter(|current| current.generation == generation)
        else {
            return;
        };
        current.resources.stopping.send_replace(true);
        if let Some(lease) = &current.lease {
            lease.instance().cancel();
        }
    }

    fn attached(&self) -> bool {
        self.current()
            .as_ref()
            .is_some_and(|current| !*current.resources.stopping.borrow())
    }

    fn close(&self) {
        self.closing.store(true, Ordering::Release);
        let generation = self.current().as_ref().map(|current| current.generation);
        if let Some(generation) = generation {
            self.cancel(generation);
        }
    }

    fn shutdown(&self) -> Pending<'_, ()> {
        Box::pin(async move {
            self.close();
            let _transition = self.transitions.lock().await;
            let retiring = self.current().take();
            match retiring {
                Some(retiring) => self.retire(retiring).await,
                None => Ok(()),
            }
        })
    }

    fn local_addr(&self) -> Option<SocketAddr> {
        let current = self.current();
        let current = current
            .as_ref()
            .filter(|current| !*current.resources.stopping.borrow())?;
        *current
            .resources
            .address
            .lock()
            .expect("gRPC address lock is healthy")
    }
}

impl<A: Application> Resident<A> for Listener {
    type Error = Error;

    async fn attempt(&mut self, mut context: Service<A>) -> std::result::Result<(), Fault<Error>> {
        if *self.resources.stopping.borrow() {
            return Ok(());
        }
        let listener = TcpListener::bind(self.resources.settings.address)
            .await
            .map_err(|source| Fault::Application(failure(source)))?;
        let address = listener
            .local_addr()
            .map_err(|source| Fault::Application(failure(source)))?;
        let slots = Arc::new(Semaphore::new(self.resources.settings.connections.get()));
        let incoming =
            futures_util::stream::unfold((listener, slots), |(listener, slots)| async move {
                let permit = slots.clone().acquire_owned().await.ok()?;
                let connection = listener.accept().await.map(|(stream, _)| Connection {
                    stream,
                    _permit: permit,
                });
                Some((connection, (listener, slots)))
            });
        let ingress = Ingress {
            routes: self.resources.routes.clone(),
            middleware: self.resources.middleware.clone(),
            timeout: self.resources.settings.request_timeout,
            stopping: self.resources.stopping.subscribe(),
        };
        let mut stopping = self.resources.stopping.subscribe();
        let mut server = tonic::transport::Server::builder();
        server = server
            .concurrency_limit_per_connection(self.resources.settings.calls_per_connection.get())
            .max_concurrent_streams(Some(
                self.resources.settings.calls_per_connection.get() as u32
            ));
        if let Some(tls) = &self.resources.settings.tls {
            server = server
                .tls_config(tls.clone())
                .map_err(|source| Fault::Application(failure(source)))?;
        }
        let shutdown = async move {
            while !*stopping.borrow_and_update() {
                if stopping.changed().await.is_err() {
                    break;
                }
            }
        };
        // gRPC-Web Translates Outside Ingress, So Authentication Sees gRPC.
        let web = self.resources.settings.web;
        self.task = Some(tokio::spawn(async move {
            if web {
                server
                    .accept_http1(true)
                    .layer(tonic_web::GrpcWebLayer::new())
                    .serve_with_incoming_shutdown(ingress, incoming, shutdown)
                    .await
                    .map_err(failure)
            } else {
                server
                    .serve_with_incoming_shutdown(ingress, incoming, shutdown)
                    .await
                    .map_err(failure)
            }
        }));
        *self
            .resources
            .address
            .lock()
            .expect("gRPC address lock is healthy") = Some(address);
        tracing::info!(%address, "gRPC listener ready");
        context.ready();
        let completed = context
            .until_stopping(self.task.as_mut().expect("listener owns serving task"))
            .await;
        if let Some(result) = completed {
            self.task.take();
            result
                .map_err(|source| Fault::Application(failure(source)))?
                .map_err(Fault::Application)?;
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        tracing::debug!("stopping gRPC listener");
        self.resources.stopping.send_replace(true);
        if let Some(task) = &mut self.task {
            let joined = tokio::time::timeout(self.resources.settings.shutdown_timeout, task)
                .await
                .map_err(|_| Error::NotReady {
                    domain: "gRPC connection settlement",
                })?;
            self.task.take();
            joined.map_err(failure)??;
        }
        *self
            .resources
            .address
            .lock()
            .expect("gRPC address lock is healthy") = None;
        tracing::info!("gRPC listener stopped");
        Ok(())
    }

    async fn finalize(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Connected for Connection {
    type ConnectInfo = TcpConnectInfo;
    fn connect_info(&self) -> TcpConnectInfo {
        self.stream.connect_info()
    }
}

impl AsyncRead for Connection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buffer)
    }
}
impl AsyncWrite for Connection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, bytes)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
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
