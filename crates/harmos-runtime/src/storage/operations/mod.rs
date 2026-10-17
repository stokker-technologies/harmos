/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod abandon_file;
mod append_file;
mod create_file;
mod delete_file;
mod finish_file;
mod follow_file;
mod follow_objects;
mod follow_records;
mod follow_stream;
mod query;
mod read_file;
mod resume_file;
mod storage_status;
mod write_file;
mod write_objects;
mod write_records;
mod write_schema;
mod write_streams;

/* === Re-Exports === */
pub use harmos_domain::operations::storage::*;

/* === Imports === */

// Standard Library Imports

use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::catalog::{Catalog, Member, Visitor};

use crate::operations::OperationManager;

use crate::storage::StorageManager;

use crate::{Result, operations::Lane};

/* === Constants === */
const IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/* === Macros === */
// Bind Each Declaration To Its Original Storage Handler.
macro_rules! provider {
    ($($module:ident),* $(,)?) => {
        impl<C: 'static, V> Catalog<V> for Provider<C>
        where V: Visitor $(+ Member<$module::Handler<C>>)* {
            type Entries = [V::Entry; [$(stringify!($module)),*].len()];
            const ENTRIES: Self::Entries = [$(<V as Member<$module::Handler<C>>>::ENTRY),*];
        }
    };
}

provider! {
    query,
    read_file,
    follow_records,
    follow_stream,
    follow_file,
    storage_status,
    resume_file,
    create_file,
    append_file,
    finish_file,
    abandon_file,
    write_records,
    write_streams,
    write_file,
    delete_file,
    write_schema,
    write_objects,
    follow_objects,
}

/* === Type Aliases === */

/* === Structs === */

struct Provider<C>(std::marker::PhantomData<fn() -> C>);

/* === Enums === */
/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
// Serve The Storage Operations Over One Storage Manager.
pub fn register<C: 'static>(
    operations: &OperationManager,
    owner: &str,
    lane: impl Lane,
    storage: Arc<StorageManager<C>>,
) -> Result<()> {
    operations.register_wire_operations::<Provider<C>, _>(owner, lane, storage)
}

/* === Private Functions === */
fn output_timeout() -> crate::Error {
    crate::Error::backend(
        "storage output",
        crate::BackendFailure::Transient,
        std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "consumer delivery deadline exceeded",
        ),
    )
}

fn classify(error: &crate::Error) -> crate::operations::FailureKind {
    let mut cause = error;
    while let crate::Error::Persistence { source } = cause {
        cause = source;
    }
    match cause {
        crate::Error::Backend {
            kind: crate::BackendFailure::Transient,
            ..
        } => crate::operations::FailureKind::Transient,
        _ => crate::operations::FailureKind::Permanent,
    }
}

/* === Tests === */
