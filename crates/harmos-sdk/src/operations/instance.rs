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

pub use harmos_domain::operations::instance::*;

/* === Imports === */

// Standard Library Imports
use std::future::Future;

// Third Party Imports

// Local Imports
use super::{OperationDeclaration, Response};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

// One submitted operation. Dropping an unsettled instance requests its
// cancellation; `detach` hands the instance to the host and keeps it running.
pub trait Instance<O: OperationDeclaration>: Send + Sized {
    type Error: std::error::Error + Send + Sync + 'static;

    fn id(&self) -> Id;

    fn status(&self) -> impl Future<Output = Result<Status, Self::Error>> + Send;

    fn cancel(&self) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn join(self) -> impl Future<Output = Result<Response<O::Response>, Self::Error>> + Send;

    fn cancel_and_join(self) -> impl Future<Output = Result<Response<O::Response>, Self::Error>> + Send {
        async move {
            self.cancel().await?;
            self.join().await
        }
    }

    fn detach(self) -> Id;
}

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
