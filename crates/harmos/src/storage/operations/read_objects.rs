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
use std::any::TypeId;

use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::{ObjectResult, ObjectSelection};
use crate::MemorySize;

use crate::operations::{Context, Operation, StreamOutput};
use crate::operations::{Grpc, OperationDeclaration, OutputDeclaration, Visibility, WireOperation};
use crate::schema::{Described, Field};

use crate::storage::StorageManager;
use crate::storage::{ObjectFields, StorageCursor};

use crate::{Error, Result};
use harmos_contract::plugin::Grant;

/* === Constants === */

/* === Macros === */
crate::storage::codec::product!(ReadObjectsRequest {
    selection: crate::storage::codec::Nested<ObjectSelection> => 1,
    fields: crate::storage::codec::Optional<crate::storage::codec::Nested<ObjectFields>> => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReadObjectsRequest {
    pub selection: ObjectSelection,
    pub fields: Option<ObjectFields>,
}

pub struct ReadObjects;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemorySize for ReadObjectsRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.selection.memory_size()
            - std::mem::size_of_val(&self.selection)
            + self.fields.memory_size()
            - std::mem::size_of_val(&self.fields)
    }
}

impl OperationDeclaration for ReadObjects {
    type Request = ReadObjectsRequest;
    type Response = StorageCursor;
    const ID: &'static str = "storage.read_objects";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    fn grant(request: &ReadObjectsRequest) -> Option<Grant> {
        Some(match &request.selection.schema {
            Some(schema) => super::survey(&schema.id),
            None => Grant::Survey(super::EVERY_SCHEMA.into()),
        })
    }

    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<ObjectResult>()))
    }
}

impl OutputDeclaration for ReadObjects {
    type Item = ObjectResult;
}

impl WireOperation for ReadObjects {
    const VERSION: u32 = 1;
    const GRPC: Option<Grpc> = Some(super::grpc("ReadObjects"));
    crate::with_protobuf! {
        fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
            Some(visitor.download::<Self>())
        }
    }
    const OUTPUT: Option<&'static [Field]> = Some(ObjectResult::FIELDS);
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = ReadObjects;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<ObjectResult>;

    fn new(storage: Arc<StorageManager<C>>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut ReadObjectsRequest,
        output: &mut Self::Endpoints,
    ) -> Result<StorageCursor> {
        request.selection.validate()?;
        if let Some(fields) = &request.fields {
            fields.validate()?;
        }

        let query = request.selection.query(false)?;
        // Streaming Never Retries Automatically. Move The Original Input Plan
        // Into Query Admission Without Allocating A Second Copy Of Predicates.
        if let Some(fields) = request.fields.take() {
            let mut reader = self.storage.query(query.project(fields)?).await?;
            let boundary = reader.view().ok_or(Error::ViewExpired)?.saved()?;
            check_boundary(&request.selection, &boundary)?;
            while let Some(page) = reader.next_page().await? {
                for row in page.iter() {
                    output
                        .send_with(ObjectResult::wrapping_memory(row), || {
                            Ok(ObjectResult::Fields(row.clone()))
                        })
                        .await?;
                }
            }
            return Ok(boundary);
        }
        let mut reader = self.storage.query(query).await?;
        let boundary = reader.view().ok_or(Error::ViewExpired)?.saved()?;
        check_boundary(&request.selection, &boundary)?;
        while let Some(page) = reader.next_page().await? {
            for entry in page.iter() {
                output
                    .send_with(ObjectResult::wrapping_memory(entry), || {
                        Ok(ObjectResult::Value(entry.clone()))
                    })
                    .await?;
            }
        }
        Ok(boundary)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn check_boundary(selection: &ObjectSelection, boundary: &StorageCursor) -> Result<()> {
    if let Some(after) = &selection.selection.after
        && !boundary.covers(after)?
    {
        return Err(Error::InvalidInput {
            field: "storage query",
            reason: "start exceeds saved boundary",
        });
    }
    Ok(())
}

/* === Tests === */
