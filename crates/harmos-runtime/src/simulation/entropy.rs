/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::future::Future;
use std::sync::{Arc, Mutex};

tokio::task_local! {
    static CURRENT: Context;
}

#[derive(Clone)]
pub(crate) struct Context {
    inner: Arc<Inner>,
}

struct Inner {
    state: Mutex<u64>,
    draws: Mutex<Vec<u64>>,
}

impl Context {
    pub(crate) fn seeded(seed: u64) -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(seed),
                draws: Mutex::new(Vec::new()),
            }),
        }
    }

    pub(crate) async fn scope<F: Future>(&self, future: F) -> F::Output {
        CURRENT.scope(self.clone(), future).await
    }

    pub(crate) fn current() -> Option<Self> {
        CURRENT.try_with(Clone::clone).ok()
    }

    pub(crate) fn trace(&self) -> Vec<u64> {
        self.inner.draws.lock().expect("simulation entropy trace lock is healthy").clone()
    }

    pub(super) fn draw(&self) -> u64 {
        let mut state = self.inner.state.lock().expect("simulation entropy state lock is healthy");
        *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut mixed = *state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^= mixed >> 31;
        drop(state);

        self.inner.draws.lock().expect("simulation entropy trace lock is healthy").push(mixed);
        mixed
    }
}

pub fn entropy() -> u64 {
    CURRENT.try_with(Context::draw).expect("simulation entropy is only available inside simulated work")
}
