/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

extern crate self as harmos;
pub mod host;
pub mod lifecycle;
pub mod storage;
pub mod streams;
#[cfg(feature = "runtime")]
pub type Result<T> = std::result::Result<T, Error>;
pub use harmos_domain::*;
#[cfg(feature = "runtime")]
pub use harmos_sdk::{Dependencies, DependencyError, Operation, OperationContext, RegistrationBatch, Response};
#[cfg(feature = "runtime")]
mod error;
#[cfg(feature = "runtime")]
pub use error::Error;
