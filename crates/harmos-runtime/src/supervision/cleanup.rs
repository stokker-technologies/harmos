/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinHandle;

use super::executor;
use crate::Error;

pub(crate) type Outcome = Result<(), Arc<Error>>;
pub(crate) type Finalize<'a> = Pin<Box<dyn Future<Output = Outcome> + Send + 'a>>;

pub(crate) struct Cleanup<R: Resource> {
    original: Arc<Mutex<Option<R>>>,
    joining: AsyncMutex<Join>,
}

pub(crate) struct Owned<R> {
    resource: Option<R>,
    original: Arc<Mutex<Option<R>>>,
}

enum Join {
    Ready,
    Running(JoinHandle<Outcome>),
    Settled,
}

pub(crate) trait Resource: Send + 'static {
    fn finalize(&mut self) -> Finalize<'_>;
}

impl Resource for Box<dyn Resource> {
    fn finalize(&mut self) -> Finalize<'_> {
        self.as_mut().finalize()
    }
}

impl<R: Resource> Cleanup<R> {
    pub(crate) fn new(resource: R) -> Self {
        Self {
            original: Arc::new(Mutex::new(Some(resource))),
            joining: AsyncMutex::new(Join::Ready),
        }
    }

    pub(crate) fn take(&self) -> Option<Owned<R>> {
        let resource = self.original.lock().expect("cleanup owner is healthy").take()?;
        Some(Owned {
            resource: Some(resource),
            original: Arc::clone(&self.original),
        })
    }

    pub(crate) async fn settle(&self) -> Outcome {
        self.finish(true).await
    }

    pub(crate) async fn release(&self) -> Outcome {
        self.finish(false).await
    }

    async fn finish(&self, finalize: bool) -> Outcome {
        let mut joining = self.joining.lock().await;
        if matches!(*joining, Join::Settled) {
            return Ok(());
        }
        if matches!(*joining, Join::Ready) {
            let mut original = self.take().ok_or_else(|| Arc::new(Error::NotReady { domain: "instance cleanup owner" }))?;
            let future = async move {
                if finalize {
                    original.resource().finalize().await?;
                }
                original.release();
                Ok(())
            };
            *joining = Join::Running(executor::task(future));
        }
        let Join::Running(task) = &mut *joining else { unreachable!("cleanup owns its join before waiting") };
        let result = task.await.unwrap_or_else(|source| {
            Err(Arc::new(Error::TaskFailed {
                domain: "instance cleanup",
                source: Arc::new(source),
            }))
        });
        if result.is_err() {
            *joining = Join::Ready;
            return result;
        }

        *joining = Join::Settled;
        Ok(())
    }
}

impl<R> Owned<R> {
    fn release(mut self) {
        drop(self.resource.take());
    }

    pub(crate) fn resource(&mut self) -> &mut R {
        self.resource.as_mut().expect("original resource is owned")
    }
}

impl<R> Drop for Owned<R> {
    fn drop(&mut self) {
        let Some(resource) = self.resource.take() else {
            return;
        };
        let mut original = self.original.lock().expect("cleanup owner is healthy");
        assert!(original.is_none(), "physical resources have one owner");
        *original = Some(resource);
    }
}
