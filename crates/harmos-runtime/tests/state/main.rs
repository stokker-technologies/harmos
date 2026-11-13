/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../state/managed.rs"]
mod managed;
use crate::support;

#[path = "capture.rs"]
mod capture;

#[path = "consumers.rs"]
mod consumers;

#[path = "../state/durability.rs"]
mod durability;

#[path = "idempotency.rs"]
mod idempotency;

#[path = "ordering.rs"]
mod ordering;

#[path = "poison.rs"]
mod poison;

#[path = "records.rs"]
mod records;

#[cfg(feature = "plugin-guest")]
#[path = "../state/listeners.rs"]
mod listeners;

#[path = "undo.rs"]
mod undo;
mod undo_redo;
