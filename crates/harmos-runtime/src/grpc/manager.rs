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

pub use crate::config::GrpcSettings as Settings;

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeSet,
    convert::Infallible,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

// Third Party Imports
use tonic::{
    body::Body,
    codegen::{Service, http},
    server::NamedService,
};

// Local Imports
use super::{listener, middleware::Middleware};
use crate::{Application, Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct GrpcManager {
    access: Arc<std::sync::OnceLock<AccessOwner>>,
    access_setup: Arc<tokio::sync::Mutex<()>>,
    state: Arc<Mutex<State>>,
    host: Arc<dyn listener::Host>,
    operations: crate::operations::Manager,
}

#[must_use = "dropping the attachment detaches the listener"]
pub struct Attached {
    host: Arc<dyn listener::Host>,
    generation: u64,
    address: SocketAddr,
}

struct AccessOwner {
    control: Arc<super::access::Access>,
    _port: Arc<dyn harmos_operations::access::Access>,
}

struct State {
    routes: tonic::service::Routes,
    services: BTreeSet<String>,
    middleware: Vec<Arc<dyn Middleware>>,
    closed: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl GrpcManager {
    pub(crate) fn new<A: Application>(operations: &crate::operations::Manager) -> Self {
        Self {
            access: Arc::new(std::sync::OnceLock::new()),
            access_setup: Arc::new(tokio::sync::Mutex::new(())),
            state: Arc::new(Mutex::new(State {
                routes: tonic::service::Routes::default(),
                services: BTreeSet::new(),
                middleware: Vec::new(),
                closed: false,
            })),
            host: Arc::new(listener::Owner::<A>::new(operations.clone())),
            operations: operations.clone(),
        }
    }

    pub fn register_service<S>(&self, service: S) -> Result<()>
    where
        S: Service<http::Request<Body>, Response = http::Response<Body>, Error = Infallible> + NamedService + Clone + Send + Sync + 'static,
        S::Future: Send + 'static,
    {
        let mut state = self.state.lock().expect("gRPC configuration lock is healthy");
        state.configurable(self.host.attached())?;
        if state.services.contains(S::NAME) {
            return Err(Error::AlreadyRegistered {
                domain: "gRPC service",
                name: S::NAME.to_owned(),
            });
        }
        self.operations.reserve_grpc_service(S::NAME)?;
        state.services.insert(S::NAME.to_owned());
        state.routes = state.routes.clone().add_service(service);
        Ok(())
    }

    pub fn register_middleware(&self, middleware: Arc<dyn Middleware>) -> Result<()> {
        let mut state = self.state.lock().expect("gRPC configuration lock is healthy");
        state.configurable(self.host.attached())?;
        state.middleware.push(middleware);
        Ok(())
    }

    pub async fn attach(&self, settings: Settings) -> Result<Attached> {
        self.attach_inner(settings, None, Vec::new()).await
    }

    pub async fn attach_with_certificate(
        &self, settings: Settings, certificate: super::ServerCertificate, handshake_timeout: std::time::Duration,
    ) -> Result<Attached> {
        if settings.tls.is_some() || handshake_timeout.is_zero() || self.operations.clock().now().checked_add(handshake_timeout).is_none() {
            return Err(Error::InvalidInput {
                field: "gRPC reloadable TLS",
                reason: "requires no fixed TLS configuration and a positive supported handshake timeout",
            });
        }
        self.attach_inner(settings, Some((certificate, handshake_timeout)), Vec::new()).await
    }

    async fn attach_inner(
        &self, settings: Settings, certificate: Option<(super::ServerCertificate, std::time::Duration)>,
        extra_middleware: Vec<Arc<dyn Middleware>>,
    ) -> Result<Attached> {
        settings.validate(self.operations.clock())?;
        if self.operations.resources().resource::<harmos_storage::StorageManager>().is_ok() {
            let required = self.operations.transfer_message_bytes().get();
            for (field, bytes) in [
                ("GrpcSettings.operations.decode.message_bytes", settings.operations.decode.message_bytes.get()),
                ("GrpcSettings.operations.encode.message_bytes", settings.operations.encode.message_bytes.get()),
            ] {
                if bytes < required {
                    return Err(Error::InvalidInput {
                        field,
                        reason: "Start gRPC: ceiling cannot carry StorageSettings.transfer.chunk_bytes plus framing",
                    });
                }
            }
        }
        let router = super::routing::Router::new(self.operations.clone(), settings.operations).map_err(|_| Error::InvalidInput {
            field: "gRPC operation settings",
            reason: "invalid transport bounds",
        })?;
        let resources = {
            let state = self.state.lock().expect("gRPC configuration lock is healthy");
            if state.closed {
                return Err(Error::Closing { domain: "gRPC configuration" });
            }
            let methods = self.operations.grpc_methods();
            let mut routes = state.routes.clone();
            if settings.reflection {
                let service = super::reflection::Reflection::service(self.operations.clone());
                if state.services.contains(
                    <tonic_reflection::pb::v1::server_reflection_server::ServerReflectionServer<
                        super::reflection::Reflection,
                    > as NamedService>::NAME,
                ) {
                    return Err(Error::AlreadyRegistered {
                        domain: "gRPC service",
                        name: "grpc.reflection.v1.ServerReflection".to_owned(),
                    });
                }
                routes = routes.add_service(service);
            }
            tracing::info!(
                address = %settings.address,
                registered_services = state.services.len(),
                registered_routes = methods.len(),
                middleware = state.middleware.len(),
                reflection = settings.reflection,
                "starting gRPC listener"
            );
            let routes = routes.into_axum_router().fallback_service(router);
            let mut middleware = extra_middleware;
            middleware.extend(state.middleware.iter().cloned());
            Arc::new(listener::Resources::new(settings, routes.into(), self.operations.clone(), middleware, certificate))
        };
        let (generation, address) = self.host.attach(resources).await?;
        Ok(Attached {
            host: self.host.clone(),
            generation,
            address,
        })
    }

    pub async fn configure_access(
        &self, settings: super::access::AccessSettings, store: Arc<dyn super::access::CredentialStore>,
    ) -> Result<Arc<super::access::Access>> {
        let _configuration = self.access_setup.lock().await;
        if self.access.get().is_some() {
            return Err(Error::AlreadyRegistered {
                domain: "host access",
                name: "Access".into(),
            });
        }
        // The Listener View Shares Transport Ownership Without Retaining Its Access Owner.
        let manager = Self {
            state: self.state.clone(),
            host: self.host.clone(),
            operations: self.operations.clone(),
            access: Arc::new(std::sync::OnceLock::new()),
            access_setup: Arc::new(tokio::sync::Mutex::new(())),
        };
        let control = super::access::Access::new(manager, settings, store).await.map_err(access_failure)?;
        let port: Arc<dyn harmos_operations::access::Access> = Arc::new(super::access::Port(control.clone()));
        let catalog = harmos_operations::access::catalog().map_err(access_failure)?;
        use crate::operations::RegistrationExt;
        control.activate().await.map_err(access_failure)?;
        if let Err(error) = self.operations.register_native(catalog.lane(crate::operations::Lane::default())) {
            control.shutdown().await.map_err(access_failure)?;
            return Err(error);
        }
        *self.operations.registry.access.0.write().expect("access port lock is healthy") = Some(Arc::downgrade(&port));
        self.access.set(AccessOwner { control: control.clone(), _port: port }).map_err(|_| Error::AlreadyRegistered {
            domain: "host access",
            name: "Access".into(),
        })?;
        Ok(control)
    }

    pub fn access(&self) -> Result<Arc<super::access::Access>> {
        self.access.get().map(|owner| owner.control.clone()).ok_or(Error::NotReady { domain: "Access" })
    }

    pub(super) async fn attach_access(
        &self, settings: Settings, certificate: super::ServerCertificate, handshake_timeout: std::time::Duration,
        middleware: Arc<dyn Middleware>,
    ) -> Result<Attached> {
        self.attach_inner(settings, Some((certificate, handshake_timeout)), vec![middleware]).await
    }

    pub fn methods(&self) -> Vec<crate::operations::GrpcMethod> {
        self.operations.grpc_methods()
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.host.local_addr()
    }

    pub(crate) fn close_admission(&self) {
        self.state.lock().expect("gRPC configuration lock is healthy").closed = true;
        self.host.close();
    }

    #[tracing::instrument(target = "harmos::grpc", name = "grpc.shutdown", level = "debug", skip_all)]
    pub async fn shutdown(&self) -> Result<()> {
        self.close_admission();
        if let Some(access) = self.access.get() {
            access.control.shutdown().await.map_err(access_failure)?;
        }
        self.host.shutdown().await
    }
}

impl Attached {
    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }

    pub async fn detach(self) -> Result<()> {
        self.host.detach(self.generation).await
    }
}

impl Drop for Attached {
    fn drop(&mut self) {
        self.host.request_cancel(self.generation);
    }
}

impl Settings {
    fn validate(&self, clock: &crate::clock::Clock) -> Result<()> {
        if self.request_timeout.is_zero()
            || self.shutdown_timeout.is_zero()
            || clock.now().checked_add(self.request_timeout).is_none()
            || clock.now().checked_add(self.shutdown_timeout).is_none()
            || self.connections.get() > tokio::sync::Semaphore::MAX_PERMITS
            || self.calls_per_connection.get() > u32::MAX as usize
        {
            return Err(Error::InvalidInput {
                field: "gRPC settings",
                reason: "timeouts and capacity must be positive supported bounds",
            });
        }
        Ok(())
    }
}

impl State {
    fn configurable(&self, attached: bool) -> Result<()> {
        if attached || self.closed {
            return Err(Error::Closing { domain: "gRPC configuration" });
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn access_failure(source: harmos_operations::execution::operations::ServingError) -> Error {
    Error::OperationFailed {
        operation: "Access.Configure".into(),
        source: Arc::new(source),
    }
}

/* === Tests === */
