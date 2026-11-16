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
/// Its policy's timeout is the plugin's startup deadline. Attaching, a managed entry's start or replacement, a
/// reconnect and a scoped sample start each run one load under it; [`LoadOptions`] replaces it for one load.
pub struct Load;

/// Per-Call Options of One Plugin Load.
#[derive(Clone, Copy, Debug, Default)]
pub struct LoadOptions {
    /// Replaces the `plugins.load` Policy for This Load Only; Later Loads, Including Reconnects, Use the Policy.
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
        std::mem::size_of::<Self>()
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
    let loaded = match operations.submit_native_with::<Load>(request, options.policy) {
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
