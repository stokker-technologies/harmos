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
use std::error::Error as StdError;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports
use tokio::sync::watch;
use tokio::task::JoinHandle;
// Local Imports
use super::session::Session;
use super::{Active, Error};
use crate::{
    Application,
    plugin::{DispatchError, PluginEndpoint},
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Outcome = Result<(), Arc<dyn StdError + Send + Sync>>;

/* === Structs === */
pub struct Serving<A: Application> {
    instance: Option<PluginEndpoint>,
    session: Option<Session<A>>,
    publications: Vec<Arc<crate::streams::PublicationLease>>,
    revocation: Option<watch::Sender<bool>>,
    finalization: Option<Finalization<A>>,
}

struct Cleanup<A: Application> {
    instance: Option<PluginEndpoint>,
    session: Option<Session<A>>,
    publications: Vec<Arc<crate::streams::PublicationLease>>,
    started: tokio::time::Instant,
    within: Duration,
}

/* === Enums === */
enum Finalization<A: Application> {
    Pending(JoinHandle<(Cleanup<A>, Outcome)>),
    Failed(Box<Cleanup<A>>, Arc<dyn StdError + Send + Sync>),
    Complete,
}

/* === Traits === */

/* === Implementations === */
impl<A: Application> Active<A> {
    pub fn serve(mut self) -> Result<Serving<A>, Error> {
        if !self.store.data().active {
            return Err(Error::Capability {
                code: "revoked",
                message: "serve".to_owned(),
            });
        }
        let instance = self.routing.take();
        let publications = self.store.data().publications.values().cloned().collect();
        let session = Session::start(self);
        let revocation = instance.as_ref().map(PluginEndpoint::revocation);
        Ok(Serving {
            instance,
            session: Some(session),
            publications,
            revocation,
            finalization: None,
        })
    }
}

impl<A: Application> Serving<A> {
    pub fn scope(&self) -> Option<&crate::plugin::PluginScope> {
        self.instance.as_ref().and_then(PluginEndpoint::scope)
    }

    pub fn operations(&self) -> Result<crate::operations::Component, Error> {
        let session = self.session.as_ref().ok_or(DispatchError::Unavailable)?;
        let endpoint = session.endpoint();
        if !endpoint.accepts() {
            return Err(DispatchError::Unavailable.into());
        }
        Ok(crate::operations::Component::new(endpoint))
    }

    pub(crate) fn exit(
        &self,
    ) -> Result<impl Future<Output = Result<(), Error>> + Send + 'static, Error> {
        let endpoint = self
            .session
            .as_ref()
            .ok_or(DispatchError::Unavailable)?
            .endpoint();
        Ok(async move { endpoint.settle().await })
    }

    pub async fn finalize(&mut self, within: Duration) -> crate::Result<()> {
        let cleanup = match self.finalization.take() {
            None => {
                if let Some(instance) = &mut self.instance {
                    instance.quiesce();
                }
                Some(Cleanup {
                    instance: self.instance.take(),
                    session: self.session.take(),
                    publications: self.publications.clone(),
                    started: crate::supervision::executor::now(),
                    within,
                })
            }
            Some(Finalization::Failed(cleanup, _)) => Some(*cleanup),
            retained => {
                self.finalization = retained;
                None
            }
        };
        if let Some(mut cleanup) = cleanup {
            self.finalization = Some(Finalization::Pending(crate::supervision::executor::task(
                async move {
                    let remaining = cleanup.within.saturating_sub(
                        crate::supervision::executor::now().duration_since(cleanup.started),
                    );
                    let result = {
                        let finalizing = cleanup.finalize();
                        let mut finalizing = std::pin::pin!(finalizing);
                        let guarded =
                            std::future::poll_fn(|context| {
                                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    finalizing.as_mut().poll(context)
                                })) {
                                    Ok(poll) => poll,
                                    Err(_) => std::task::Poll::Ready(Err(Error::Guest {
                                        message: "guest cleanup machinery panicked".to_owned(),
                                    })),
                                }
                            });
                        let mut guarded = std::pin::pin!(guarded);
                        match crate::supervision::executor::timeout(remaining, guarded.as_mut())
                            .await
                        {
                            Ok(result) => result,
                            Err(_) => {
                                tracing::warn!(
                                    "guest finalization is still draining after its grace window"
                                );
                                guarded.await
                            }
                        }
                    };
                    let outcome =
                        result.map_err(|error| Arc::new(error) as Arc<dyn StdError + Send + Sync>);
                    (cleanup, outcome)
                },
            )));
        }
        if let Some(Finalization::Pending(task)) = &mut self.finalization {
            let (cleanup, outcome) = task.await.map_err(|source| crate::Error::TaskFailed {
                domain: "guest finalization",
                source: Arc::new(source),
            })?;
            self.finalization = Some(match outcome {
                Ok(()) => {
                    drop(cleanup);
                    Finalization::Complete
                }
                Err(source) => Finalization::Failed(Box::new(cleanup), source),
            });
        }
        match &self.finalization {
            Some(Finalization::Complete) => Ok(()),
            Some(Finalization::Failed(_, source)) => Err(crate::Error::TaskFailed {
                domain: "guest finalization",
                source: Arc::clone(source),
            }),
            _ => unreachable!("the original ending receipt was awaited"),
        }
    }
}

impl<A: Application> Cleanup<A> {
    async fn finalize(&mut self) -> Result<(), Error> {
        if let Some(instance) = &mut self.instance {
            instance.quiesce();
        }
        let session = self.session.as_mut().ok_or(DispatchError::Unavailable)?;
        session.close(self.instance.take()).await
    }
}

impl<A: Application> Drop for Serving<A> {
    fn drop(&mut self) {
        if let Some(revocation) = &self.revocation {
            revocation.send_replace(false);
        }
        for lease in &self.publications {
            lease.revoke();
        }
    }
}

impl<A: Application> Drop for Cleanup<A> {
    fn drop(&mut self) {
        for lease in &self.publications {
            lease.revoke();
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
