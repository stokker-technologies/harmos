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
use super::Selection;

use crate::operations::{Context, Operation, StreamOutput};
use crate::operations::{OperationDeclaration, OutputDeclaration, Visibility, WireOperation};
use crate::schema::{Described, Field};

use crate::storage::{FileQuery, StorageManager};
use crate::storage::{FileReference, StorageCursor};

use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct QueryFiles;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for QueryFiles {
    type Request = Selection;
    type Response = StorageCursor;
    const ID: &'static str = "storage.query_files";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<FileReference>()))
    }
}

impl OutputDeclaration for QueryFiles {
    type Item = FileReference;
}

impl WireOperation for QueryFiles {
    const VERSION: u32 = 1;
    const OUTPUT: Option<&'static [Field]> = Some(FileReference::FIELDS);
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = QueryFiles;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<FileReference>;

    fn new(storage: Arc<StorageManager<C>>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        selection: &mut Selection,
        output: &mut Self::Endpoints,
    ) -> Result<StorageCursor> {
        selection.validate()?;

        let query = FileQuery {
            scope: selection.scope.clone(),
            item: selection.item.clone(),
            options: selection.options()?,
        };
        let mut reader = self.storage.query(query).await?;
        let boundary = reader.view().ok_or(Error::ViewExpired)?.saved()?;
        if let Some(after) = &selection.after
            && !boundary.covers(after)?
        {
            return Err(Error::InvalidInput {
                field: "storage query",
                reason: "start exceeds saved boundary",
            });
        }
        while let Some(page) = reader.next_page().await? {
            for file in page.iter() {
                output
                    .send_with(file.memory_size(), || Ok(file.clone()))
                    .await?;
            }
        }
        Ok(boundary)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
