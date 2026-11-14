/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod credentials;
mod pairings;

/* === Re-Exports === */

pub use credentials::{CredentialStore, Credentials, Pairing, Pending, StoreResult};

/* === Imports === */

// Standard Library Imports
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

// Third Party Imports
use harmos_operations::{access::Pending as Operation, execution::operations::ServingError};
use harmos_protocol::host::access as wire;
use tokio::sync::Mutex;

// Local Imports
use super::{Attached, GrpcManager, ServerCertificate};
use crate::services::status::{failure, identifier};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Port(pub Arc<Access>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Principal {
    pub pairing_id: String,
}

pub struct AccessSettings {
    pub listener: wire::ListenerValue,
    pub transport: super::Settings,
    pub handshake_timeout: std::time::Duration,
    pub pairings: NonZeroUsize,
    pub label_bytes: NonZeroUsize,
}
pub struct Access {
    manager: GrpcManager,
    store: Arc<dyn CredentialStore>,
    settings: AccessSettings,
    state: Mutex<State>,
    authorized: std::sync::RwLock<Vec<(String, Vec<u8>)>>,
}
struct State {
    value: Credentials,
    certificate: ServerCertificate,
    attached: Option<Attached>,
    admission: Arc<AtomicBool>,
    issued: Option<u64>,
    epoch: String,
}
struct Authorization {
    owner: std::sync::Weak<Access>,
    admission: Arc<AtomicBool>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl std::ops::Deref for Port {
    type Target = Arc<Access>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Access {
    pub(super) async fn new(
        manager: GrpcManager, settings: AccessSettings, store: Arc<dyn CredentialStore>,
    ) -> Result<Arc<Self>, ServingError> {
        let action = "Access.Configure";
        validate(&settings.listener, action)?;
        if settings.transport.tls.is_some() || settings.handshake_timeout.is_zero() {
            return Err(failure(action, "reloadable TLS requires no fixed certificate and a positive handshake timeout"));
        }
        let value = store.load().await.map_err(|_| failure(action, "credential store unavailable"))?;
        let value = match value {
            Some(value) => value,
            None => {
                let (certificate_pem, private_key_pem) = generate(&settings.listener.certificate_names, action)?;
                let value = Credentials {
                    revision: 1,
                    listener_revision: 1,
                    listener: settings.listener.clone(),
                    certificate_pem,
                    private_key_pem,
                    pairings: Vec::new(),
                };
                store.commit(0, value.clone()).await.map_err(|_| failure(action, "credential store unavailable"))?;
                value
            }
        };
        validate(&value.listener, action)?;
        if value.revision == 0 || value.listener_revision == 0 || value.pairings.len() > settings.pairings.get() {
            return Err(failure(action, "stored credentials exceed configured bounds or have an invalid revision"));
        }
        let certificate = ServerCertificate::from_pem(value.certificate_pem.as_bytes(), value.private_key_pem.as_bytes())
            .map_err(|_| failure(action, "stored certificate is invalid"))?;
        let authorized = value.pairings.iter().map(|p| (p.id.clone(), p.credential_sha256.clone())).collect();
        let control = Arc::new(Self {
            manager,
            store,
            settings,
            authorized: std::sync::RwLock::new(authorized),
            state: Mutex::new(State {
                value,
                certificate,
                attached: None,
                admission: Arc::new(AtomicBool::new(false)),
                issued: None,
                epoch: identifier(action)?,
            }),
        });
        Ok(control)
    }
    pub(super) async fn activate(self: &Arc<Self>) -> Result<(), ServingError> {
        let mut state = self.state.lock().await;
        if state.value.listener.enabled {
            let (attached, admission) = self.stage(&state.value.listener, state.certificate.clone()).await?;
            admission.store(true, Ordering::Release);
            state.admission = admission;
            state.attached = Some(attached);
        }
        Ok(())
    }
    async fn stage(
        self: &Arc<Self>, value: &wire::ListenerValue, certificate: ServerCertificate,
    ) -> Result<(Attached, Arc<AtomicBool>), ServingError> {
        let mut settings = self.settings.transport.clone();
        settings.address = value.endpoint.parse().map_err(|_| failure("Access.Listener.Set", "invalid listener endpoint"))?;
        let admission = Arc::new(AtomicBool::new(false));
        let middleware: Arc<dyn super::middleware::Middleware> = Arc::new(Authorization {
            owner: Arc::downgrade(self),
            admission: admission.clone(),
        });
        let attached = self
            .manager
            .attach_access(settings, certificate, self.settings.handshake_timeout, middleware)
            .await
            .map_err(|_| failure("Access.Listener.Set", "cannot bind listener with configured transport policy"))?;
        Ok((attached, admission))
    }
    async fn loaded(&self, state: &State, action: &'static str) -> Result<(), ServingError> {
        let stored = self
            .store
            .load()
            .await
            .map_err(|_| failure(action, "credential store unavailable"))?
            .ok_or_else(|| failure(action, "credential store lost access state"))?;
        if stored.revision != state.value.revision {
            return Err(failure(action, "credential store changed outside its manager; reload access configuration"));
        }
        Ok(())
    }
    async fn persist(&self, state: &State, candidate: &mut Credentials, action: &'static str) -> Result<(), ServingError> {
        candidate.revision = state.value.revision.checked_add(1).ok_or_else(|| failure(action, "revision exhausted"))?;
        self.store
            .commit(state.value.revision, candidate.clone())
            .await
            .map_err(|_| failure(action, "credential store unavailable or revision conflicted"))
    }
    fn publish(&self, state: &mut State, candidate: Credentials) {
        *self.authorized.write().expect("pairing authorization lock is healthy") =
            candidate.pairings.iter().map(|p| (p.id.clone(), p.credential_sha256.clone())).collect();
        state.value = candidate;
        state.issued = None;
    }
    pub async fn shutdown(&self) -> Result<(), ServingError> {
        let mut state = self.state.lock().await;
        state.admission.store(false, Ordering::Release);
        if let Some(attached) = state.attached.take() {
            attached.detach().await.map_err(|_| failure("Access.Shutdown", "listener did not settle"))?;
        }
        Ok(())
    }
    fn endpoint(state: &State) -> String {
        state.attached.as_ref().map_or_else(String::new, |a| format!("https://{}", a.local_addr()))
    }
}
impl harmos_operations::access::Access for Port {
    fn listener_get(&self, _: wire::ListenerGetRequest) -> Operation<'_, wire::ListenerGetResponse> {
        Box::pin(async move {
            let state = self.state.lock().await;
            self.loaded(&state, "Access.Listener.Get").await?;
            Ok(wire::ListenerGetResponse {
                value: Some(state.value.listener.clone()),
                revision: state.value.listener_revision.to_be_bytes().to_vec(),
                bound_endpoint: Access::endpoint(&state),
            })
        })
    }
    fn listener_set(&self, request: wire::ListenerSetRequest) -> Operation<'_, wire::ListenerSetResponse> {
        Box::pin(async move {
            let action = "Access.Listener.Set";
            let value = request.value.ok_or_else(|| failure(action, "whole listener value is required"))?;
            validate(&value, action)?;
            let mut state = self.state.lock().await;
            self.loaded(&state, action).await?;
            if request.expected_revision != state.value.listener_revision.to_be_bytes() {
                return Err(failure(action, "revision conflict: get again and retry"));
            }
            let mut candidate = state.value.clone();
            candidate.listener = value.clone();
            candidate.listener_revision =
                state.value.listener_revision.checked_add(1).ok_or_else(|| failure(action, "listener revision exhausted"))?;
            let rotate = candidate.listener.certificate_names != state.value.listener.certificate_names;
            if rotate {
                let (cert, key) = generate(&candidate.listener.certificate_names, action)?;
                candidate.certificate_pem = cert;
                candidate.private_key_pem = key;
                candidate.pairings.clear();
            }
            let certificate = ServerCertificate::from_pem(candidate.certificate_pem.as_bytes(), candidate.private_key_pem.as_bytes())
                .map_err(|_| failure(action, "invalid certificate"))?;
            let replace = value.enabled && (state.attached.is_none() || value.endpoint != state.value.listener.endpoint);
            let staged = if replace { Some(self.stage(&value, certificate.clone()).await?) } else { None };
            if let Err(error) = self.persist(&state, &mut candidate, action).await {
                if let Some((attached, _)) = staged {
                    attached.detach().await.map_err(|_| failure(action, "failed staged listener cleanup"))?;
                }
                return Err(error);
            }
            if rotate && !replace {
                state
                    .certificate
                    .reload(candidate.certificate_pem.as_bytes(), candidate.private_key_pem.as_bytes())
                    .map_err(|_| failure(action, "validated certificate publication failed"))?;
            }
            self.publish(&mut state, candidate);
            if let Some((attached, admission)) = staged {
                state.admission.store(false, Ordering::Release);
                let old = state.attached.replace(attached);
                state.certificate = certificate;
                state.admission = admission;
                state.admission.store(true, Ordering::Release);
                // Retirement Is Owned by the Listener Manager; Drop Signals Its Graceful Stop.
                drop(old);
            } else if !value.enabled {
                state.admission.store(false, Ordering::Release);
                drop(state.attached.take());
            }
            Ok(wire::ListenerSetResponse {
                value: Some(value),
                revision: state.value.listener_revision.to_be_bytes().to_vec(),
                bound_endpoint: Access::endpoint(&state),
            })
        })
    }
    fn pairings_create(&self, request: wire::PairingsCreateRequest) -> Operation<'_, wire::PairingsCreateResponse> {
        Box::pin(self.create_pairing(request))
    }
    fn pairings_list(&self, request: wire::PairingsListRequest) -> Operation<'_, wire::PairingsListResponse> {
        Box::pin(self.list_pairings(request))
    }
    fn pairings_delete(&self, request: wire::PairingsDeleteRequest) -> Operation<'_, wire::PairingsDeleteResponse> {
        Box::pin(self.delete_pairing(request))
    }
    fn certificates_create(&self, _: wire::CertificatesCreateRequest) -> Operation<'_, wire::CertificatesCreateResponse> {
        Box::pin(async move {
            let action = "Access.Certificates.Create";
            let mut state = self.state.lock().await;
            self.loaded(&state, action).await?;
            let mut candidate = state.value.clone();
            let (cert, key) = generate(&candidate.listener.certificate_names, action)?;
            ServerCertificate::from_pem(cert.as_bytes(), key.as_bytes()).map_err(|_| failure(action, "invalid generated certificate"))?;
            candidate.certificate_pem = cert.clone();
            candidate.private_key_pem = key;
            candidate.pairings.clear();
            self.persist(&state, &mut candidate, action).await?;
            state
                .certificate
                .reload(candidate.certificate_pem.as_bytes(), candidate.private_key_pem.as_bytes())
                .map_err(|_| failure(action, "validated certificate publication failed"))?;
            self.publish(&mut state, candidate);
            Ok(wire::CertificatesCreateResponse { certificate_pem: cert })
        })
    }
}
impl super::middleware::Middleware for Authorization {
    fn enter<'a>(&'a self, call: &'a mut super::middleware::Invocation) -> super::middleware::Enter<'a> {
        Box::pin(async move {
            use sha2::{Digest, Sha256};
            if !self.admission.load(Ordering::Acquire) {
                return Err(crate::Error::Unauthenticated);
            }
            let owner = self.owner.upgrade().ok_or(crate::Error::Unauthenticated)?;
            let credential = call
                .metadata()
                .get("authorization")
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "))
                .ok_or(crate::Error::Unauthenticated)?;
            let digest = Sha256::digest(credential.as_bytes());
            let principal = owner
                .authorized
                .read()
                .expect("pairing authorization lock is healthy")
                .iter()
                .find(|(_, expected)| constant_equal(expected, &digest))
                .map(|(pairing_id, _)| Principal { pairing_id: pairing_id.clone() })
                .ok_or(crate::Error::Unauthenticated)?;
            call.context_mut().insert(principal)
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn validate(value: &wire::ListenerValue, action: &'static str) -> Result<(), ServingError> {
    if value.endpoint.parse::<std::net::SocketAddr>().is_err()
        || value.certificate_names.is_empty()
        || value.certificate_names.iter().any(String::is_empty)
    {
        return Err(failure(action, "valid endpoint and certificate names are required"));
    }
    Ok(())
}
fn generate(names: &[String], action: &'static str) -> Result<(String, String), ServingError> {
    let key = rcgen::generate_simple_self_signed(names.to_vec()).map_err(|_| failure(action, "certificate generation failed"))?;
    Ok((key.cert.pem(), key.signing_key.serialize_pem()))
}
fn constant_equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && left.iter().zip(right).fold(0u8, |difference, (a, b)| difference | (a ^ b)) == 0
}

/* === Tests === */
