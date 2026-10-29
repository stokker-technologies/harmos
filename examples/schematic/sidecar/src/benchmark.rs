/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Select The Sidecar Catalog And Its Resources.

pub(super) const CATALOG: harmos::Catalog = schematic_operations::transfer::Native::CATALOG;

pub(super) fn prepare(setup: &mut harmos::Setup<super::Plugin>) {
    let transfer = std::sync::Arc::new(schematic_operations::transfer::Store::default());
    setup.resource::<schematic_operations::transfer::Store>(transfer);
}
