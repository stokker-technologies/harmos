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
        settings.validate()?;
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
            Arc::new(listener::Resources::new(settings, routes.into(), state.middleware.clone()))
        };
        let (generation, address) = self.host.attach(resources).await?;
        Ok(Attached {
            host: self.host.clone(),
            generation,
            address,
        })
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
    fn validate(&self) -> Result<()> {
        if !self.web && !self.allowed_origins.is_empty() {
            return Err(Error::InvalidInput {
                field: "gRPC allowed origins",
                reason: "cross-origin access requires gRPC-Web to be enabled",
            });
        }
        if self.request_timeout.is_zero()
            || self.shutdown_timeout.is_zero()
            || tokio::time::Instant::now().checked_add(self.request_timeout).is_none()
            || tokio::time::Instant::now().checked_add(self.shutdown_timeout).is_none()
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

/* === Tests === */
