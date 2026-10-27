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
use std::future::Future;
use std::sync::Arc;
// Third Party Imports
use futures_util::stream::{FuturesUnordered, StreamExt};
use tokio::sync::{Mutex, watch};
use tokio::task::{JoinError, JoinHandle};
// Local Imports
use super::executor;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Task<T> {
    stopping: watch::Sender<bool>,
    join: Mutex<Join<T>>,
}

/* === Enums === */

enum Join<T> {
    Running(JoinHandle<T>),
    Settled(Result<Arc<T>, Arc<JoinError>>),
}

/* === Traits === */

/* === Implementations === */

impl<T: Send + Sync + 'static> Task<T> {
    pub(crate) fn spawn<F, W>(work: W) -> Self
    where
        W: FnOnce(watch::Sender<bool>) -> F,
        F: Future<Output = T> + Send + 'static,
    {
        Self::with_cancellation(watch::channel(false).0, work)
    }

    pub(crate) fn with_cancellation<F, W>(stopping: watch::Sender<bool>, work: W) -> Self
    where
        W: FnOnce(watch::Sender<bool>) -> F,
        F: Future<Output = T> + Send + 'static,
    {
        let task = executor::task(work(stopping.clone()));
        Self {
            stopping,
            join: Mutex::new(Join::Running(task)),
        }
    }

    pub(crate) fn stop(&self) {
        self.stopping.send_replace(true);
    }

    pub(crate) async fn settle(&self) -> Result<Arc<T>, Arc<JoinError>> {
        self.settle_retry(|| None::<std::future::Ready<T>>).await
    }

    pub(crate) async fn settle_retry<F>(&self, retry: impl FnOnce() -> Option<F>) -> Result<Arc<T>, Arc<JoinError>>
    where
        F: Future<Output = T> + Send + 'static,
    {
        let mut join = self.join.lock().await;
        if matches!(&*join, Join::Settled(_))
            && let Some(future) = retry()
        {
            *join = Join::Running(executor::task(future));
        }
        if let Join::Running(task) = &mut *join {
            let result = task.await.map(Arc::new).map_err(Arc::new);
            *join = Join::Settled(result);
        }
        let Join::Settled(result) = &*join else { unreachable!("task was joined") };
        result.clone()
    }
}

impl<T> Drop for Task<T> {
    fn drop(&mut self) {
        self.stopping.send_replace(true);
    }
}

/* === Public Functions === */

pub(crate) async fn settle_all<E, F>(tasks: impl IntoIterator<Item = F>) -> Result<(), E>
where
    F: Future<Output = Result<(), E>>,
{
    let mut pending: FuturesUnordered<_> = tasks.into_iter().enumerate().map(|(index, task)| async move { (index, task.await) }).collect();
    let mut failure = None;
    while let Some((index, result)) = pending.next().await {
        if let Err(error) = result
            && failure.as_ref().is_none_or(|(first, _)| index < *first)
        {
            failure = Some((index, error));
        }
    }
    failure.map_or(Ok(()), |(_, error)| Err(error))
}

/* === Private Functions === */

/* === Tests === */
