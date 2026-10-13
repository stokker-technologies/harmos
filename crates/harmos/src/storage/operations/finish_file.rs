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
use super::FileMutation;

use crate::operations::{Context, Operation};
use crate::operations::{OperationDeclaration, ReplaySafety, Visibility, WireOperation};
use crate::storage::FileReference;

use crate::storage::StorageManager;

use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct FinishFile;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for FinishFile {
    type Request = FileMutation;
    type Response = FileReference;
    const ID: &'static str = "storage.finish_file";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}
impl WireOperation for FinishFile {
    const VERSION: u32 = 1;
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = FinishFile;
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
        request: &mut FileMutation,
        _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<FileReference>> {
        let result: ::core::result::Result<FileReference, Self::Error> = async move {
            self.storage
                .finish_file_once(&request.mutation, &request.session)
                .await
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    fn classify(error: &Error) -> crate::operations::Failure {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
