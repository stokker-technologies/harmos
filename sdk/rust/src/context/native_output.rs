/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Child Output

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::{Error, Result};
use harmos_protocol::plugin::{InstanceInvokeResponse, InstanceSamplesResponse, instance_invoke_response, instance_samples_response};
use std::{
    result::Result as StdResult,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::mpsc;
use tonic::Status;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Clone)]
pub(crate) enum ChildOutput {
    Invoke(mpsc::Sender<StdResult<InstanceInvokeResponse, Status>>),
    Samples { sender: mpsc::Sender<StdResult<InstanceSamplesResponse, Status>>, active: Arc<AtomicBool> },
}

/* === Traits === */

/* === Implementations === */

impl ChildOutput {
    pub(super) fn check(&self) -> Result<()> {
        if let Self::Samples { active, .. } = self
            && !active.load(Ordering::Acquire)
        {
            return Err(Status::invalid_argument("Call Child: No Unanswered Samples Tick").into());
        }
        Ok(())
    }

    pub(super) async fn send(&self, frame: instance_invoke_response::Frame) -> Result<()> {
        match self {
            Self::Invoke(sender) => sender
                .send(Ok(InstanceInvokeResponse { frame: Some(frame) }))
                .await
                .map_err(|_| Status::unavailable("Send Child Frame: Host Closed"))?,
            Self::Samples { sender, .. } => {
                let cancel = matches!(&frame, instance_invoke_response::Frame::ChildCancel(_));
                let frame = match frame {
                    instance_invoke_response::Frame::Child(child) => instance_samples_response::Frame::Child(child),
                    instance_invoke_response::Frame::ChildCancel(cancel) => instance_samples_response::Frame::ChildCancel(cancel),
                    instance_invoke_response::Frame::ChildDemand(demand) => instance_samples_response::Frame::ChildDemand(demand),
                    instance_invoke_response::Frame::Response(_) => {
                        return Err(Error::Invalid("Send Child Frame: Unexpected Result".into()));
                    }
                };
                let permit = sender.reserve().await.map_err(|_| Status::unavailable("Send Samples Child: Host Closed"))?;
                if !cancel {
                    self.check()?;
                }
                permit.send(Ok(InstanceSamplesResponse { samples: Vec::new(), frame: Some(frame) }));
            }
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
