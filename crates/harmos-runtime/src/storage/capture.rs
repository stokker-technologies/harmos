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

// Third Party Imports

// Local Imports
use super::manager::encoded_size;
use super::state::Permit;
use super::{ReadLimits, RecordInput};
use crate::{Error, Result, state::Envelope};
use std::{future::Future, pin::Pin};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct ProjectedRecords<'a> {
    permit: &'a mut Permit,
    limits: ReadLimits,
    refused: bool,
}

pub(crate) struct Projection {
    pub(crate) limits: ReadLimits,
    pub(crate) handler: Box<dyn RecordProjection>,
}

/* === Enums === */

/* === Traits === */

pub trait RecordProjection: Send + Sync + 'static {
    fn project<'a>(
        &'a self, record: &'a Envelope<&serde_json::Value>, output: &'a mut ProjectedRecords<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
}

/* === Implementations === */

impl<'a> ProjectedRecords<'a> {
    pub(crate) fn new(permit: &'a mut Permit, limits: ReadLimits) -> Self {
        Self { permit, limits, refused: false }
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.push", level = "debug", skip_all)]
    pub async fn push(&mut self, record: RecordInput) -> Result<()> {
        if self.refused {
            return Err(limit());
        }
        self.refused = true;
        let result = self.permit.push(record, self.limits).await;
        self.refused = result.is_err();
        result.map_err(Into::into)
    }

    pub(crate) fn check(&self, maximum: std::num::NonZeroUsize) -> Result<()> {
        if self.refused {
            return Err(limit());
        }
        for entry in self.permit.projected() {
            if encoded_size(entry)? > maximum.get() {
                return Err(Error::InvalidInput { field: "record projection", reason: "one entry exceeds max_batch_bytes" });
            }
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn limit() -> Error {
    Error::InvalidInput { field: "record projection", reason: "capture limit exceeded" }
}

/* === Tests === */
