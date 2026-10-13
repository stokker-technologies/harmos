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
use crate::schema::Described;

use crate::storage::SchemaInput;
use crate::storage::{SchemaInfo, SchemaKey, StorageCursor};

use crate::storage::{SchemaQuery, StorageManager};

use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */
crate::storage::codec::product!(QuerySchemasRequest {
    selection: crate::storage::codec::Nested<Selection> => 1,
    schema: crate::storage::codec::Optional<crate::storage::codec::Nested<SchemaKey>> => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuerySchemasRequest {
    pub selection: Selection,
    pub schema: Option<SchemaKey>,
}

pub struct QuerySchemas;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl crate::MemorySize for QuerySchemasRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.selection.memory_size()
            - std::mem::size_of::<Selection>()
            + self.schema.memory_size()
            - std::mem::size_of::<Option<SchemaKey>>()
    }
}

impl OperationDeclaration for QuerySchemas {
    type Request = QuerySchemasRequest;
    type Response = StorageCursor;
    const ID: &'static str = "storage.query_schemas";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<SchemaInfo>()))
    }
}

impl OutputDeclaration for QuerySchemas {
    type Item = SchemaInfo;
}

impl WireOperation for QuerySchemas {
    const VERSION: u32 = 2;
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(SchemaInfo::DEFINITION);
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = QuerySchemas;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<SchemaInfo>;

    fn new(storage: Arc<StorageManager<C>>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut QuerySchemasRequest,
        output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StorageCursor>> {
        let result: ::core::result::Result<StorageCursor, Self::Error> = async move {
            request.selection.validate()?;
            if request.selection.scope.is_some() || request.selection.item.is_some() {
                return Err(Error::InvalidInput {
                    field: "schema query",
                    reason: "scope and item filters do not apply to schemas",
                });
            }

            let query = SchemaQuery {
                schema: request.schema.clone(),
                options: request.selection.options()?,
            };
            let mut reader = self.storage.query(query).await?;
            let boundary = reader.view().ok_or(Error::ViewExpired)?.saved()?;
            if let Some(after) = &request.selection.after
                && !boundary.covers(after)?
            {
                return Err(Error::InvalidInput {
                    field: "storage query",
                    reason: "start exceeds saved boundary",
                });
            }
            while let Some(page) = reader.next_page().await? {
                for schema in page.iter() {
                    let scratch =
                        schema
                            .schema
                            .validation_memory_bytes(
                                &harmos_contract::config::SchemaSettings::default(),
                            )
                            .map_err(|_| Error::InvalidInput {
                                field: "storage schema",
                                reason: "invalid backend schema",
                            })?;
                    let bytes = schema
                        .memory_size()
                        .saturating_add(scratch)
                        .saturating_add(std::mem::size_of::<SchemaInfo>());
                    output
                        .send_with(bytes, || {
                            Ok(SchemaInfo {
                                position: schema.position.clone(),
                                schema: SchemaInput::new(schema.key.clone(), &schema.schema)?,
                            })
                        })
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
