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

use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::MemorySize;

use crate::operations::{Context, Operation};
use crate::operations::{Grpc, OperationDeclaration, Visibility, WireOperation};
use crate::storage::ReadLimits;

use crate::storage::frame::FrameQuery;
use crate::storage::frame::{Collected, Frame};

use crate::storage::codec::{Nested, product};
use crate::storage::{QueryOptions, StorageManager};

use crate::{Error, Result};
use harmos_contract::plugin::Grant;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Reductions;

#[derive(Clone, Debug, PartialEq)]
pub struct ReductionsRequest {
    pub frame: Frame,
    pub page: ReadLimits,
}

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
product!(ReductionsRequest { frame: Nested<Frame> => 1, page: Nested<ReadLimits> => 2 });

impl MemorySize for ReductionsRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(
            self.frame
                .memory_size()
                .saturating_sub(std::mem::size_of::<Frame>()),
        )
    }
}

impl OperationDeclaration for Reductions {
    type Request = ReductionsRequest;
    type Response = Collected;
    const ID: &'static str = "storage.reductions";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    fn grant(request: &ReductionsRequest) -> Option<Grant> {
        let mut schemas = request
            .frame
            .scans
            .iter()
            .map(|scan| scan.selection.schema.as_ref().map(|schema| &schema.id));
        let first = schemas.next().flatten();
        Some(match first {
            Some(id) if schemas.all(|schema| schema == Some(id)) => super::survey(id),
            _ => Grant::Survey(super::EVERY_SCHEMA.into()),
        })
    }
}

impl WireOperation for Reductions {
    const VERSION: u32 = 1;
    const GRPC: Option<Grpc> = Some(super::grpc("Reductions"));
    crate::with_protobuf! {
        fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
            Some(visitor.unary::<Self>())
        }
    }
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = Reductions;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    fn new(storage: Arc<StorageManager<C>>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut ReductionsRequest,
        _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<Collected>> {
        let result: ::core::result::Result<Collected, Self::Error> = async move {
            let query = FrameQuery {
                frame: request.frame.clone(),
                options: QueryOptions {
                    page: Some(request.page),
                    ..QueryOptions::default()
                },
            };
            let mut reader = self.storage.query(query).await?;
            let collected = reader.read().await?;
            Ok(Collected::clone(&collected))
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
