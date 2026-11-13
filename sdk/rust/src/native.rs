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
    pub(crate) async fn reply(&self, child: v1::ChildResponse) -> std::result::Result<(), tonic::Status> {
        let watch = {
            let mut calls = self.calls.lock().expect("Child Calls Poisoned");
            if matches!(&child.result, Some(child_response::Result::Item(_))) {
                calls.watches.get(&child.call).cloned()
            } else {
                calls.watches.remove(&child.call)
            }
        };
        if let Some((watch, limit, _)) = watch {
            if matches!(&child.result, Some(child_response::Result::Item(bytes) | child_response::Result::Payload(bytes)) if bytes.len() > limit)
            {
                return Err(tonic::Status::resource_exhausted("Watch: Item Exceeds Consumer Limit"));
            }
            let item = match child.result {
                Some(child_response::Result::Item(bytes)) => Ok(Some(bytes)),
                Some(child_response::Result::Payload(_)) => Ok(None),
                Some(child_response::Result::Failure(failure)) => Err(failure.into()),
                None => return Err(tonic::Status::invalid_argument("Watch: Child Result Missing")),
            };
            let _ = watch.send(item).await;
            return Ok(());
        }
        if self.calls.lock().expect("Child Calls Poisoned").cancelled.contains(&child.call) {
            return Ok(());
        }
        let result = match child.result {
            Some(child_response::Result::Payload(payload)) => Ok(payload),
            Some(child_response::Result::Failure(failure)) => Err(failure.into()),
            Some(child_response::Result::Item(_)) | None => return Err(tonic::Status::invalid_argument("Unary Child Result Missing")),
        };
        let send = self
            .calls
            .lock()
            .expect("Child Calls Poisoned")
            .pending
            .remove(&child.call)
            .ok_or_else(|| tonic::Status::invalid_argument("Unknown or Duplicate Child"))?;
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
