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
use std::pin::Pin;
use std::sync::Arc;
// Third Party Imports
use tokio::io::{duplex, split};
// Local Imports
use super::{Opening, SharedOpening, connection};
use crate::Attached;
use crate::attach::{End, Ending};

/* === Constants === */

const BUFFER: usize = 64 * 1024;

/* === Macros === */

/* === Type Aliases === */

type EndWork = Pin<Box<dyn Future<Output = std::io::Result<()>> + Send>>;

/* === Structs === */

struct LocalEnd {
    opening: Option<SharedOpening>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
    work: Option<EndWork>,
    completed: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl End for LocalEnd {
    fn settle(&mut self) -> Pin<Box<dyn Future<Output = std::io::Result<()>> + Send + '_>> {
        Box::pin(async move {
            if self.completed {
                return Ok(());
            }
            if self.work.is_none() {
                drop(self.stop.take());
                let task = self.task.take();
                let opening = Arc::clone(self.opening.as_ref().expect("original opening is retained"));
                self.work = Some(Box::pin(async move {
                    if let Some(task) = task {
                        return task.await.map_err(std::io::Error::other)?;
                    }
                    opening.lock().await.cleanup().await.map_err(std::io::Error::other)
                }));
            }
            let result = self.work.as_mut().expect("original physical ending work is retained").await;
            self.work = None;
            result?;
            self.opening = None;
            self.completed = true;
            Ok(())
        })
    }
}

/* === Public Functions === */

pub(crate) fn binary(opening: Opening) -> Attached {
    let opening = Arc::new(tokio::sync::Mutex::new(opening));
    let (here, there) = duplex(BUFFER);
    let (control_here, control_there) = duplex(256);
    let (read, write) = split(there);
    let (control_read, control_write) = split(control_there);
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let serving = opening.clone();
    let task = tokio::spawn(async move {
        connection::run_binary(serving, (Box::new(read), Box::new(write)), (Box::new(control_read), Box::new(control_write)), async {
            let _ = stopped.await;
        })
        .await
    });
    let ending = Ending::new(LocalEnd {
        opening: Some(opening),
        stop: Some(stop),
        task: Some(task),
        work: None,
        completed: false,
    });
    let (read, write) = split(here);
    let (control_read, control_write) = split(control_here);
    Attached::new(write, read).control((Box::new(control_read), Box::new(control_write))).ending(ending)
}

/* === Private Functions === */

/* === Tests === */
