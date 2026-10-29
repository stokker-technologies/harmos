/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[cfg(feature = "sidecar")]
#[path = "../build/main.rs"]
mod build;
#[cfg(feature = "generate-proto")]
#[path = "../proto/main.rs"]
mod proto;

#[cfg(feature = "generate-proto")]
extern crate harmos_domain as harmos;
