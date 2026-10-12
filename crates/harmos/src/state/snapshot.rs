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
use std::collections::{HashMap, VecDeque};
use std::future::{Future, poll_fn};
use std::num::{NonZeroU64, NonZeroUsize};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::pin;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::task::Poll;
// Third Party Imports
use serde::{Serialize, de::DeserializeOwned};
// Local Imports
use super::snapshot_codec;
use crate::state::{Tail, TrackedState, Undos, dispatch::Dispatch};
use crate::storage::{SchemaKey, StorageManager};
use crate::{
    Application, At, Entry, Error, Incompatible, Payload, Position, Receipt, Recorded, RequestKey,
    Result,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Encoding<A> = fn(
    &SnapshotPolicy,
    &At<<A as Application>::State>,
    &Undos<A>,
    &Tail<A>,
    &HashMap<RequestKey, Receipt>,
) -> Result<Vec<u8>>;
type Decoding<A> = fn(
    &[u8],
    &SnapshotPolicy,
    &Dispatch<A, Payload<A>>,
    usize,
) -> std::result::Result<Restored<A>, Incompatible>;

/* === Structs === */
#[derive(Clone, Debug)]
pub struct SnapshotPolicy {
    pub schema: SchemaKey,
    pub every: NonZeroU64,
    pub max_bytes: NonZeroUsize,
}

pub(crate) struct Snapshots<A: Application> {
    pub(crate) policy: SnapshotPolicy,
    encode: Encoding<A>,
    pub(crate) decode: Decoding<A>,
    last: AtomicU64,
}

pub(crate) struct Restored<A: Application> {
    pub(crate) state: At<A::State>,
    pub(crate) undos: Undos<A>,
    pub(crate) receipts: HashMap<RequestKey, Receipt>,
    pub(crate) tail: VecDeque<Arc<Entry<Recorded<A>>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<A: Application> Snapshots<A> {
    pub(crate) fn new(policy: SnapshotPolicy) -> Self
    where
        A::State: Serialize + DeserializeOwned,
    {
        Self {
            policy,
            encode: snapshot_codec::encode::<A>,
            decode: snapshot_codec::decode::<A>,
            last: AtomicU64::new(0),
        }
    }

    pub(crate) fn resumed(&self, position: Position) {
        self.last.store(position.0, Ordering::Relaxed);
    }

    pub(crate) async fn capture(
        &self,
        storage: &StorageManager<crate::storage::ErasedCatalog>,
        tracked: &TrackedState<A>,
        undos: &Undos<A>,
        receipts: &HashMap<RequestKey, Receipt>,
        final_save: bool,
    ) {
        if tracked.poisoned() {
            return;
        }
        let position = tracked.state.read().await.position;
        let last = self.last.load(Ordering::Relaxed);
        if position.0 <= last || (!final_save && position.0 - last < self.policy.every.get()) {
            return;
        }
        // The journal writer is the only mutator. Reserve before holding readers;
        // no snapshot worker, detached state copy, or second persistence queue.
        let result = async {
            if final_save {
                // Shutdown cannot wait for the periodic saver to free queue space.
                storage.flush().await?;
                storage.flush_recovery().await?;
            }
            let mut sealing = tracked.sealing.subscribe();
            let reservation = storage.reserve_snapshot(self.policy.max_bytes.get());
            let mut permit = if final_save {
                reservation.await?
            } else {
                if *sealing.borrow() {
                    return Ok(());
                }
                let mut reservation = pin!(reservation);
                let mut changed = pin!(sealing.changed());
                let Some(permit) = poll_fn(|cx| {
                    if changed.as_mut().poll(cx).is_ready() {
                        Poll::Ready(None)
                    } else {
                        reservation.as_mut().poll(cx).map(Some)
                    }
                })
                .await
                else {
                    return Ok(());
                };
                permit?
            };
            let state = tracked.state.read().await;
            let tail = tracked.tail.read().await;
            self.last.store(position.0, Ordering::Relaxed);
            let payload = catch_unwind(AssertUnwindSafe(|| {
                (self.encode)(&self.policy, &state, undos, &tail, receipts)
            }))
            .map_err(|_| Error::InvalidInput {
                field: "state snapshot",
                reason: "serializer panicked",
            })??;
            permit.snapshot_payload(payload)?;
            permit.commit(position);
            Ok::<(), Error>(())
        }
        .await;
        if let Err(error) = result {
            tracing::warn!(%error, position = position.0, "snapshot skipped; recovery retains the journal");
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
