/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Portable Operation Cancellation Signal

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::{Arc, Mutex, Weak};
use std::task::{Poll, Waker};

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// A Sticky, Executor-Independent Signal Shared By One Operation Attempt.
#[derive(Clone, Default)]
pub struct Cancellation(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    requested: bool,
    waiters: Vec<Weak<Mutex<Option<Waker>>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Cancellation {
    pub fn is_requested(&self) -> bool {
        self.0.lock().expect("cancellation lock").requested
    }

    #[doc(hidden)]
    pub fn request(&self) {
        let waiters = {
            let mut state = self.0.lock().expect("cancellation lock");
            state.requested = true;
            std::mem::take(&mut state.waiters)
        };
        for waiter in waiters.into_iter().filter_map(|waiter| waiter.upgrade()) {
            let waker = waiter.lock().expect("cancellation waiter lock").take();
            if let Some(waker) = waker {
                waker.wake();
            }
        }
    }

    pub async fn cancelled(&self) {
        let waiter = Arc::new(Mutex::new(None));
        std::future::poll_fn(|context| {
            let mut state = self.0.lock().expect("cancellation lock");
            if state.requested {
                return Poll::Ready(());
            }
            *waiter.lock().expect("cancellation waiter lock") = Some(context.waker().clone());
            state.waiters.retain(|entry| entry.strong_count() != 0);
            if !state.waiters.iter().any(|entry| entry.ptr_eq(&Arc::downgrade(&waiter))) {
                state.waiters.push(Arc::downgrade(&waiter));
            }
            Poll::Pending
        })
        .await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
