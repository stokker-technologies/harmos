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

use crate::{Context, Result};
use harmos_protocol::plugin::instance_invoke_response;
use harmos_protocol::v1::{self, child_response};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl crate::Plugin {
    pub fn serve(self) -> Result<()> {
        let catalog = self.catalog()?;
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        runtime.block_on(tokio::task::LocalSet::new().run_until(crate::server::serve(self, catalog)))
    }
}

impl Context {
    pub(crate) fn reply(&self, child: v1::ChildResponse) -> std::result::Result<(), tonic::Status> {
        let result = match child.result {
            Some(child_response::Result::Payload(payload)) => Ok(payload),
            Some(child_response::Result::Failure(failure)) => Err(failure.into()),
            None => return Err(tonic::Status::invalid_argument("Child Result Missing")),
        };
        let send = self
            .calls
            .lock()
            .expect("Child Calls Poisoned")
            .pending
            .remove(&child.call)
            .ok_or_else(|| tonic::Status::invalid_argument("Unknown or Duplicate Child"))?;
        let _ = send.send(result);
        Ok(())
    }

    pub(crate) async fn cancel(&self, reason: String) -> Result<()> {
        self.cancellation.send_replace(Some(reason.clone()));
        let pending = std::mem::take(&mut self.calls.lock().expect("Child Calls Poisoned").pending);
        for (call, send) in pending {
            if let Some(output) = &self.output {
                output
                    .send(Ok(harmos_protocol::plugin::InstanceInvokeResponse {
                        frame: Some(instance_invoke_response::Frame::ChildCancel(v1::ChildCancel { call })),
                    }))
                    .await
                    .map_err(|_| tonic::Status::unavailable("Host Closed"))?;
            }
            let _ = send.send(Err(self.cancel_failure(reason.clone())));
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
