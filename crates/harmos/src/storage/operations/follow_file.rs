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
use crate::operations::{OperationDeclaration, OutputDeclaration, Visibility, WireOperation};
use crate::schema::{Described, Field};
use crate::storage::FileProgress;

use crate::storage::StorageManager;

use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct FollowFile;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for FollowFile {
    type Request = FileSelection;
    type Response = FileProgress;
    const ID: &'static str = "storage.follow_file";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<FileBytes>()))
    }
}
impl OutputDeclaration for FollowFile {
    type Item = FileBytes;
}
impl WireOperation for FollowFile {
    const VERSION: u32 = 1;
    const OUTPUT: Option<&'static [Field]> = Some(FileBytes::FIELDS);
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = FollowFile;
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
    ) -> Result<FileProgress> {
        selection.maximum()?;

        selection.transfer(&self.storage, output, true).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
