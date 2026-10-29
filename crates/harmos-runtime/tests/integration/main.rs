/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Each Domain Keeps Its Original Support Namespace, So Shared Support Files Load Once Per Domain.
#![allow(clippy::duplicate_mod, reason = "domains retain their own support namespaces under one binary")]

extern crate harmos_runtime as harmos;

/* === Modules === */

#[path = "../application/main.rs"]
mod application;
#[path = "../observers/main.rs"]
mod observers;
#[path = "../operations/main.rs"]
mod operations;
#[cfg(feature = "plugin")]
#[path = "../plugin/main.rs"]
mod plugin;
mod report;
#[path = "../runtime/main.rs"]
mod runtime;
#[path = "../services/main.rs"]
mod services;
#[cfg(feature = "plugin-sidecar")]
#[path = "../sidecar/main.rs"]
mod sidecar;
#[cfg(feature = "simulation")]
#[path = "../simulation/main.rs"]
mod simulation;
#[path = "../state/main.rs"]
mod state;
#[path = "../storage/main.rs"]
mod storage;
#[path = "../streams/main.rs"]
mod streams;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
