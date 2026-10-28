/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod throughput_protocol;

use std::num::NonZeroUsize;

use harmos_sidecar::run_plugin;
use throughput_protocol::TransportPlugin;

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    run_plugin::<TransportPlugin>(NonZeroUsize::new(8).unwrap()).await
}
