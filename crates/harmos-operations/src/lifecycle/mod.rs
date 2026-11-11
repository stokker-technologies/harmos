/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod cancel;
mod status;
mod watch;

/* === Re-Exports === */

pub use cancel::Cancel;
pub use harmos_domain::model::operations::lifecycle::*;

pub use harmos_protocol::host::InstanceState;
pub use status::Status;
pub use watch::Watch;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

pub const POLICY: harmos_domain::OperationPolicy = harmos_domain::OperationPolicy {
    timeout: std::time::Duration::from_secs(30),
    attempt_timeout: std::time::Duration::from_secs(30),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos_domain::RetryPolicy::Never,
};
const WATCH_POLICY: harmos_domain::OperationPolicy = harmos_domain::OperationPolicy {
    timeout: harmos_domain::operations::UNBOUNDED,
    attempt_timeout: harmos_domain::operations::UNBOUNDED,
    cancellation_timeout: std::time::Duration::ZERO,
    ..POLICY
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

pub fn lifecycle_catalog() -> crate::execution::operations::Catalog {
    lifecycle_catalog_batch().named("harmos.lifecycle")
}

pub fn lifecycle_catalog_batch() -> crate::execution::__private::Catalog {
    use crate::execution::operations::OperationFactory;
    crate::execution::__private::Catalog {
        name: "",
        operations: vec![
            OperationFactory::protobuf::<Status>().expect("status declaration"),
            OperationFactory::protobuf::<Cancel>().expect("cancel declaration"),
            OperationFactory::protobuf::<Watch>().expect("watch declaration"),
        ],
        services: vec![],
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn state(status: harmos_domain::operations::instance::Status) -> InstanceState {
    InstanceState {
        phase: status.phase.as_str().to_owned(),
        outcome: status.outcome.map(harmos_domain::operations::instance::Outcome::as_str).unwrap_or_default().to_owned(),
        cancellation_requested: status.cancellation_requested,
        attempt: status.attempt,
    }
}

/* === Tests === */
