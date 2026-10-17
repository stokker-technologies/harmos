/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// The Closed Catalog Of Authored Operations.

/* === Modules === */
pub mod greeting;


/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports


/* === Constants === */
pub const POLICY: harmos_domain::operations::OperationPolicy = harmos_domain::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    retries: harmos_domain::operations::RetryPolicy::Never,
    restart: harmos_domain::operations::RestartPolicy::Never,
};


/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[harmos_domain::catalog]
pub enum Operations { Greet(greeting::Greet) }


/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
