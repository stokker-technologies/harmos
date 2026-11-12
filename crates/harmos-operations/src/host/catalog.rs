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

use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};
use crate::{Error, Identifier, MessageError};
use harmos_operations::plugin::Grant;
use harmos_protocol::host::{CatalogEntriesListResponse, CatalogKind as Kind};
use harmos_storage::{IdentityKind, IdentitySelection, ReadLimits, StorageManager, StorageSchema, StoredIdentity, query::SchemaSelection};
use serde::Serialize;
use std::{num::NonZeroUsize, sync::Arc};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Result<T> = std::result::Result<T, Error>;

/* === Structs === */

pub struct List;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for List {
    type Request = harmos_protocol::host::CatalogEntriesListRequest;
    type Response = CatalogEntriesListResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.catalog_entries.list";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.host.v1", "CatalogEntries", "List"));
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<Option<Arc<StorageManager>>>(), Resource::of::<crate::execution::operations::access::DiscoveryAccess>()];
    fn grant(request: &Self::Request) -> std::result::Result<Option<Grant>, MessageError> {
        if let Some(schema) = &request.schema {
            Identifier::new(schema.as_str()).map_err(|_| MessageError::borrowed("invalid catalog schema identifier"))?;
        }
        let (family, all) = match kind(request)? {
            Kind::Operations => return Ok(None),
            Kind::Schemas => ("schemas", "schemas:*"),
            Kind::Streams => ("streams", "streams:*"),
            Kind::Objects => ("objects", "objects:*"),
        };
        Ok(Some(Grant::of(Self::ID, request.schema.as_ref().map_or_else(|| all.to_owned(), |schema| format!("{family}:schema:{schema}")))))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> std::result::Result<Self::Response, ServingError> {
        run(context, request).await.map_err(|error| ServingError::Native(Arc::new(error)))
    }
}

/* === Public Functions === */

pub fn limits(request: &harmos_protocol::host::CatalogEntriesListRequest) -> std::result::Result<(usize, usize), MessageError> {
    let page = request.page.as_ref().ok_or_else(|| MessageError::borrowed("List requires page_size and max_bytes"))?;
    let rows = page.page_size;
    let bytes = page.max_bytes;
    if rows == 0 || bytes < 2 || usize::try_from(bytes).is_err() {
        return Err(MessageError::borrowed("List requires positive page_size and representable max_bytes of at least two"));
    }
    Ok((rows as usize, bytes as usize))
}

/* === Private Functions === */

fn kind(request: &harmos_protocol::host::CatalogEntriesListRequest) -> std::result::Result<Kind, MessageError> {
    Kind::try_from(request.kind).map_err(|_| MessageError::borrowed("unknown catalog kind"))
}
async fn run(context: ExecutionContext, request: harmos_protocol::host::CatalogEntriesListRequest) -> Result<CatalogEntriesListResponse> {
    let storage = context.resource::<Option<Arc<StorageManager>>>().map_err(|_| Error::NotReady { domain: "catalog storage" })?;
    let access = context
        .resource::<crate::execution::operations::access::DiscoveryAccess>()
        .map_err(|_| Error::NotReady { domain: "catalog discovery" })?;
    let host = access;
    let kind = kind(&request)?;
    let schema = request.schema.as_deref().map(Identifier::new).transpose().map_err(|_| Error::InvalidInput {
        field: "catalog schema",
        reason: "invalid identifier",
    })?;
    let page = match kind {
        Kind::Operations => {
            let (rows, bytes) = limits(&request)?;
            let token = request.page.as_ref().map(|page| page.page_token.as_str()).filter(|token| !token.is_empty());
            host.0.discover(token, rows, bytes).map_err(Error::Access)?
        }
        Kind::Schemas => {
            let schema = schema.clone();
            stored::<StorageSchema>(
                storage.as_deref().as_ref().ok_or(Error::NotReady { domain: "catalog storage" })?,
                &SchemaSelection { schema: None },
                &request,
                |entry| schema.as_ref().is_none_or(|schema| schema == &entry.key.id),
            )
            .await?
        }
        Kind::Streams | Kind::Objects => {
            let selection = IdentitySelection {
                kind: if kind == Kind::Streams { IdentityKind::Streams } else { IdentityKind::Objects },
                schema: schema.clone(),
            };
            stored::<StoredIdentity>(
                storage.as_deref().as_ref().ok_or(Error::NotReady { domain: "catalog storage" })?,
                &selection,
                &request,
                |_| true,
            )
            .await?
        }
    };

    Ok(page)
}

async fn stored<T>(
    storage: &StorageManager, selection: &T::Selection, request: &harmos_protocol::host::CatalogEntriesListRequest,
    keep: impl Fn(&T) -> bool,
) -> Result<CatalogEntriesListResponse>
where
    T: harmos_storage::follow::Entry,
{
    let (rows, bytes) = limits(request)?;
    let after = request
        .page
        .as_ref()
        .map(|page| page.page_token.as_str())
        .filter(|token| !token.is_empty())
        .map(str::parse::<u64>)
        .transpose()
        .map_err(|_| Error::InvalidInput {
            field: "catalog cursor",
            reason: "cursor must be a storage position",
        })?
        .unwrap_or(0);
    let limits = ReadLimits {
        rows: NonZeroUsize::new(rows).expect("validated catalog rows"),
        bytes: NonZeroUsize::new(bytes).expect("validated catalog bytes"),
    };
    let page = storage.page::<T>(selection, after, limits).await?;
    let mut entries = vec![b'['];
    let mut next = None;
    let mut count = 0usize;
    for entry in page.entries() {
        let position = entry.position().sequence;
        if keep(entry) {
            let start = entries.len();
            if count > 0 {
                entries.push(b',');
            }
            if write(&mut entries, entry).is_err() || entries.len() + 1 > bytes {
                entries.truncate(start);
                if next.is_none() {
                    return Err(Error::ResourceExhausted { resource: "catalog entry bytes" });
                }
                break;
            }
            count += 1;
        }
        next = Some(position.to_string());
    }
    entries.push(b']');
    Ok(CatalogEntriesListResponse {
        entries,
        next_page_token: next.unwrap_or_default(),
        operations: Vec::new(),
        resolved_at: None,
    })
}

fn write(bytes: &mut Vec<u8>, entry: &impl Serialize) -> std::result::Result<(), serde_json::Error> {
    serde_json::to_writer(bytes, entry)
}

/* === Tests === */
