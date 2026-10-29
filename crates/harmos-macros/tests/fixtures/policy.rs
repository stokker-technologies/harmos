/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


use std::time::Duration;

use harmos::operations::{OperationPolicy, RetryPolicy};


pub const POLICY: OperationPolicy =
    OperationPolicy { timeout: Duration::from_secs(1), attempt_timeout: Duration::from_secs(1), retries: RetryPolicy::Never };
