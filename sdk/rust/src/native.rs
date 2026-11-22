/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Plugin Serving

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    env, mem,
    net::SocketAddr,
    result,
    sync::{Arc, OnceLock},
    thread,
};

// Third Party Imports
use tokio::{
    net::TcpListener,
    runtime::Builder,
    sync::{oneshot, watch},
    task::LocalSet,
};
use tonic::Status;

// Local Imports
use crate::{Context, Error, Plugin, Result, server};
use harmos_protocol::plugin::ChildResponse;
use harmos_protocol::plugin::child_response;
use harmos_protocol::plugin::{LaunchesAnnounceRequest, launches_client::LaunchesClient};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Owns a Plugin's Transport and Runtime. Call `close` to Await Graceful Settlement.
/// Dropping the Handle Requests the Same Shutdown Without Waiting for It.
#[must_use = "close the listener to await plugin shutdown"]
pub struct Listener {
    address: SocketAddr,
    shutdown: watch::Sender<bool>,
    finished: Option<oneshot::Receiver<Result<()>>>,
    worker: Option<thread::JoinHandle<()>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Listener {
    /// The Concrete Address Selected When Binding, Including a Port Chosen for Port Zero.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Run Stop Hooks Once, Drain Accepted RPCs, and Release the Owned Runtime.
    /// Repeated Calls Are No-Ops. Cancellation of This Future Still Requests Shutdown.
    pub async fn close(&mut self) -> Result<()> {
        self.shutdown.send_replace(true);
        self.wait().await
    }

    /// Await Transport Shutdown Without Requesting It.
    pub async fn wait(&mut self) -> Result<()> {
        let Some(finished) = self.finished.as_mut() else {
            return Ok(());
        };
        let result = finished.await;
        self.finished = None;
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| Error::Invalid("Close Plugin: Inspect the Panicked Listener Thread".into()))?;
        }
        result.map_err(|_| Error::Invalid("Close Plugin: Inspect the Interrupted Listener Thread".into()))?
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.shutdown.send_replace(true);
    }
}

impl Plugin {
    /// Start an Addressable Listener on Its Own Runtime; No Caller LocalSet Is Required.
    pub async fn start(mut self, address: impl Into<String>) -> Result<Listener> {
        let declarations = self.declarations()?;
        let cleanup = Arc::new(server::Cleanup {
            hooks: mem::take(&mut self.cleanup),
            result: OnceLock::new(),
        });
        let address = address.into();
        let socket = TcpListener::bind(&address)
            .await
            .map_err(|error| Error::Invalid(format!("Start Plugin: Choose an Available Listening Address {address}: {error}")))?;
        let address = socket.local_addr()?;
        let socket = socket.into_std()?;
        let (shutdown, stopped) = watch::channel(false);
        let signal = shutdown.clone();
        let (ready, started) = oneshot::channel();
        let (complete, finished) = oneshot::channel();
        let worker = thread::Builder::new().name("harmos-plugin".into()).spawn(move || {
            let result = (|| {
                let runtime = Builder::new_current_thread().enable_all().build()?;
                runtime.block_on(LocalSet::new().run_until(server::run(
                    self,
                    declarations,
                    socket,
                    signal,
                    stopped,
                    ready,
                    cleanup.clone(),
                )))
            })();
            let cleaned = cleanup.run();
            let _ = complete.send(result.and(cleaned));
        })?;
        let mut listener = Listener {
            address,
            shutdown,
            finished: Some(finished),
            worker: Some(worker),
        };
        if started.await.is_err() {
            listener.close().await?;
            return Err(Error::Invalid("Start Plugin: Inspect the Listener Startup Failure".into()));
        }
        Ok(listener)
    }

    /// Serve on `127.0.0.1:0`; with launch credentials, announce the bound address to the host once.
    pub fn run(self) -> Result<()> {
        let launch = launch()?;
        let runtime = Builder::new_current_thread().enable_all().build()?;
        runtime.block_on(async move {
            let mut listener = self.start("127.0.0.1:0").await?;
            if let Some((endpoint, token)) = launch {
                let address = listener.address().to_string();
                LaunchesClient::connect(format!("http://{endpoint}"))
                    .await
                    .map_err(|error| Error::Invalid(format!("Announce Plugin Launch: Check HARMOS_HOST_ENDPOINT {endpoint}: {error}")))?
                    .announce(LaunchesAnnounceRequest { token, address })
                    .await
                    .map_err(|status| Error::Invalid(format!("Announce Plugin Launch: {}", status.message())))?;
            }
            listener.wait().await
        })
    }
}

impl Context {
    pub(crate) async fn reply(&self, child: ChildResponse) -> result::Result<(), Status> {
        let watch = {
            let mut calls = self.calls.lock().expect("Child Calls Poisoned");
            if matches!(&child.result, Some(child_response::Result::Item(_))) {
                calls.watches.get(&child.call).cloned()
            } else {
                calls.watches.remove(&child.call)
            }
        };
        if let Some(watch) = watch {
            if matches!(&child.result, Some(child_response::Result::Item(bytes) | child_response::Result::Payload(bytes)) if bytes.len() > watch.limit)
            {
                return Err(Status::resource_exhausted("Watch: Item Exceeds Consumer Limit"));
            }
            let item = match child.result {
                Some(child_response::Result::Item(bytes)) => Ok(Some(bytes)),
                Some(child_response::Result::Payload(_)) => Ok(None),
                Some(child_response::Result::Failure(failure)) => Err(failure.into()),
                None => return Err(Status::invalid_argument("Watch: Child Result Missing")),
            };
            let _ = watch.sender.send(item).await;
            return Ok(());
        }
        if self.calls.lock().expect("Child Calls Poisoned").cancelled.contains(&child.call) {
            return Ok(());
        }
        let result = match child.result {
            Some(child_response::Result::Payload(payload)) => Ok(payload),
            Some(child_response::Result::Failure(failure)) => Err(failure.into()),
            Some(child_response::Result::Item(_)) | None => return Err(Status::invalid_argument("Unary Child Result Missing")),
        };
        let send = self
            .calls
            .lock()
            .expect("Child Calls Poisoned")
            .pending
            .remove(&child.call)
            .ok_or_else(|| Status::invalid_argument("Unknown or Duplicate Child"))?;
        let _ = send.sender.send(result);
        Ok(())
    }

    pub(crate) async fn cancel(&self, reason: String) -> Result<()> {
        self.cancellation.send_replace(Some(reason.clone()));
        self.cancel_calls(&reason).await?;
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn launch() -> Result<Option<(String, String)>> {
    match (env::var("HARMOS_HOST_ENDPOINT").ok(), env::var("HARMOS_LAUNCH_TOKEN").ok()) {
        (Some(endpoint), Some(token)) => Ok(Some((endpoint, token))),
        (None, None) => Ok(None),
        (Some(_), None) => Err(Error::Invalid("Start Plugin: Set HARMOS_LAUNCH_TOKEN Beside HARMOS_HOST_ENDPOINT".into())),
        (None, Some(_)) => Err(Error::Invalid("Start Plugin: Set HARMOS_HOST_ENDPOINT Beside HARMOS_LAUNCH_TOKEN".into())),
    }
}

/* === Tests === */
