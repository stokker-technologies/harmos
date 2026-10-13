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
use super::{FileBytes, FileSelection};

use crate::operations::{Context, Operation, StreamOutput};
use crate::operations::{Grpc, OperationDeclaration, OutputDeclaration, Visibility, WireOperation};
use crate::schema::Described;
use crate::storage::FileProgress;

use crate::storage::StorageManager;

use crate::{Error, Result};
use harmos_contract::plugin::Grant;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct ReadFiles;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for ReadFiles {
    type Request = FileSelection;
    type Response = FileProgress;
    const ID: &'static str = "storage.read_files";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    fn grant(selection: &FileSelection) -> Option<Grant> {
        Some(super::survey(&selection.file.id))
    }

    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<FileBytes>()))
    }
}
impl OutputDeclaration for ReadFiles {
    type Item = FileBytes;
}
impl WireOperation for ReadFiles {
    const VERSION: u32 = 1;
    const GRPC: Option<Grpc> = Some(super::grpc("ReadFiles"));
    crate::with_protobuf! {
        fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
            Some(visitor.download::<Self>())
        }
    }
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(FileBytes::DEFINITION);
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = ReadFiles;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<FileBytes>;
    fn new(storage: Arc<StorageManager<C>>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        selection: &mut FileSelection,
        output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<FileProgress>> {
        let result: ::core::result::Result<FileProgress, Self::Error> = async move {
            selection.maximum()?;

            selection.transfer(&self.storage, output, false).await
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
