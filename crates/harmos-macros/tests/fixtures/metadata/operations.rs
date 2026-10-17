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
use std::time::Duration;
// Third Party Imports

// Local Imports

/* === Constants === */
pub const SDK_POLICY: harmos::operations::OperationPolicy = harmos::operations::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    retries: harmos::operations::RetryPolicy::Never,
    restart: harmos::operations::RestartPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
#[harmos::operation(
    id = "selected.read",
    version = 3,
    response = Response,
    description = "Selected \"reading\" μ",
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Idempotent,
    policy = harmos::operations::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_nanos(17),
        retries: harmos::operations::RetryPolicy::OnTransient {
            max_retries: 3,
            backoff: harmos::operations::Backoff::Fixed {
                delay: Duration::from_nanos(7),
                jitter: harmos::operations::Jitter::Deterministic,
            },
        },
        restart: harmos::operations::RestartPolicy::Never,
    },
)]
pub struct Selected {
    value: u32,
}

#[harmos::message]
#[harmos::operation(
    id = "unselected.read",
    version = 1,
    response = Response,
    policy = <Selected as harmos::operations::OperationDeclaration>::POLICY,
)]
pub struct Unselected {
    value: u32,
}

#[harmos::message]
pub struct Response {
    value: u32,
}

/* === Enums === */
#[harmos::catalog]
pub enum Operations {
    Selected(Selected),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
