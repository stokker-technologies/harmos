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
use std::io;
use std::pin::Pin;
// Third Party Imports
use tokio::task::JoinHandle;
// Local Imports
use super::Arrival;
use crate::supervision::executor;
use harmos_sidecar::attach::{End, Ending};
use harmos_sidecar::{Frame, transport};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct DrivenEnd {
    sender: transport::Sender,
    task: Option<JoinHandle<io::Result<()>>>,
    original: Option<Ending>,
    failure: Option<String>,
}

/* === Enums === */
pub(super) enum Connecting {
    Unavailable,
    Binary(transport::Sender, transport::Receiver),
}

pub(super) struct Sender(transport::Sender);
pub(super) struct Receiver(transport::Receiver);

/* === Traits === */

/* === Implementations === */
impl Connecting {
    pub fn new(data: transport::Io, control: Option<transport::Io>, ending: Option<Ending>) -> (Self, Option<Ending>) {
        let Some(control) = control else {
            return (Self::Unavailable, ending);
        };
        let transport::Connection { sender, receiver, mut driver } = transport::Connection::new(data, control);
        let task = executor::task(async move { driver.run().await });
        let ending = Ending::new(DrivenEnd { sender: sender.clone(), task: Some(task), original: ending, failure: None });
        (Self::Binary(sender, receiver), Some(ending))
    }

    pub async fn connect(self) -> io::Result<(Sender, Receiver)> {
        match self {
            Self::Unavailable => {
                Err(io::Error::new(io::ErrorKind::Unsupported, "attachment requires an independent binary control channel"))
            }
            Self::Binary(sender, receiver) => Ok((Sender(sender), Receiver(receiver))),
        }
    }
}

impl Sender {
    pub async fn send(&self, frame: Frame) -> io::Result<()> {
        self.0.send(frame).await
    }
}

impl Receiver {
    pub async fn receive(&mut self) -> io::Result<Option<Arrival>> {
        Ok(self.0.receive().await?.map(Arrival::Binary))
    }
}

impl End for DrivenEnd {
    fn settle(&mut self) -> Pin<Box<dyn Future<Output = io::Result<()>> + Send + '_>> {
        Box::pin(async move {
            self.sender.close();
            if let Some(task) = self.task.as_mut() {
                let joined = task.await;
                self.task.take();
                if let Err(error) = joined {
                    self.failure = Some(error.to_string());
                }
                // Protocol EOF/Failure Is Reported By The Receiver; Joining Proves Physical I/O Ended.
            }
            if let Some(original) = self.original.as_mut() {
                original.settle().await?;
            }
            if let Some(failure) = &self.failure {
                return Err(io::Error::other(failure.clone()));
            }
            self.original.take();
            Ok(())
        })
    }
}

impl Drop for DrivenEnd {
    fn drop(&mut self) {
        self.sender.close();
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
