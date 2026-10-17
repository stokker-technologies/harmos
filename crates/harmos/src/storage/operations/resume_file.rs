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
use harmos_domain::operations::storage::ResumeFile;

// Standard Library Imports

use std::sync::Arc;

// Third Party Imports

// Local Imports

use crate::operations::{Context, Operation};
use crate::storage::FileProducer;

use crate::storage::StorageManager;

use crate::{Error, Result};

/* === Constants === */

/* === Macros === */
/* === Type Aliases === */

/* === Structs === */

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

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
        request: &mut ResumeFile,
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

    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
