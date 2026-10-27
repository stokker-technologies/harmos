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

use harmos_domain::operations::storage::AppendFile;

// Standard Library Imports

use std::sync::Arc;

// Third Party Imports

// Local Imports

use crate::operations::{Context, Operation};
use crate::storage::{FileChunk, FileProducer};

use crate::storage::StorageManager;

use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
    payload: Option<FileChunk>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C: 'static> Operation for Handler<C> {
    type Declaration = AppendFile;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();
    async fn initialize<'ctx>(storage: Arc<StorageManager<C>>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self { storage, payload: None })
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.operation.append_file", level = "trace", skip_all)]
    async fn run<'a>(
        &mut self, _: Self::Context<'a>, request: &mut AppendFile, _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<FileProducer>> {
        if self.payload.is_none() {
            let bytes = std::mem::take(&mut request.bytes);
            self.payload = Some(FileChunk::new(bytes));
        }
        let payload = self.payload.as_ref().expect("file payload").clone();
        let result: ::core::result::Result<FileProducer, Self::Error> = async move {
            self.storage.append_file_once(&request.mutation, &request.session, request.offset, payload).await.map_err(Into::into)
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
