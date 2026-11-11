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

pub struct Lifecycle;

/* === Traits === */

/* === Implementations === */

impl Lifecycle {
    pub const CATALOG: crate::execution::operations::Catalog = <Self as crate::execution::operations::OperationCatalog>::CATALOG;
    pub fn catalog() -> Vec<harmos_domain::OperationDefinition> {
        <Self as crate::execution::operations::OperationCatalog>::batch().operations.into_iter().map(|factory| factory.definition).collect()
    }
}
impl crate::execution::operations::OperationCatalog for Lifecycle {
    const CATALOG: crate::execution::operations::Catalog = crate::execution::operations::Catalog::of::<Self>("harmos.lifecycle", &[], &[]);
    fn batch() -> crate::execution::__private::RegistrationBatch {
        use crate::execution::operations::OperationFactory;
        crate::execution::__private::RegistrationBatch {
            operations: vec![
                OperationFactory::protobuf::<Status>().expect("status declaration"),
                OperationFactory::protobuf::<Cancel>().expect("cancel declaration"),
                OperationFactory::protobuf::<Watch>().expect("watch declaration"),
            ],
            services: vec![],
        }
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
