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

use crate::{Context, Plugin, Result, server};

use std::result::Result as StdResult;
use tokio::{runtime::Builder, task::LocalSet};
use tonic::Status;

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_protocol::plugin::ChildResponse;
use harmos_protocol::plugin::child_response;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Plugin {
    pub fn serve(self) -> Result<()> {
        let declarations = self.declarations()?;
        let runtime = Builder::new_current_thread().enable_all().build()?;
        runtime.block_on(LocalSet::new().run_until(server::serve(self, declarations)))
    }
}

impl Context {
    pub(crate) async fn reply(&self, child: ChildResponse) -> StdResult<(), Status> {
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
        self.cancel_calls(false, &reason, true).await?;
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
