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
use std::sync::Arc;
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use super::super::{Error, Resources, Result};
use crate::contract::{self as worker, PAYLOAD_BYTES, Payload};
use harmos::sidecar::Supervisor;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct Exchanging {
    resources: Arc<Resources>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos::operation]
impl Exchanging {
    async fn initialize(resources: Arc<Resources>, _: &Supervisor) -> Result<Self> {
        resources.constructed.fetch_add(1, Ordering::SeqCst);
        Ok(Self { resources })
    }
    async fn run(
        &mut self,
        _: &Supervisor,
        request: &mut worker::Exchange,
    ) -> std::result::Result<Payload, Error> {
        if request.bytes.len() > PAYLOAD_BYTES {
            return Err(Error::PayloadTooLarge {
                actual: request.bytes.len(),
                limit: PAYLOAD_BYTES,
            });
        }
        self.resources.completed.fetch_add(1, Ordering::SeqCst);
        Ok(Payload {
            sequence: request.sequence,
            bytes: std::mem::take(&mut request.bytes),
        })
    }
    async fn cleanup(&mut self) -> Result<()> {
        self.resources.finalized.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
