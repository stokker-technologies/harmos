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
use std::sync::{Arc, Mutex};

// Third Party Imports
use harmos::{Context, Message, Operation, Plugin, Visibility};
use tokio::sync::Notify;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Message)]
pub struct Payload {
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

#[derive(Message)]
pub struct Progress {
    pub expected: u64,
}

#[derive(Message)]
pub struct Acknowledged {
    pub process_id: u32,
    pub rows: u64,
    pub bytes: u64,
}

#[derive(Default)]
struct State {
    totals: Mutex<(u64, u64)>,
    changed: Notify,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn plugin() -> Plugin {
    let state = Arc::new(State::default());
    let receiver = state.clone();
    let consume = Operation::new("consumption.consume", async move |_: &Context, row: Payload| {
        let mut totals = receiver.totals.lock().unwrap();
        if row.sequence != totals.0 || !row.bytes.iter().all(|byte| *byte == (totals.0 % 251) as u8) {
            return Err(std::io::Error::other("Payload Loss, Reordering or Corruption").into());
        }
        totals.0 += 1;
        totals.1 += row.bytes.len() as u64;
        receiver.changed.notify_waiters();
        Ok(())
    })
    .visibility(Visibility::Public);
    let progress = Operation::new("consumption.progress", async move |_: &Context, request: Progress| {
        loop {
            let changed = state.changed.notified();
            let (rows, bytes) = *state.totals.lock().unwrap();
            if rows >= request.expected {
                return Ok(Acknowledged {
                    process_id: std::process::id(),
                    rows,
                    bytes,
                });
            }
            changed.await;
        }
    })
    .visibility(Visibility::Public);
    Plugin::new("consumption", env!("CARGO_PKG_VERSION")).operation(consume).operation(progress)
}

/* === Private Functions === */

/* === Tests === */
