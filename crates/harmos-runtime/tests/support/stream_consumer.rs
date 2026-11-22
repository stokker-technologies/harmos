/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Stream Consumer Fixture

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// Third Party Imports
use harmos::plugin::{Context, HostOperation, Operation, Plugin, SubscriptionItem};
use harmos::{Message, Visibility};
use tokio::sync::Notify;

// Local Imports

/* === Structs === */

#[derive(Message)]
pub struct Payload {
    #[harmos(tag = 474216250)]
    pub sequence: u64,
    #[harmos(tag = 95543208)]
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
    ready: AtomicBool,
    changed: Notify,
}

/* === Public Functions === */

pub fn plugin() -> Plugin {
    let state = Arc::new(State::default());
    let receiver = state.clone();
    let consume = Operation::new("consumption.consume", async move |context: &Context, _: Progress| {
        let mut rows = context.subscribe::<Payload>("consumption.payload", NonZeroUsize::new(1024 * 1024).unwrap()).await?;
        receiver.ready.store(true, Ordering::Release);
        receiver.changed.notify_waiters();
        while let Some(item) = rows.next().await? {
            let SubscriptionItem::Row(row) = item else {
                return Err(std::io::Error::other("Read Host Stream: Increase the Configured Window to Retain Every Row").into());
            };
            let row = row.value;
            let mut totals = receiver.totals.lock().unwrap();
            if row.sequence != totals.0 || !row.bytes.iter().all(|byte| *byte == (totals.0 % 251) as u8) {
                return Err(std::io::Error::other("Publish Consecutive Rows With Matching Payload Bytes").into());
            }
            totals.0 += 1;
            totals.1 += row.bytes.len() as u64;
            receiver.changed.notify_waiters();
        }
        Ok(())
    })
    .visibility(Visibility::Public);
    let progress = Operation::new("consumption.progress", async move |_: &Context, request: Progress| {
        loop {
            let changed = state.changed.notified();
            let (rows, bytes) = *state.totals.lock().unwrap();
            if state.ready.load(Ordering::Acquire) && rows >= request.expected {
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
    Plugin::new("consumption", env!("CARGO_PKG_VERSION"))
        .register(|ctx| {
            ctx.register(HostOperation::SubscriptionsSubscribe)?;
            ctx.register(consume)?;
            ctx.register(progress)?;
            Ok(())
        })
        .expect("Register Plugin Declarations")
}
