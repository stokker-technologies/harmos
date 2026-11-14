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

use crate::execution::operations::{
    ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility, access::DiscoveryAccess,
};
use crate::plugin::Grant;
use crate::{Error, Identifier, MessageError};
use harmos_protocol::host::OperationsGetRequest;
use harmos_protocol::host::OperationsGetResponse;
use harmos_protocol::host::OperationsListRequest;
use harmos_protocol::host::OperationsListResponse;
use harmos_protocol::host::SchemaDescription;
use harmos_protocol::host::SchemasListRequest;
use harmos_protocol::host::SchemasListResponse;
use harmos_protocol::host::StreamIdentity;
use harmos_protocol::host::StreamsListRequest;
use harmos_protocol::host::StreamsListResponse;
use harmos_storage::follow::Entry;
use harmos_storage::{IdentityKind, IdentitySelection, ReadLimits, StorageManager, StorageSchema, StoredIdentity, query::SchemaSelection};
use std::{num::NonZeroUsize, sync::Arc};

/* === Constants === */

/* === Macros === */

macro_rules! operation {
    ($name:ident, $request:ty, $response:ty, $service:literal, $method:literal, $resources:expr) => {
        impl ProtobufOperation for $name {
            type Request = $request;
            type Response = $response;
            type Endpoints = ();
            const ID: &'static str = concat!("harmos.host.v1.", $service, "/", $method);
            const GRPC: Option<Grpc> = Some(Grpc::new("harmos.host.v1", $service, $method));
            const POLICY: crate::OperationPolicy = super::POLICY;
            const VISIBILITY: Visibility = Visibility::Public;
            const RESOURCES: &'static [Resource] = $resources;
            fn grant(request: &Self::Request) -> Result<Option<Grant>, MessageError> {
                Self::authorize(request)
            }
            async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
                Self::serve(context, request).await.map_err(|error| ServingError::Native(Arc::new(error)))
            }
        }
    };
}

/* === Type Aliases === */

/* === Structs === */

pub struct List;
pub struct Get;
pub struct Schemas;
pub struct Streams;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

operation!(List, OperationsListRequest, OperationsListResponse, "Operations", "List", &[Resource::of::<DiscoveryAccess>()]);
operation!(Get, OperationsGetRequest, OperationsGetResponse, "Operations", "Get", &[Resource::of::<DiscoveryAccess>()]);
operation!(Schemas, SchemasListRequest, SchemasListResponse, "Schemas", "List", &[Resource::of::<Option<Arc<StorageManager>>>()]);
operation!(Streams, StreamsListRequest, StreamsListResponse, "Streams", "List", &[Resource::of::<Option<Arc<StorageManager>>>()]);

impl List {
    fn authorize(_: &OperationsListRequest) -> Result<Option<Grant>, MessageError> {
        Ok(None)
    }
    async fn serve(context: ExecutionContext, _: OperationsListRequest) -> Result<OperationsListResponse, Error> {
        let access = context.resource::<DiscoveryAccess>().map_err(|_| Error::NotReady { domain: "operation discovery" })?;
        let operations = access.0.discover().map_err(Error::Access)?;
        Ok(OperationsListResponse { operations })
    }
}
impl Get {
    fn authorize(request: &OperationsGetRequest) -> Result<Option<Grant>, MessageError> {
        harmos_protocol::schema::operation_identity(&request.id).map_err(|error| MessageError::new(error.to_string()))?;
        Ok(None)
    }
    async fn serve(context: ExecutionContext, request: OperationsGetRequest) -> Result<OperationsGetResponse, Error> {
        Self::authorize(&request)?;
        let entries = List::serve(context, OperationsListRequest {}).await?;
        let operation =
            entries.operations.into_iter().find(|entry| entry.contract.as_ref().is_some_and(|contract| contract.id == request.id)).ok_or(
                Error::InvalidInput {
                    field: "Get Operation",
                    reason: "operation is unavailable or unauthorized",
                },
            )?;
        Ok(OperationsGetResponse { operation: Some(operation) })
    }
}
impl Schemas {
    fn authorize(request: &SchemasListRequest) -> Result<Option<Grant>, MessageError> {
        grant(Self::ID, "schemas", request.schema.as_deref())
    }
    async fn serve(context: ExecutionContext, request: SchemasListRequest) -> Result<SchemasListResponse, Error> {
        Self::authorize(&request)?;
        let storage = storage(&context)?;
        let entries = stored::<StorageSchema>(&storage, &SchemaSelection { schema: None }).await?;
        let schemas = entries
            .into_iter()
            .filter(|entry| request.schema.as_ref().is_none_or(|schema| schema == entry.key.id.as_str()))
            .map(|entry| SchemaDescription {
                position: Some(entry.position.into()),
                key: Some(entry.key.into()),
                schema: Some(entry.schema.as_ref().clone().into()),
            })
            .collect();
        Ok(SchemasListResponse { schemas })
    }
}
impl Streams {
    fn authorize(request: &StreamsListRequest) -> Result<Option<Grant>, MessageError> {
        grant(Self::ID, "streams", request.schema.as_deref())
    }
    async fn serve(context: ExecutionContext, request: StreamsListRequest) -> Result<StreamsListResponse, Error> {
        Self::authorize(&request)?;
        let storage = storage(&context)?;
        let schema = request.schema.map(Identifier::new).transpose().map_err(harmos_domain::Error::from)?;
        let entries = stored::<StoredIdentity>(&storage, &IdentitySelection { kind: IdentityKind::Streams, schema }).await?;
        let streams = entries
            .into_iter()
            .filter_map(|entry| match entry {
                StoredIdentity::Stream { schema, scope, item, stream, first } => Some(StreamIdentity {
                    schema: Some(schema.into()),
                    scope: scope.map(|scope| scope.to_string()),
                    item: item.to_string(),
                    stream: stream.to_string(),
                    first: Some(first.into()),
                }),
                StoredIdentity::Object { .. } => None,
            })
            .collect();
        Ok(StreamsListResponse { streams })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn grant(id: &'static str, family: &str, schema: Option<&str>) -> Result<Option<Grant>, MessageError> {
    if let Some(schema) = schema {
        Identifier::new(schema).map_err(|error| MessageError::new(error.to_string()))?;
    }
    let scope = schema.map_or_else(|| format!("{family}:*"), |schema| format!("{family}:schema:{schema}"));
    Ok(Some(Grant::of(id, scope)))
}
fn storage(context: &ExecutionContext) -> Result<Arc<StorageManager>, Error> {
    let resource = context.resource::<Option<Arc<StorageManager>>>().map_err(|_| Error::NotReady { domain: "registry storage" })?;
    resource.as_ref().clone().ok_or(Error::NotReady { domain: "registry storage" })
}
async fn stored<T>(storage: &StorageManager, selection: &T::Selection) -> Result<Vec<T>, Error>
where
    T: Entry + Clone,
{
    let through = storage.get_saved_position().sequence;
    let mut after = 0;
    let mut entries = Vec::new();
    let limits = ReadLimits {
        rows: NonZeroUsize::MAX,
        bytes: NonZeroUsize::MAX,
    };
    loop {
        let page = storage.page::<T>(selection, after, limits).await?;
        if page.entries().is_empty() {
            break;
        }
        for entry in page.entries() {
            after = entry.position().sequence;
            if after > through {
                break;
            }
            entries.push(entry.clone());
        }
        if after >= through {
            break;
        }
    }
    Ok(entries)
}

/* === Tests === */
