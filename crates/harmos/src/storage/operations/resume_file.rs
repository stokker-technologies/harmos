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
use crate::storage::FileProducer;

use crate::storage::StorageManager;

use crate::{Error, Result};

/* === Constants === */

/* === Macros === */
/* === Type Aliases === */

/* === Structs === */
pub struct ResumeFile;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for ResumeFile {
    type Request = FileMutation;
    type Response = FileProducer;
    const ID: &'static str = "storage.resume_file";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}
impl WireOperation for ResumeFile {
    const VERSION: u32 = 1;
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = ResumeFile;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();
    async fn initialize<'ctx>(
        storage: Arc<StorageManager<C>>,
        _: Self::Context<'ctx>,
    ) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut FileMutation,
        _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<FileProducer>> {
        let result: ::core::result::Result<FileProducer, Self::Error> = async move {
            self.storage
                .resume_file_once(&request.mutation, &request.session)
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
