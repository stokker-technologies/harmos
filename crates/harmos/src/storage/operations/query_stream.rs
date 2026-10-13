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
use crate::operations::{OperationDeclaration, OutputDeclaration, Visibility, WireOperation};
use crate::schema::Described;
use crate::storage::{StorageCursor, StoredRow};

use crate::storage::{StorageManager, StreamQuery};

use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct QueryStream;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for QueryStream {
    type Request = DataSelection;
    type Response = StorageCursor;
    const ID: &'static str = "storage.query_stream";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<StoredRow>()))
    }
}

impl OutputDeclaration for QueryStream {
    type Item = StoredRow;
}

impl WireOperation for QueryStream {
    const VERSION: u32 = 1;
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(StoredRow::DEFINITION);
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = QueryStream;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<StoredRow>;

    fn new(storage: Arc<StorageManager<C>>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        selection: &mut DataSelection,
        output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StorageCursor>> {
        let result: ::core::result::Result<StorageCursor, Self::Error> = async move {
            selection.validate()?;

            let mut query =
                StreamQuery::new(selection.schema.clone(), super::data_selection::decode);
            query.scope = selection.selection.scope.clone();
            query.item = selection.selection.item.clone();
            query.source_time = selection.source_time();
            query.options = selection.selection.options()?;
            query.stream = selection.stream.clone();
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
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
