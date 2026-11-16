/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Compound Storage Mutation Operation

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::error;

// Third Party Imports

// Local Imports
use super::requests::{CompoundOutcome, MutateRequest, MutateResponse, required};
use crate::MemorySize;
use crate::execution::operations::{ExecutionContext, NativeOperation, Resource, ServingError};
use crate::operations::{FailureKind, ReplaySafety};
use harmos_storage::{ObjectVersionToken, StorageManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Mutate;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl NativeOperation for Mutate {
    type Request = MutateRequest;
    type Response = MutateResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.mutate";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<_> = async {
            let mutation = required(request.mutation)?;

            // Refuse Replies That Cannot Fit Before Admitting Any Effects.
            let mut frontier = storage.status().accepted;
            frontier.sequence = u64::MAX;
            let mut maximum = MutateResponse {
                outcome: CompoundOutcome::Applied { positions: Vec::new() },
                receipt: Some(super::transfers::receipt(&mutation, Some(frontier.clone()))),
            };
            let applied_bytes = maximum.memory_size().saturating_add(request.items.len().saturating_mul(frontier.memory_size()));
            maximum.outcome = CompoundOutcome::Conflict {
                index: 0,
                current: Some(ObjectVersionToken::new(frontier)),
            };
            if applied_bytes.max(maximum.memory_size()) > storage.settings().response.batch_bytes.get() {
                return Err(harmos_storage::Error::ResourceExhausted {
                    resource: "compound mutation reply; reduce the item batch or raise response.batch_bytes",
                }
                .into());
            }

            let receipt = storage.mutate_once(&mutation, request.items).await?;
            Ok(MutateResponse {
                outcome: receipt.outcome,
                receipt: Some(super::transfers::receipt(&mutation, Some(receipt.position))),
            })
        }
        .await;
        result.map_err(super::admission::failure).and_then(|response| super::admission::read_response(&context, response))
    }

    fn classify(error: &(dyn error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
