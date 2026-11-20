/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Launch Announcements on the Loopback Bootstrap Listener

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::HashMap,
    fmt,
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex, Weak},
};

// Third Party Imports
use sha2::{Digest, Sha256};
use tokio::{
    net::TcpListener,
    sync::{self, oneshot, watch},
    task::JoinHandle,
};
use tonic::{Request, Response, Status};

// Local Imports
use super::Error;
use harmos_protocol::plugin::launches_server::LaunchesServer;
use harmos_protocol::plugin::{LaunchesAnnounceRequest, LaunchesAnnounceResponse};

/* === Constants === */

/// Names the Bootstrap Listener a Started Plugin Announces To, as a Bare `127.0.0.1:PORT` Socket Address.
pub const HOST_ENDPOINT: &str = "HARMOS_HOST_ENDPOINT";

/// Carries One Launch Attempt's Single-Use Credential: 64 Lowercase Hexadecimal Characters.
pub const LAUNCH_TOKEN: &str = "HARMOS_LAUNCH_TOKEN";

/* === Macros === */

/* === Type Aliases === */

type Announcement = Result<SocketAddr, String>;

/* === Structs === */

/// Owns the Loopback Bootstrap Listener and the Pending Launches Waiting for Their Announcement.
///
/// The listener binds on the first reservation, serves only `harmos.plugin.v1.Launches/Announce`
/// and stops with [`Launches::shutdown`] or when the last owner is dropped. Each [`Launch`] holds one
/// single-use credential; dropping it revokes the credential, so a launch attempt's credential lives
/// exactly as long as the attempt.
#[derive(Clone, Default)]
pub struct Launches {
    inner: Arc<Inner>,
}

/// One Launch Attempt's Credential and the Address Its Plugin Announces.
pub struct Launch {
    token: String,
    key: [u8; 32],
    endpoint: SocketAddr,
    announced: oneshot::Receiver<Announcement>,
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    pending: Mutex<HashMap<[u8; 32], Pending>>,
    listening: sync::Mutex<Listening>,
}

struct Pending {
    owner: String,
    announced: oneshot::Sender<Announcement>,
}

struct Service(Weak<Inner>);

/* === Enums === */

#[derive(Default)]
enum Listening {
    #[default]
    Idle,
    Serving {
        endpoint: SocketAddr,
        stop: watch::Sender<bool>,
        task: JoinHandle<()>,
    },
    Closed,
}

/* === Traits === */

/* === Implementations === */

impl Launches {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserve One Launch Attempt for `owner`, Binding the Bootstrap Listener on First Use.
    pub async fn reserve(&self, owner: impl Into<String>) -> Result<Launch, Error> {
        let endpoint = self.listen().await?;
        let mut secret = [0u8; 32];
        getrandom::fill(&mut secret).map_err(|error| {
            Error::Invalid(format!("Reserve Plugin Launch: the system random source failed ({error}); start the plugin again"))
        })?;
        let token = secret.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        let key = Sha256::digest(token.as_bytes()).into();
        let (sender, announced) = oneshot::channel();
        self.inner.pending().insert(key, Pending { owner: owner.into(), announced: sender });
        Ok(Launch {
            token,
            key,
            endpoint,
            announced,
            inner: self.inner.clone(),
        })
    }

    /// Revoke Every Pending Credential and Release the Listener; Later Reservations Are Refused.
    pub async fn shutdown(&self) {
        let previous = std::mem::replace(&mut *self.inner.listening.lock().await, Listening::Closed);
        self.inner.pending().clear();
        if let Listening::Serving { stop, task, .. } = previous {
            stop.send_replace(true);
            let _ = task.await;
        }
    }

    async fn listen(&self) -> Result<SocketAddr, Error> {
        let mut listening = self.inner.listening.lock().await;
        match &*listening {
            Listening::Serving { endpoint, .. } => return Ok(*endpoint),
            Listening::Closed => {
                return Err(Error::Invalid("Reserve Plugin Launch: the host is stopping; start the plugin after it restarts".into()));
            }
            Listening::Idle => {}
        }
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.map_err(|error| {
            Error::Invalid(format!("Bind Plugin Launch Listener: {error}; free a loopback port and start the plugin again"))
        })?;
        let endpoint = listener.local_addr()?;
        let (stop, mut stopping) = watch::channel(false);
        let incoming = futures_util::stream::unfold(listener, |listener| async {
            let accepted = listener.accept().await.map(|(socket, _)| socket);
            Some((accepted, listener))
        });
        let service = LaunchesServer::new(Service(Arc::downgrade(&self.inner)));
        let task = tokio::spawn(async move {
            let shutdown = async move {
                let _ = stopping.wait_for(|stopped| *stopped).await;
            };
            if let Err(error) =
                tonic::transport::Server::builder().add_service(service).serve_with_incoming_shutdown(incoming, shutdown).await
            {
                tracing::error!(%endpoint, %error, "plugin launch listener failed; managed plugins cannot announce until the host restarts");
            }
        });
        tracing::debug!(%endpoint, "plugin launch listener ready");
        *listening = Listening::Serving { endpoint, stop, task };
        Ok(endpoint)
    }
}

impl Launch {
    /// The Two Variables the Started Process Receives; Never Log or Persist Them.
    pub fn environment(&self) -> [(&'static str, String); 2] {
        [(HOST_ENDPOINT, self.endpoint.to_string()), (LAUNCH_TOKEN, self.token.clone())]
    }

    pub fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    /// Await the Announced Listener Address; Cancel-Safe, and the Credential Stays Valid Until Drop.
    pub async fn announced(&mut self) -> Result<SocketAddr, Error> {
        match (&mut self.announced).await {
            Ok(Ok(address)) => Ok(address),
            Ok(Err(reason)) => Err(Error::Invalid(reason)),
            Err(_) => Err(Error::Invalid(
                "Await Plugin Announcement: the host stopped accepting launch announcements; start the plugin after it restarts".into(),
            )),
        }
    }
}

impl fmt::Debug for Launch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Launch").field("endpoint", &self.endpoint).finish_non_exhaustive()
    }
}

impl Drop for Launch {
    fn drop(&mut self) {
        self.inner.pending().remove(&self.key);
    }
}

impl Inner {
    fn pending(&self) -> std::sync::MutexGuard<'_, HashMap<[u8; 32], Pending>> {
        self.pending.lock().expect("plugin launch lock is healthy")
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Listening::Serving { stop, task, .. } = self.listening.get_mut() {
            stop.send_replace(true);
            task.abort();
        }
    }
}

#[tonic::async_trait]
impl harmos_protocol::plugin::launches_server::Launches for Service {
    async fn announce(&self, request: Request<LaunchesAnnounceRequest>) -> Result<Response<LaunchesAnnounceResponse>, Status> {
        let request = request.into_inner();
        let refused =
            || Status::unauthenticated("Announce Plugin: the launch credential is invalid or spent; restart the plugin through the host");
        let inner = self.0.upgrade().ok_or_else(refused)?;
        let key: [u8; 32] = Sha256::digest(request.token.as_bytes()).into();
        // The First Authenticated Announcement Consumes the Credential, Whatever Address It Carries.
        let pending = inner.pending().remove(&key).ok_or_else(refused)?;
        match loopback(&request.address) {
            Ok(address) => {
                tracing::debug!(plugin = %pending.owner, %address, "managed plugin announced its listener");
                let _ = pending.announced.send(Ok(address));
                Ok(Response::new(LaunchesAnnounceResponse {}))
            }
            Err(reason) => {
                let _ = pending.announced.send(Err(format!(
                    "Announce Plugin: the plugin announced `{}`, which is {reason}; announce a bound loopback socket, then restart the plugin",
                    request.address
                )));
                Err(Status::invalid_argument(format!(
                    "Announce Plugin: `{}` is {reason}; announce a bound loopback socket and restart through the host",
                    request.address
                )))
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn loopback(address: &str) -> Result<SocketAddr, &'static str> {
    let socket: SocketAddr = address.parse().map_err(|_| "not a socket address")?;
    if !socket.ip().is_loopback() {
        return Err("not a loopback address");
    }
    if socket.port() == 0 {
        return Err("not bound to a port");
    }
    Ok(socket)
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use harmos_protocol::plugin::{LaunchesAnnounceRequest, launches_client::LaunchesClient};

    use super::{Launch, Launches};

    async fn announce(launch: &Launch, token: &str, address: &str) -> Result<(), tonic::Status> {
        let mut client = LaunchesClient::connect(format!("http://{}", launch.endpoint())).await.unwrap();
        client
            .announce(LaunchesAnnounceRequest {
                token: token.into(),
                address: address.into(),
            })
            .await
            .map(|_| ())
    }

    fn token(launch: &Launch) -> String {
        launch.environment()[1].1.clone()
    }

    #[tokio::test]
    async fn an_announcement_wakes_exactly_its_launch() {
        // Two Launches Wait at Once on One Listener.
        let launches = Launches::new();
        let mut first = launches.reserve("first").await.unwrap();
        let mut second = launches.reserve("second").await.unwrap();
        assert_eq!(first.endpoint(), second.endpoint());
        assert_ne!(token(&first), token(&second));
        assert_eq!(token(&first).len(), 64);

        // The Second Credential Delivers Only to the Second Launch.
        announce(&first, &token(&second), "127.0.0.1:4002").await.unwrap();
        assert_eq!(second.announced().await.unwrap(), "127.0.0.1:4002".parse().unwrap());
        assert!(futures_util::poll!(std::pin::pin!(first.announced())).is_pending(), "Leave the Other Launch Waiting");
        announce(&first, &token(&first), "[::1]:4001").await.unwrap();
        assert_eq!(first.announced().await.unwrap(), "[::1]:4001".parse().unwrap());
        launches.shutdown().await;
    }

    #[tokio::test]
    async fn wrong_replayed_and_retired_credentials_are_refused() {
        let launches = Launches::new();
        let mut launch = launches.reserve("plugin").await.unwrap();
        let spent = token(&launch);

        // A Credential Nobody Reserved Is Refused.
        let wrong = announce(&launch, &"0".repeat(64), "127.0.0.1:4001").await.unwrap_err();
        assert_eq!(wrong.code(), tonic::Code::Unauthenticated);

        // The First Announcement Spends the Credential; Its Replay Is Refused.
        announce(&launch, &spent, "127.0.0.1:4001").await.unwrap();
        let replayed = announce(&launch, &spent, "127.0.0.1:4001").await.unwrap_err();
        assert_eq!(replayed.code(), tonic::Code::Unauthenticated);
        launch.announced().await.unwrap();

        // A Replaced Attempt's Dropped Launch Retires Its Credential Before It Was Used.
        let replaced = launches.reserve("plugin").await.unwrap();
        let retired = token(&replaced);
        drop(replaced);
        let refused = announce(&launch, &retired, "127.0.0.1:4001").await.unwrap_err();
        assert_eq!(refused.code(), tonic::Code::Unauthenticated);
        launches.shutdown().await;
    }

    #[tokio::test]
    async fn an_address_that_is_not_a_bound_loopback_socket_fails_its_launch() {
        for address in ["listening", "192.0.2.1:4001", "127.0.0.1:0"] {
            // An Authenticated Announcement With an Unusable Address Spends the Credential and Fails the Launch.
            let launches = Launches::new();
            let mut launch = launches.reserve("plugin").await.unwrap();
            let refused = announce(&launch, &token(&launch), address).await.unwrap_err();
            assert_eq!(refused.code(), tonic::Code::InvalidArgument, "{address}");
            assert!(launch.announced().await.is_err(), "{address}");
            let replayed = announce(&launch, &token(&launch), "127.0.0.1:4001").await.unwrap_err();
            assert_eq!(replayed.code(), tonic::Code::Unauthenticated, "{address}");
            launches.shutdown().await;
        }
    }

    #[tokio::test]
    async fn shutdown_revokes_pending_launches_and_refuses_new_ones() {
        let launches = Launches::new();
        let mut launch = launches.reserve("plugin").await.unwrap();
        launches.shutdown().await;
        assert!(launch.announced().await.is_err(), "End the Pending Launch");
        assert!(LaunchesClient::connect(format!("http://{}", launch.endpoint())).await.is_err(), "Release the Listener");
        assert!(launches.reserve("plugin").await.is_err(), "Refuse a Launch After Shutdown");
    }
}
