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

#[cfg(feature = "plugin-sidecar")]
pub(crate) struct Lease {
    transports: Transports,
    key: Key,
    poller: Poller,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Driving {
    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn retain(
        &self, owner: String, generation: PluginGeneration, poll: impl Fn(&mut Context<'_>) + Send + Sync + 'static,
    ) -> Lease {
        let key = (owner, generation);
        let poller: Poller = Arc::new(poll);
        let mut transports = self.transports.lock().expect("plugin transport owner is healthy");
        assert!(!transports.contains_key(&key), "plugin generation has one physical transport");
        transports.insert(key.clone(), poller.clone());
        Lease { transports: self.transports.clone(), key, poller }
    }

    pub(crate) fn tick(&self, context: &mut Context<'_>) {
        let pollers: Vec<_> = self.transports.lock().expect("plugin transport owner is healthy").values().cloned().collect();
        for poll in pollers {
            poll(context);
        }
    }
}

#[cfg(feature = "plugin-sidecar")]
impl Drop for Lease {
    fn drop(&mut self) {
        let mut transports = self.transports.lock().expect("plugin transport owner is healthy");
        if transports.get(&self.key).is_some_and(|poller| Arc::ptr_eq(poller, &self.poller)) {
            transports.remove(&self.key);
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
