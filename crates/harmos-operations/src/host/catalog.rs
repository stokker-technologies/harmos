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
use harmos_domain::plugin::Grant;
use harmos_protocol::host::{CatalogKind as Kind, CatalogPage};
use harmos_storage::{
    ErasedCatalog, IdentityKind, IdentitySelection, ReadLimits, StorageManager, StorageSchema, StoredIdentity, query::SchemaSelection,
};
use serde::Serialize;
use std::{num::NonZeroUsize, sync::Arc};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Result<T> = std::result::Result<T, Error>;

/* === Structs === */

pub struct Catalog;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Catalog {
    type Request = harmos_protocol::host::Catalog;
    type Response = CatalogPage;
    type Endpoints = ();
    const ID: &'static str = "host.catalog";
    const GRPC: Grpc = Grpc::new("harmos.host.v1", "Host", "Catalog");
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[
        Resource::of::<Option<Arc<StorageManager<ErasedCatalog>>>>(),
        Resource::of::<crate::execution::operations::access::DiscoveryAccess>(),
    ];
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

pub fn limits(request: &harmos_protocol::host::Catalog) -> std::result::Result<(usize, usize), MessageError> {
    let rows = request.rows.unwrap_or(64);
    let bytes = request.bytes.unwrap_or(64 * 1024);
    if rows == 0 || rows > 256 || !(2..=4 * 1024 * 1024).contains(&bytes) {
        return Err(MessageError::borrowed("rows must be 1..=256 and bytes must be 2..=4194304"));
    }
    Ok((rows as usize, bytes as usize))
}

/* === Private Functions === */

fn kind(request: &harmos_protocol::host::Catalog) -> std::result::Result<Kind, MessageError> {
    Kind::try_from(request.kind).map_err(|_| MessageError::borrowed("unknown catalog kind"))
}
async fn run(context: ExecutionContext, request: harmos_protocol::host::Catalog) -> Result<CatalogPage> {
    let storage =
        context.resource::<Option<Arc<StorageManager<ErasedCatalog>>>>().map_err(|_| Error::NotReady { domain: "catalog storage" })?;
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
            host.0.discover(request.after.as_deref(), rows, bytes).map_err(Error::Access)?
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
    storage: &StorageManager<ErasedCatalog>, selection: &T::Selection, request: &harmos_protocol::host::Catalog, keep: impl Fn(&T) -> bool,
) -> Result<CatalogPage>
where
    T: harmos_storage::follow::Entry,
{
    let (rows, bytes) = limits(request)?;
    let after = request
        .after
        .as_deref()
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
    Ok(CatalogPage { entries, next, operations: Vec::new() })
}

fn write(bytes: &mut Vec<u8>, entry: &impl Serialize) -> std::result::Result<(), serde_json::Error> {
    serde_json::to_writer(bytes, entry)
}

/* === Tests === */
