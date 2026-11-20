/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The Operation That Loads and Initializes a Plugin and Owns Its Startup Deadline

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    future::Future,
    mem,
    sync::{Arc, Mutex},
    time::Duration,
};

// Third Party Imports
use futures_util::{FutureExt, future::BoxFuture};

// Local Imports
use super::Error;
use crate::operations::{Manager, OperationPolicy};
use harmos_domain::MemorySize;
use harmos_operations::execution::operations::{ExecutionContext, NativeOperation, Resource, ServingError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Loads and Initializes One Plugin: Start or Reach It, Describe It, Admit Its Operations and Open Its Demands.
///
/// Its policy's timeout is the plugin's startup deadline. `GrpcPluginSettings::policy.load` configures it, defaulting
/// to [`NativeOperation::POLICY`]. Attaching, a managed entry's start or replacement, a reconnect and a scoped sample
/// start each run one load under it; [`LoadOptions`] replaces it for one direct load.
pub struct Load;

/// Per-Call Options of One Plugin Load.
#[derive(Clone, Copy, Debug, Default)]
pub struct LoadOptions {
    /// Replaces the Configured `plugins.load` Policy for This Load Only; Later Loads, Reconnects Included, Use It.
    pub policy: Option<OperationPolicy>,
}

/// One Load's Work, Taken Once by the Operation's Single Attempt; Only the Runtime Builds It.
#[derive(Clone)]
pub struct Request {
    work: Arc<Mutex<Option<BoxFuture<'static, ()>>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for Request {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
    }
}

impl NativeOperation for Load {
    type Request = Request;
    type Response = ();
    type Endpoints = ();
    const ID: &'static str = "plugins.load";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_secs(30),
        cancellation_timeout: Duration::ZERO,
        retries: crate::RetryPolicy::Never,
    };
    const RESOURCES: &'static [Resource] = &[];
    async fn run(context: ExecutionContext, request: Request, _: &()) -> Result<(), ServingError> {
        let work = request.work.lock().expect("plugin load lock is healthy").take();
        let Some(work) = work else {
            return Err(ServingError::Native(Arc::new(Error::Invalid("Load Plugin: the load already ran; load the plugin again".into()))));
        };
        // Cancellation and the Deadline End the Work Where It Stands; Its Owner Settles What It Started.
        tokio::select! {
            biased;
            () = context.cancellation.cancelled() => Err(ServingError::Native(Arc::new(Error::Invalid("Load Plugin: cancelled".into())))),
            () = work => Ok(()),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/// Run `work` as One Load of `plugin`; the Operation's Policy, or `options` for This Call, Bounds It.
///
/// Work the deadline or a cancellation ends is dropped where it stands, so it keeps anything its owner must settle
/// afterwards, such as a spawned process, outside itself.
pub(super) async fn load<T: Send + 'static>(
    operations: &Manager, plugin: &str, options: LoadOptions, work: impl Future<Output = Result<T, Error>> + Send + 'static,
) -> Result<T, Error> {
    let outcome = Arc::new(Mutex::new(None));
    let written = outcome.clone();
    let work = async move {
        let result = work.await;
        *written.lock().expect("plugin load outcome lock is healthy") = Some(result);
    };
    let request = Request {
        work: Arc::new(Mutex::new(Some(work.boxed()))),
    };
    // A Direct Caller's Override Replaces the Configured Policy for This Load Only.
    let policy = match options.policy {
        Some(policy) => (policy, "per-call policy override"),
        None => (operations.registry.plugin_load, "GrpcPluginSettings.policy.load"),
    };
    let loaded = match operations.submit_native_with::<Load>(request, policy) {
        Ok(invocation) => invocation.await.map(|_| ()),
        Err(error) => Err(error),
    };
    let finished = outcome.lock().expect("plugin load outcome lock is healthy").take();
    match (loaded, finished) {
        (Ok(()), Some(result)) => result,
        (Err(crate::Error::OperationRefused(crate::Refusal::TimedOut { timeout, configured_by, .. })), _) => Err(Error::LoadDeadline {
            plugin: plugin.to_owned(),
            timeout,
            configured_by,
        }),
        (Err(error), _) => Err(Error::Invalid(format!("Load Plugin {plugin}: {error}; load the plugin again"))),
        (Ok(()), None) => Err(Error::Invalid(format!("Load Plugin {plugin}: the load ended without a result; load the plugin again"))),
    }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use std::{io, sync::Arc, time::SystemTime};

    use tokio::sync;

    use super::{Error, LoadOptions, load};
    use crate::{Runtime, clock::Clock, clock::tests::Model};

    #[tokio::test]
    async fn failed_readiness_settles_load_and_releases_its_process_owner_without_advancing_time() {
        // Start With a Clock That Cannot Consume the Readiness Deadline.
        let (clock, _controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
        let runtime = Runtime::<Model>::start_with((), (), |setup| {
            setup.settings().clock(clock.clone());
        })
        .await
        .unwrap();
        let origin = clock.now();
        let process = Arc::new(sync::Mutex::new(()));
        let held = process.clone();
        let (entered, entering) = sync::oneshot::channel();
        let (fail, failing) = sync::oneshot::channel();
        let operations = runtime.operations.clone();

        // Fail a Readiness Read While Staging Owns the Process Lock.
        let loading = tokio::spawn(async move {
            let work = async move {
                let _process = held.lock().await;
                entered.send(()).unwrap();
                failing.await.unwrap();
                Err::<(), _>(Error::Io(io::ErrorKind::UnexpectedEof.into()))
            };
            load(&operations, "managed", LoadOptions::default(), work).await
        });
        entering.await.unwrap();
        assert!(process.try_lock().is_err());
        fail.send(()).unwrap();

        // Return the Readiness Failure and Its Owner Before Any Deadline Elapses.
        let result = loading.await.unwrap();
        assert!(matches!(result, Err(Error::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof));
        assert!(process.try_lock().is_ok());
        assert_eq!(clock.now(), origin);
        runtime.stop().await.unwrap();
    }
}
