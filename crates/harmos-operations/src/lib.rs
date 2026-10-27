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
#[cfg(feature = "runtime")]
mod error;
pub mod host;
pub mod lifecycle;
pub mod storage;
pub mod streams;

pub mod operations {
    pub use harmos_domain::operations::*;
    #[cfg(feature = "lifecycle")]
    pub use harmos_sdk::operations::Executable;
}

/* === Re-Exports === */

#[cfg(feature = "runtime")]
pub use error::Error;
pub use harmos_domain::*;
#[cfg(feature = "lifecycle")]
pub use harmos_sdk::{
    Context, Dependencies, Dependent, Executable, Handler, Operation, ExecutionContext, RegistrationBatch, Resource, ResourceError,
    Response,
};
pub use host::Host;
pub use lifecycle::Lifecycle;
pub use storage::Storage;
pub use streams::Streams;

/* === Imports === */

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

#[cfg(feature = "runtime")]
pub type Result<T> = std::result::Result<T, Error>;

/* === Structs === */

pub struct Builtins;

/* === Enums === */

/* === Traits === */

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
        batch.operations.extend(harmos_sdk::RegistrationBatch::new(dependencies.clone()).operation::<lifecycle::Initialize>().operations);
        batch.operations.extend(harmos_sdk::RegistrationBatch::new(dependencies).operation::<lifecycle::Shutdown>().operations);
        batch
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
