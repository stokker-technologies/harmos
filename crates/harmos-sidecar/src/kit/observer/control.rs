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
use std::collections::BTreeMap;
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::{Instance, ObserverHost, ObserverStatus, ObserverTask};
use crate::{ObserverControl, ObserverUpdate, Refusal, Supervisor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Start<S> = Box<dyn Fn(&S, &Supervisor) -> std::io::Result<ObserverTask> + Send + Sync>;

/* === Structs === */
pub(crate) struct Controlled<S> {
    declarations: BTreeMap<&'static str, Start<S>>,
    generations: BTreeMap<String, Generation>,
    finished: tokio::sync::watch::Sender<bool>,
}

struct Generation {
    number: u64,
    task: Option<ObserverTask>,
    monitor: Option<tokio::task::JoinHandle<()>>,
    stopping: Option<Stopping>,
}

struct Joining {
    observer: ObserverTask,
    monitor: Option<tokio::task::JoinHandle<()>>,
}

struct Stopping {
    original: Arc<tokio::sync::Mutex<Joining>>,
    task: tokio::task::JoinHandle<()>,
    completion: tokio::sync::watch::Receiver<Option<Result<ObserverStatus, String>>>,
}

pub(crate) struct Stopped(tokio::sync::watch::Receiver<Option<Result<ObserverStatus, String>>>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Stopped {
    pub(crate) async fn wait(mut self) -> Result<ObserverStatus, Refusal> {
        loop {
            if let Some(result) = self.0.borrow_and_update().clone() {
                return result.map_err(Refusal::of);
            }
            self.0.changed().await.map_err(Refusal::of)?;
        }
    }
}

impl Stopping {
    fn begin(original: Arc<tokio::sync::Mutex<Joining>>, recover: bool) -> Self {
        let (completed, completion) = tokio::sync::watch::channel(None);
        let retained = original.clone();
        let task = tokio::spawn(async move {
            let mut joining = retained.lock().await;
            let result = if recover {
                joining.observer.recover().await
            } else {
                joining.observer.join().await
            };
            let status = joining.observer.status();
            let result = match (result, status) {
                (_, ObserverStatus::Retained(_)) | (Ok(()), _) => Ok(status),
                (Err(error), _) => Err(error.to_string()),
            };
            if joining.observer.is_finished()
                && let Some(monitor) = joining.monitor.take()
            {
                monitor.abort();
                let _ = monitor.await;
            }
            completed.send_replace(Some(result));
        });
        Self {
            original,
            task,
            completion,
        }
    }
}

impl Generation {
    fn begin(&mut self, recover: bool) -> Stopped {
        if let Some(stopping) = &self.stopping {
            let failed = matches!(
                &*stopping.completion.borrow(),
                Some(Ok(ObserverStatus::Retained(_)))
            );
            if !recover || !failed {
                return Stopped(stopping.completion.clone());
            }
            let original = stopping.original.clone();
            let stopping = Stopping::begin(original, true);
            let receipt = Stopped(stopping.completion.clone());
            self.stopping = Some(stopping);
            return receipt;
        }
        let Some(mut task) = self.task.take() else {
            return Stopped(
                tokio::sync::watch::channel(Some(Ok(ObserverStatus::Stopped(
                    super::ObserverExit::Cancelled,
                ))))
                .1,
            );
        };
        task.cancel();
        let original = Arc::new(tokio::sync::Mutex::new(Joining {
            observer: task,
            monitor: self.monitor.take(),
        }));
        let stopping = Stopping::begin(original, false);
        let receipt = Stopped(stopping.completion.clone());
        self.stopping = Some(stopping);
        receipt
    }
    fn unsettled(&self) -> bool {
        self.task.is_some()
            || self.stopping.as_ref().is_some_and(|stopping| {
                !matches!(
                    &*stopping.completion.borrow(),
                    Some(Ok(ObserverStatus::Stopped(_)))
                )
            })
    }
}

impl Drop for Generation {
    fn drop(&mut self) {
        if let Some(monitor) = &self.monitor {
            monitor.abort();
        }
    }
}

impl<S> Drop for Controlled<S> {
    fn drop(&mut self) {
        let generations = std::mem::take(&mut self.generations);
        if generations
            .values()
            .all(|generation| generation.task.is_none() && generation.stopping.is_none())
        {
            self.finished.send_replace(true);
            return;
        }
        let finished = self.finished.clone();
        tokio::spawn(async move {
            let mut generations = generations;
            for generation in generations.values_mut() {
                generation.begin(false);
            }
            for generation in generations.values_mut() {
                if let Some(stopping) = generation.stopping.as_mut() {
                    let _ = (&mut stopping.task).await;
                }
            }
            let settled = generations
                .values()
                .all(|generation| !generation.unsettled());
            drop(generations);
            if settled {
                finished.send_replace(true);
            }
        });
    }
}

impl<S> Controlled<S> {
    pub(crate) fn start(
        &mut self,
        control: &ObserverControl,
        state: &S,
        host: &Supervisor,
    ) -> Result<(), Refusal> {
        let open = self
            .declarations
            .get(control.observer.as_str())
            .ok_or_else(|| Refusal::of("unknown observer"))?;
        if control.generation == 0 {
            return Err(Refusal::of("zero observer generation"));
        }
        if let Some(current) = self.generations.get(&control.observer) {
            if current.number == control.generation && current.task.is_some() {
                return Ok(());
            }
            if current.number >= control.generation {
                return Err(Refusal::of("observer generation is retired"));
            }
            if current.unsettled() {
                return Err(Refusal::of("previous observer generation has not joined"));
            }
        }
        let task = open(state, host).map_err(Refusal::of)?;
        let mut status = task.subscribe();
        let host = host.clone();
        let control = control.clone();
        let key = control.observer.clone();
        let number = control.generation;
        let monitor = tokio::spawn(async move {
            loop {
                let current = *status.borrow_and_update();
                if !host
                    .observer_update(ObserverUpdate {
                        control: control.clone(),
                        status: current,
                    })
                    .await
                {
                    break;
                }
                if matches!(current, ObserverStatus::Stopped(_)) || status.changed().await.is_err()
                {
                    break;
                }
            }
        });
        self.generations.insert(
            key,
            Generation {
                number,
                task: Some(task),
                monitor: Some(monitor),
                stopping: None,
            },
        );
        Ok(())
    }

    pub(crate) fn begin_stop(&mut self, control: &ObserverControl) -> Result<Stopped, Refusal> {
        if !self.declarations.contains_key(control.observer.as_str()) || control.generation == 0 {
            return Err(Refusal::of("invalid observer control"));
        }
        let current = self
            .generations
            .entry(control.observer.clone())
            .or_insert(Generation {
                number: control.generation,
                task: None,
                monitor: None,
                stopping: None,
            });
        if control.generation < current.number {
            return Ok(Stopped(
                tokio::sync::watch::channel(Some(Ok(ObserverStatus::Stopped(
                    super::ObserverExit::Cancelled,
                ))))
                .1,
            ));
        }
        if control.generation > current.number {
            if current.unsettled() {
                return Err(Refusal::of("another observer generation is live"));
            }
            current.number = control.generation;
        }
        Ok(current.begin(true))
    }

    pub(crate) fn begin_quiesce(&mut self) -> Vec<Stopped> {
        self.generations
            .values_mut()
            .map(|generation| generation.begin(true))
            .collect()
    }
}

impl<S: 'static> Controlled<S> {
    pub(crate) fn of<R: Send + Sync + 'static, H: ObserverHost<R>>(
        instances: Vec<Instance<R, H>>,
        resources: fn(&S) -> Arc<R>,
    ) -> Self {
        let declarations = instances
            .into_iter()
            .map(|instance| {
                let id = instance.id();
                let open: Start<S> = Box::new(move |state, host| {
                    let resources = resources(state);
                    let retained = resources.clone();
                    let host = host.clone();
                    instance.start_with(resources, move || H::bind(retained, host))
                });
                (id, open)
            })
            .collect();
        Self {
            declarations,
            generations: BTreeMap::new(),
            finished: tokio::sync::watch::channel(false).0,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
