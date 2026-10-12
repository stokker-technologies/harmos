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
use std::sync::Arc;
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos::operations::{Context, OperationPolicy, RetryPolicy};

/* === Constants === */
const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(1),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Read;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos::operation(policy = POLICY, input = u32)]
impl Read {
    fn new(_: Arc<()>) -> Self { Self }
    async fn run(&mut self, _: Context<'_>, _: &(), _: &mut harmos::operations::StreamInput<u64>) -> Result<(), harmos::Error> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn main() {}

/* === Tests === */
