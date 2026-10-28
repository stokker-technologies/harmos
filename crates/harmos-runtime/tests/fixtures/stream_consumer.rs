/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

extern crate harmos_runtime as harmos;

#[path = "../support/native_streams/plugin.rs"]
mod plugin;

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap();
    runtime.block_on(harmos_runtime::sidecar::run_plugin::<plugin::Receiver>(std::num::NonZeroUsize::new(8).unwrap())).unwrap();
}
