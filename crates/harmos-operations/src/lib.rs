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

/* === Re-Exports === */

pub use host::Host;
pub use lifecycle::Lifecycle;
pub use storage::Storage;
pub use streams::Streams;

/* === Enums === */

pub struct Builtins;

/* === Implementations === */

impl Builtins {
    pub fn catalog() -> Vec<harmos_domain::OperationDefinition> {
        let mut definitions = Host::catalog();
        definitions.extend(Lifecycle::catalog());
        definitions.extend(Storage::catalog());
        definitions.extend(Streams::catalog());
        definitions.push(lifecycle::Initialize::definition());
        definitions.push(lifecycle::Shutdown::definition());
        definitions
    }
    #[cfg(feature = "runtime")]
    pub fn registration(dependencies: Dependencies) -> RegistrationBatch {
        let mut batch = Host::registration(dependencies.clone());
        batch.operations.extend(Lifecycle::registration(dependencies.clone()).operations);
        batch.operations.extend(Storage::registration(dependencies.clone()).operations);
        batch.operations.extend(Streams::registration(dependencies.clone()).operations);
        batch.operations.extend(lifecycle::Initialize::registration(dependencies.clone()).operations);
        batch.operations.extend(lifecycle::Shutdown::registration(dependencies).operations);
        batch
    }
}
