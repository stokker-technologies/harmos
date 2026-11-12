/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use super::snapshot::SavedSnapshots;
use crate::execution::operations::access::GrantAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};
use harmos_operations::operations::ReplaySafety;
use harmos_protocol::storage as wire;
use harmos_storage::{
    StorageManager,
    query::{
        columnar::{Cancellation, ColumnSource},
        protobuf::required,
    },
};

/* === Structs === */

pub struct ScanIndex;

/* === Implementations === */

impl ProtobufOperation for ScanIndex {
    type Request = wire::ScanIndex;
    type Response = wire::IndexPage;
    type Endpoints = ();
    const ID: &'static str = "storage.scan_index";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "ScanIndex"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<SavedSnapshots>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.source.clone())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let snapshots = context.resource::<SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
        let cancel = Cancellation::default();
        super::native::run(
            &context,
            async {
                let source: ColumnSource = required(request.source)?.try_into()?;
                super::native::admit(&context, Self::ID, &source)?;
                let read: harmos_storage::query::index::IndexRead = required(request.read)?.try_into()?;
                let (definition, revision) = storage.get_index_definition(&read.name).await?;
                super::native::source_matches(&source, &definition.source)?;
                let mut snapshot = snapshots.read(required(request.snapshot)?)?;
                let page = snapshot.list_index_entries(&read, &cancel).await?;
                let (_, current) = storage.get_index_definition(&read.name).await?;
                if current != revision {
                    return Err(crate::Error::InvalidInput {
                        field: "index revision",
                        reason: "index rebuilt during the read",
                    });
                }
                super::native::admit(&context, Self::ID, &source)?;
                let page: wire::IndexPage = page.into();
                super::native::page_bytes(page.buffer_bytes(), read.limits.bytes.get())?;
                snapshot.retain();
                Ok(page)
            },
            cancel.clone(),
        )
        .await
    }
}
