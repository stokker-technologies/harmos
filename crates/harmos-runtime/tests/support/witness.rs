/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Witness In-Process Plugin Events: Plugins Mark Named Events, Stories Await Them
#![allow(dead_code)]

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

// Third Party Imports
use tokio::sync::watch;

// Local Imports

/* === Constants === */
static NEXT: AtomicUsize = AtomicUsize::new(1);

static LEDGERS: Mutex<BTreeMap<String, Weak<Witness>>> = Mutex::new(BTreeMap::new());

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// One Story's Event Ledger, Reachable By Key From A Plugin's Serialized Configuration.
pub struct Witness {
    key: String,
    events: watch::Sender<BTreeMap<String, usize>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Witness {
    pub fn open() -> Arc<Self> {
        let key = format!("witness-{}", NEXT.fetch_add(1, Ordering::SeqCst));
        let witness = Arc::new(Self { key: key.clone(), events: watch::Sender::new(BTreeMap::new()) });
        LEDGERS.lock().expect("the ledger registry is never poisoned").insert(key, Arc::downgrade(&witness));
        witness
    }

    pub fn find(key: &str) -> Option<Arc<Self>> {
        LEDGERS.lock().expect("the ledger registry is never poisoned").get(key).and_then(Weak::upgrade)
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn mark(&self, name: &str) {
        self.events.send_modify(|events| {
            *events.entry(name.to_owned()).or_default() += 1;
        });
    }

    pub fn count(&self, name: &str) -> usize {
        self.events.borrow().get(name).copied().unwrap_or_default()
    }

    pub fn seen(&self, name: &str) -> bool {
        self.count(name) > 0
    }

    pub async fn marked(&self, name: &str) {
        let mut events = self.events.subscribe();
        events.wait_for(|events| events.contains_key(name)).await.expect("the witness owns its sender");
    }
}

impl Drop for Witness {
    fn drop(&mut self) {
        if let Ok(mut ledgers) = LEDGERS.lock() {
            ledgers.remove(&self.key);
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
