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
use crate::storage::state::SnapshotPayload;
use crate::storage::{SchemaKey, StorageManager};
use crate::{
    Application, Error, Result, runtime::Incompatible, state::At, state::Entry, state::Position, state::Receipt, state::Recorded,
    state::RequestKey, storage::Payload,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Encoding<A> = fn(
    &SnapshotPolicy,
    &Dispatch<A, Payload<A>>,
    &At<<A as Application>::State>,
    &Undos<A>,
    &Tail<A>,
    &HashMap<RequestKey, Receipt>,
) -> Result<SnapshotPayload>;
type Decoding<A> = fn(&SnapshotPayload, &SnapshotPolicy, &Dispatch<A, Payload<A>>, usize) -> std::result::Result<Restored<A>, Incompatible>;

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
    last_sequence: AtomicU64,
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
            last_sequence: AtomicU64::new(0),
        }
    }

    pub(crate) fn resumed(&self, position: Position, sequence: u64) {
        self.last.store(position.0, Ordering::Relaxed);
        self.last_sequence.store(sequence, Ordering::Relaxed);
    }

    #[tracing::instrument(target = "harmos::state", name = "state.capture", level = "debug", skip_all)]
    pub(crate) async fn capture(
        &self, storage: &StorageManager, tracked: &TrackedState<A>, undos: &Undos<A>, receipts: &HashMap<RequestKey, Receipt>,
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
        let mut size_bytes = None;
        let result = async {
            if final_save {
                // Shutdown cannot wait for the periodic saver to free queue space.
                storage.flush().await?;
                storage.flush_recovery().await?;
            }
            let mut sealing = tracked.sealing.subscribe();
            let reservation = storage.reserve_snapshot(self.policy.max_bytes.get());
            let mut permit =
                if final_save {
                    reservation.await?
                } else {
                    if *sealing.borrow() {
                        return Ok(());
                    }
                    let mut reservation = pin!(reservation);
                    let mut changed = pin!(sealing.changed());
                    let Some(permit) = poll_fn(|cx| {
                        if changed.as_mut().poll(cx).is_ready() { Poll::Ready(None) } else { reservation.as_mut().poll(cx).map(Some) }
                    })
                    .await
                    else {
                        return Ok(());
                    };
                    permit?
                };
            let state = tracked.state.read().await;
            let tail = tracked.tail.read().await;
            let payload = catch_unwind(AssertUnwindSafe(|| (self.encode)(&self.policy, &tracked.catalog, &state, undos, &tail, receipts)))
                .map_err(|_| Error::InvalidInput {
                    field: "state snapshot",
                    reason: "serializer panicked",
                })??;
            drop(tail);
            drop(state);
            size_bytes = Some(payload.len());
            permit.snapshot_payload(payload)?;
            permit.commit(position);
            storage.flush_recovery().await?;
            self.resumed(position, storage.status().recovery_accepted.sequence);
            Ok::<(), Error>(())
        }
        .await;
        if let Err(error) = result {
            if let Error::SnapshotEntry { bytes, .. } = &error {
                size_bytes = Some(*bytes);
            }
            let error = harmos_operations::failure::Report(&error);
            let replay_entries = storage.status().recovery_accepted.sequence.saturating_sub(self.last_sequence.load(Ordering::Relaxed));
            tracing::warn!(%error, position = position.0, size_bytes = size_bytes.unwrap_or(0), size_known = size_bytes.is_some(), segment_limit = self.policy.max_bytes.get(), replay_entries,
                "snapshot skipped; the next boot replays {} journal entries", replay_entries);
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
