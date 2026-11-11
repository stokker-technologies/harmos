/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Progress Retained Native Transports While Plugin Lifecycle Futures Wait.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::task::Context;

// Third Party Imports

// Local Imports
use crate::plugin::PluginGeneration;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Key = (String, PluginGeneration);
type Poller = Arc<dyn Fn(&mut Context<'_>) + Send + Sync>;
type Transports = Arc<Mutex<BTreeMap<Key, Poller>>>;

/* === Structs === */

#[derive(Clone, Default)]
pub(crate) struct Driving {
    transports: Transports,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Driving {
    pub(crate) fn tick(&self, context: &mut Context<'_>) {
        let pollers: Vec<_> = self.transports.lock().expect("plugin transport owner is healthy").values().cloned().collect();
        for poll in pollers {
            poll(context);
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
