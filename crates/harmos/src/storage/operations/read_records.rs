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
use super::DataSelection;

use crate::operations::{Context, Operation, StreamOutput};
use crate::operations::{Grpc, OperationDeclaration, OutputDeclaration, Visibility, WireOperation};
use crate::schema::{Described, Field};

use crate::storage::{RecordQuery, StorageManager};
use crate::storage::{StorageCursor, StoredRecord};

use crate::{Error, MemorySize, Result};
use harmos_contract::plugin::Grant;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct ReadRecords;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for ReadRecords {
    type Request = DataSelection;
    type Response = StorageCursor;
    const ID: &'static str = "storage.read_records";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    fn grant(selection: &DataSelection) -> Option<Grant> {
        Some(super::survey(&selection.schema.id))
    }

    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<StoredRecord>()))
    }
}

impl OutputDeclaration for ReadRecords {
    type Item = StoredRecord;
}

impl WireOperation for ReadRecords {
    const VERSION: u32 = 1;
    const GRPC: Option<Grpc> = Some(super::grpc("ReadRecords"));
    crate::with_protobuf! {
        fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
            Some(visitor.download::<Self>())
        }
    }
    const OUTPUT: Option<&'static [Field]> = Some(StoredRecord::FIELDS);
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = ReadRecords;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<StoredRecord>;

    fn new(storage: Arc<StorageManager<C>>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        selection: &mut DataSelection,
        output: &mut Self::Endpoints,
    ) -> Result<StorageCursor> {
        selection.validate()?;
        if selection.stream.is_some() {
            return Err(Error::InvalidInput {
                field: "storage record query",
                reason: "stream selectors require QueryStream",
            });
        }

        let mut query = RecordQuery::new(selection.schema.clone(), super::data_selection::decode);
        query.scope = selection.selection.scope.clone();
        query.item = selection.selection.item.clone();
        query.source_time = selection.source_time();
        query.options = selection.selection.options()?;
        let mut reader = self.storage.query(query).await?;
        let boundary = reader.view().ok_or(Error::ViewExpired)?.saved()?;
        if let Some(after) = &selection.selection.after
            && !boundary.covers(after)?
        {
            return Err(Error::InvalidInput {
                field: "storage query",
                reason: "start exceeds saved boundary",
            });
        }
        while let Some(page) = reader.next_page().await? {
            for entry in page.iter() {
                output
                    .send_with(entry.memory_size(), || Ok(entry.clone()))
                    .await?;
            }
        }
        Ok(boundary)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
