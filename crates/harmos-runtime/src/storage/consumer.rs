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
use std::any::Any;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
// Third Party Imports

// Local Imports
use crate::{Application, state::At, state::Entry, state::Position, state::Recorded};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cursor(pub u64);

pub struct Batch<A: Application> {
    pub(crate) rows: Vec<(Cursor, crate::streams::Definition, Position, Arc<dyn Any + Send + Sync>)>,
    pub(crate) reaches: Cursor,
    pub(crate) incomplete: Vec<crate::streams::Definition>,
    pub(crate) application: PhantomData<fn() -> A>,
}

/* === Enums === */

/* === Traits === */

pub trait JournalConsumer<A: Application>: Send + 'static {
    fn resume(&self) -> Position;

    fn drain(&mut self, entries: Vec<Arc<Entry<Recorded<A>>>>, applied: Position) -> impl Future<Output = Position> + Send;

    fn flush(&mut self) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn redelivered(&mut self, stored: &[(Position, Arc<Vec<u8>>)]) {
        let _ = stored;
    }
}

pub(crate) trait Attached<A: Application>: Send {
    fn resume(&self) -> Position;

    fn drain(&mut self, entries: Vec<Arc<Entry<Recorded<A>>>>, applied: Position) -> Pin<Box<dyn Future<Output = Position> + Send + '_>>;

    fn flush(&mut self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    fn redelivered(&mut self, stored: &[(Position, Arc<Vec<u8>>)]);
}

pub trait StreamConsumer<A: Application>: Send + 'static {
    fn resume(&self) -> Cursor;

    fn selections(&self) -> Vec<crate::streams::Definition> {
        Vec::new()
    }

    fn drain(&mut self, batch: Batch<A>) -> impl Future<Output = Cursor> + Send;

    fn flush(&mut self) -> impl Future<Output = ()> + Send {
        async {}
    }
}

pub(crate) trait Recording<A: Application>: Send {
    fn resume(&self) -> Cursor;

    fn selections(&self) -> Vec<crate::streams::Definition>;

    fn drain(&mut self, batch: Batch<A>) -> Pin<Box<dyn Future<Output = Cursor> + Send + '_>>;

    fn flush(&mut self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

/* === Implementations === */

impl<A: Application, C: JournalConsumer<A>> Attached<A> for C {
    fn resume(&self) -> Position {
        JournalConsumer::resume(self)
    }

    fn drain(&mut self, entries: Vec<Arc<Entry<Recorded<A>>>>, applied: Position) -> Pin<Box<dyn Future<Output = Position> + Send + '_>> {
        Box::pin(JournalConsumer::drain(self, entries, applied))
    }

    fn flush(&mut self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(JournalConsumer::flush(self))
    }

    fn redelivered(&mut self, stored: &[(Position, Arc<Vec<u8>>)]) {
        JournalConsumer::redelivered(self, stored);
    }
}

impl Cursor {
    pub const ORIGIN: Self = Self(0);
}

impl<A: Application> Batch<A> {
    pub fn reaches(&self) -> Cursor {
        self.reaches
    }

    pub fn incomplete(&self) -> &[crate::streams::Definition] {
        &self.incomplete
    }

    pub fn publications(&self) -> impl Iterator<Item = (&crate::streams::Definition, Cursor, At<&crate::streams::PublicationRow>)> + '_ {
        self.rows.iter().filter_map(|(cursor, definition, position, row)| {
            definition.owner()?;
            let payload = crate::streams::row_value::<crate::streams::PublicationRow>(row.as_ref())?;
            Some((definition, *cursor, At { value: payload, position: *position }))
        })
    }

    pub fn select<S: crate::streams::StreamRow>(&self) -> impl Iterator<Item = At<&S>> + '_ {
        self.rows.iter().filter_map(|(_, definition, position, row)| {
            if definition.id() != S::ID || definition.owner().is_some() {
                return None;
            }

            row.downcast_ref::<S>().map(|value| At { value, position: *position })
        })
    }

    pub fn select_shared<S: crate::streams::StreamRow>(&self) -> impl Iterator<Item = At<Arc<S>>> + '_ {
        self.rows.iter().filter_map(|(_, definition, position, row)| {
            if definition.id() != S::ID || definition.owner().is_some() {
                return None;
            }
            Arc::clone(row).downcast::<S>().ok().map(|value| At { value, position: *position })
        })
    }
}

impl<A: Application, C: StreamConsumer<A>> Recording<A> for C {
    fn resume(&self) -> Cursor {
        StreamConsumer::resume(self)
    }

    fn selections(&self) -> Vec<crate::streams::Definition> {
        StreamConsumer::selections(self)
    }

    fn drain(&mut self, batch: Batch<A>) -> Pin<Box<dyn Future<Output = Cursor> + Send + '_>> {
        Box::pin(StreamConsumer::drain(self, batch))
    }

    fn flush(&mut self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(StreamConsumer::flush(self))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
