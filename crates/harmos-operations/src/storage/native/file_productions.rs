/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// List Unfinished File Productions

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::error;
use std::num::NonZeroUsize;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use super::requests::{ListFileProductionsRequest, ListFileProductionsResponse};
use super::transfers;
use crate::MemorySize;
use crate::execution::operations::{ExecutionContext, NativeOperation, Resource, ServingError};
use crate::operations::{FailureKind, OperationPolicy, ReplaySafety};
use harmos_protocol::model;
use harmos_storage::{Identifier, ReadLimits, StorageCursor, StorageManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct ListFileProductions;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u32,
    scope: Identifier,
    at: StorageCursor,
    after: Identifier,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl NativeOperation for ListFileProductions {
    type Request = ListFileProductionsRequest;
    type Response = ListFileProductionsResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.list_file_productions";
    const POLICY: OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        list(&storage, request).map_err(super::admission::failure)
    }

    fn classify(error: &(dyn error::Error + Send + Sync + 'static)) -> FailureKind {
        transfers::classify(error)
    }
}

impl TryFrom<model::ListFileProductionsRequest> for ListFileProductionsRequest {
    type Error = crate::Error;

    fn try_from(request: model::ListFileProductionsRequest) -> crate::Result<Self> {
        let scope = Identifier::new(request.scope).map_err(harmos_domain::Error::from)?;
        Ok(Self {
            scope,
            page: request.page,
            at: request.at,
        })
    }
}

impl From<ListFileProductionsResponse> for model::ListFileProductionsResponse {
    fn from(response: ListFileProductionsResponse) -> Self {
        Self {
            productions: response
                .productions
                .into_iter()
                .map(|production| model::Upload {
                    id: production.session.id.to_string(),
                    history: Some(production.session.history.into()),
                    writer_generation: production.session.generation,
                    accepted_length: production.accepted_length,
                    durable_length: production.durable_length,
                    state: model::UploadState::Producing.into(),
                })
                .collect(),
            next_page_token: response.next_page_token,
            resolved_at: response.resolved_at,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn list(storage: &StorageManager, request: ListFileProductionsRequest) -> crate::Result<ListFileProductionsResponse> {
    let page = request.page.unwrap_or_default();
    let policy = &storage.settings().response;
    let rows = NonZeroUsize::new(page.page_size as usize).unwrap_or(policy.batch_rows).min(policy.batch_rows);
    let bytes = if page.max_bytes == 0 {
        policy.batch_bytes
    } else {
        let bytes = usize::try_from(page.max_bytes).map_err(|_| exhausted())?;
        NonZeroUsize::new(bytes).ok_or_else(exhausted)?.min(policy.batch_bytes)
    };
    let at = transfers::at(request.at)?;
    let cursor = if page.page_token.is_empty() {
        None
    } else {
        if page.page_token.len() > policy.batch_bytes.get() {
            return Err(invalid());
        }
        let cursor: Cursor = serde_json::from_str(&page.page_token).map_err(|_| invalid())?;
        if cursor.version != 1 || cursor.scope != request.scope || at.as_ref().is_some_and(|at| *at != cursor.at) {
            return Err(invalid());
        }
        Some(cursor)
    };
    let at = cursor.as_ref().map(|cursor| &cursor.at).or(at.as_ref());
    let after = cursor.as_ref().map(|cursor| &cursor.after);
    let selected = storage.list_file_productions(&request.scope, after, at, ReadLimits { rows, bytes })?;
    let mut more = selected.next.is_some();
    let mut response = ListFileProductionsResponse {
        productions: selected.productions,
        next_page_token: String::new(),
        resolved_at: Some(transfers::cut(selected.at.clone())),
    };
    loop {
        response.next_page_token = if more {
            let last = response.productions.last().ok_or_else(exhausted)?;
            let cursor = Cursor {
                version: 1,
                scope: request.scope.clone(),
                at: selected.at.clone(),
                after: last.session.id.clone(),
            };
            serde_json::to_string(&cursor).map_err(|_| invalid())?
        } else {
            String::new()
        };
        if response.memory_size() <= bytes.get() {
            return Ok(response);
        }
        response.productions.pop().ok_or_else(exhausted)?;
        response.productions.shrink_to_fit();
        more = true;
    }
}

fn invalid() -> crate::Error {
    harmos_storage::Error::InvalidInput {
        field: "file discovery continuation",
        reason: "restart discovery with the same exact scope and issued storage cut",
    }
    .into()
}

fn exhausted() -> crate::Error {
    harmos_storage::Error::ResourceExhausted {
        resource: "file discovery page; increase the response byte budget",
    }
    .into()
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::operations::OperationFactory;
    use harmos_storage::{FileProduction, FileSession, StoreHistory};
    use prost::Message;

    #[test]
    fn discovery_codec_and_catalog_preserve_native_admission() {
        let wire = model::ListFileProductionsRequest {
            scope: "scope".into(),
            at: None,
            page: Some(model::PageRequest {
                page_size: 2,
                max_bytes: 4096,
                page_token: String::new(),
            }),
        };
        let decoded = model::ListFileProductionsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
        let native = ListFileProductionsRequest::try_from(decoded).unwrap();
        assert_eq!(native.scope.as_str(), "scope");
        assert_eq!(native.page, wire.page);
        assert!(ListFileProductionsRequest::try_from(model::ListFileProductionsRequest::default()).is_err());
        let history = StoreHistory {
            store: Identifier::new("store").unwrap(),
            epoch: Identifier::new("epoch").unwrap(),
        };
        let native = ListFileProductionsResponse {
            productions: vec![FileProduction {
                session: FileSession {
                    history: history.clone(),
                    id: Identifier::new("file").unwrap(),
                    generation: 3,
                },
                accepted_length: 9,
                durable_length: 6,
            }],
            resolved_at: Some(transfers::cut(StorageCursor { history, sequence: 7 })),
            next_page_token: "cursor".into(),
        };
        let wire = model::ListFileProductionsResponse::from(native);
        let decoded = model::ListFileProductionsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
        assert_eq!(decoded, wire);
        assert_eq!(decoded.productions[0].writer_generation, 3);
        assert_eq!(decoded.productions[0].accepted_length, 9);
        assert_eq!(decoded.productions[0].durable_length, 6);
        let get = OperationFactory::native::<super::super::GetFileProduction>().unwrap();
        let list = super::super::storage_catalog()
            .operations
            .into_iter()
            .find(|factory| factory.definition.id.as_str() == ListFileProductions::ID)
            .unwrap();
        assert_eq!(list.definition.visibility, get.definition.visibility);
        assert_eq!(list.definition.policy, get.definition.policy);
        assert!(list.contract().is_err());
    }
}
