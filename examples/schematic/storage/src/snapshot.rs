/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::io;
use std::sync::Arc;

use harmos::{Application, state::Entry, state::Position, state::Recorded, storage::JournalConsumer};

type Sink<A> = Box<dyn FnMut(&<A as Application>::State, Position) -> io::Result<()> + Send>;

pub struct Snapshot<A: Application> {
    replica: A::State,
    taken: Position,
    durable: Position,
    every: u64,
    since: u64,
    complete: Position,
    halted: bool,
    write: Sink<A>,
}

impl<A: Application> Snapshot<A> {
    pub fn folding(origin: A::State, at: Position, write: impl FnMut(&A::State, Position) -> io::Result<()> + Send + 'static) -> Self {
        Self {
            replica: origin,
            taken: at,
            durable: at,
            every: 1,
            since: 0,
            complete: at,
            halted: false,
            write: Box::new(write),
        }
    }

    pub fn every(mut self, entries: u64) -> Self {
        self.every = entries.max(1);
        self
    }
}

impl<A: Application> JournalConsumer<A> for Snapshot<A> {
    fn resume(&self) -> Position {
        self.taken
    }

    async fn drain(&mut self, entries: Vec<Arc<Entry<Recorded<A>>>>, applied: Position) -> Position {
        if self.halted {
            return self.durable;
        }
        for entry in &entries {
            if let Recorded::Transaction(change) = &entry.value {
                if let Some(prepared) = &entry.prepared {
                    if harmos::state::TransactionCatalog::replay(&**change, prepared, &mut self.replica).is_err() {
                        self.halted = true;
                        return self.durable;
                    }
                } else {
                    // Replay Requires The Original Accepted Bytes.
                    self.halted = true;
                    return self.durable;
                }
            }
            self.taken = entry.position;
            self.since += 1;
            if entry.metadata.commit.is_none_or(|(_, end)| end == entry.position) {
                self.complete = entry.position;
            }
        }
        // The source may omit unknown passive records. Its complete applied
        // frontier still proves the containing group is fully available.
        if let Some((_, end)) = entries.last().and_then(|entry| entry.metadata.commit)
            && end <= applied
        {
            self.taken = end;
            self.complete = end;
        }
        if self.complete == self.taken && self.since >= self.every && (self.write)(&self.replica, self.taken).is_ok() {
            self.durable = self.taken;
            self.since = 0;
        }

        self.durable
    }
}
