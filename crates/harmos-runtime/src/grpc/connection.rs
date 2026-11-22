/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Own Connection Identity, Call Settlement and End Notifications.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{future::Future, panic::AssertUnwindSafe, pin::Pin, sync::Arc};

// Third Party Imports
use futures_util::FutureExt;
use tokio::sync::watch;
use tonic::transport::server::TcpConnectInfo;

// Local Imports
use crate::operations::ConnectionId;
use crate::supervision::instance::Instance;
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct ConnectionSession {
    id: ConnectionId,
    closed: watch::Receiver<bool>,
}

#[derive(Clone)]
pub(super) struct Info {
    pub(super) tcp: TcpConnectInfo,
    pub(super) scope: Arc<Scope>,
    pub(super) _call: Arc<Call>,
}

pub(super) struct Scope {
    pub(super) id: ConnectionId,
    closed: watch::Sender<bool>,
    calls: watch::Sender<usize>,
}

pub(super) struct Connections {
    pending: watch::Sender<usize>,
}

pub(super) struct Call {
    pending: watch::Sender<usize>,
}

/* === Enums === */

/* === Traits === */

/// One Setup Observer, Called Once After Each Connection and Its Calls Settle.
/// Each Callback Runs Independently on the Runtime. It Must Yield Instead of
/// Blocking a Runtime Thread. Errors and Panics Are Logged; Shutdown Awaits It.
pub trait ConnectionEnd: Send + Sync + 'static {
    fn ended(&self, connection: ConnectionId) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}

/* === Implementations === */

impl ConnectionSession {
    pub fn id(&self) -> ConnectionId {
        self.id
    }

    // A Latched Transport End, Independent of Retained Handles and RPC Settlement.
    pub async fn ended(&self) {
        let _ = self.closed.clone().wait_for(|closed| *closed).await;
    }
}

impl Default for Connections {
    fn default() -> Self {
        Self { pending: watch::channel(0).0 }
    }
}

impl Connections {
    pub(super) fn open(&self, observer: Option<Arc<dyn ConnectionEnd>>) -> Result<Arc<Scope>> {
        let id = ConnectionId::allocate().ok_or(Error::ResourceExhausted {
            resource: "transport connection identities",
        })?;
        let scope = Arc::new(Scope {
            id,
            closed: watch::channel(false).0,
            calls: watch::channel(0).0,
        });
        let connection = scope.clone();
        self.pending.send_modify(|pending| *pending += 1);
        let pending = Call { pending: self.pending.clone() };
        tokio::spawn(async move {
            let _pending = pending;
            connection.ended().await;
            let _ = connection.calls.subscribe().wait_for(|calls| *calls == 0).await;
            drop(connection);
            if let Some(observer) = observer {
                let outcome = AssertUnwindSafe(async { observer.ended(id).await }).catch_unwind().await;
                match outcome {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => tracing::error!(?id, %error, "release connection resources: observer failed"),
                    Err(_) => tracing::error!(?id, "release connection resources: observer panicked"),
                }
            }
        });
        Ok(scope)
    }

    pub(super) async fn drain(&self) {
        let _ = self.pending.subscribe().wait_for(|pending| *pending == 0).await;
    }
}

impl Scope {
    pub(super) fn session(&self) -> ConnectionSession {
        ConnectionSession {
            id: self.id,
            closed: self.closed.subscribe(),
        }
    }

    pub(super) fn close(&self) {
        self.closed.send_if_modified(|closed| {
            if *closed {
                return false;
            }
            *closed = true;
            true
        });
    }

    pub(super) async fn ended(&self) {
        self.session().ended().await;
    }

    pub(super) fn call(&self) -> Call {
        self.calls.send_modify(|calls| *calls += 1);
        Call { pending: self.calls.clone() }
    }

    pub(super) fn track(self: &Arc<Self>, control: Arc<Instance>) {
        let call = self.call();
        let scope = self.clone();
        tokio::spawn(async move {
            let _call = call;
            let settlement = async {
                if let Err(error) = control.settle().await {
                    tracing::error!(connection = ?scope.id, %error, "settle connection call: operation cleanup remains pending");
                    let _ = control.settlement().wait_for(|status| status.phase().is_terminal()).await;
                }
            };
            tokio::pin!(settlement);
            tokio::select! {
                biased;
                _ = scope.ended() => {
                    control.request_cancel();
                    settlement.await;
                }
                _ = &mut settlement => {}
            }
        });
    }
}

impl Drop for Call {
    fn drop(&mut self) {
        self.pending.send_modify(|pending| *pending -= 1);
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Connections;

    #[tokio::test]
    async fn retained_session_does_not_retain_connection_state() {
        // Retain Only the Public Handle After Closure
        let connections = Connections::default();
        let scope = connections.open(None).unwrap();
        let retained = scope.session();
        let released = Arc::downgrade(&scope);
        assert!(futures_util::poll!(Box::pin(retained.ended())).is_pending());
        scope.close();
        scope.close();
        drop(scope);

        // Drain Internal Tasks Without Dropping the Public Handle
        connections.drain().await;
        assert!(released.upgrade().is_none());
        retained.ended().await;
        retained.clone().ended().await;
    }
}
