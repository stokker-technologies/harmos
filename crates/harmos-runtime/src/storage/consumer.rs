/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::any::Any;
use std::future::Future;
use std::marker::PhantomData;
use std::ops::Bound;
use std::pin::Pin;
use std::sync::Arc;

use crate::{Application, state::At, state::Entry, state::Position, state::Recorded};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cursor(pub u64);

pub struct Batch<A: Application> {
    pub(crate) rows: Vec<(Cursor, crate::streams::Definition, Position, Arc<dyn Any + Send + Sync>)>,
    pub(crate) reaches: Cursor,
    pub(crate) incomplete: Vec<crate::streams::Definition>,
    pub(crate) application: PhantomData<fn() -> A>,
}

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

pub trait Series<A: Application>: StreamConsumer<A> {
    fn reader(
        &mut self, _stream: crate::streams::Definition, _track: &[String], _start: Bound<Position>, _end: Bound<Position>,
        _limits: crate::streams::ReadLimits,
    ) -> impl Future<Output = Result<Box<dyn crate::streams::RowSource>, crate::streams::ReadError>> + Send {
        async { Err(crate::streams::ReadError::Unsupported) }
    }

    // Rows are spelled out rather than aliased: the census is the list of
    // names an author learns, and an erased row belongs to this signature
    // rather than beside them.
    #[allow(clippy::type_complexity)]
    fn rows(
        &mut self, stream: crate::streams::Definition, track: &[String], start: Bound<Position>, end: Bound<Position>,
    ) -> impl Future<Output = Result<Vec<At<Arc<dyn Any + Send + Sync>>>, crate::streams::BeneathWindow>> + Send;

    fn buckets(
        &mut self, stream: crate::streams::Definition, track: &[String], start: Bound<u64>, end: Bound<u64>, budget: usize,
    ) -> impl Future<Output = Result<Vec<crate::streams::Bucket>, crate::streams::BeneathWindow>> + Send;

    fn aggregate(
        &mut self, stream: crate::streams::Definition, track: &[String], start: Bound<u64>, end: Bound<u64>, kind: crate::Aggregate,
    ) -> impl Future<Output = Result<Vec<crate::streams::Aggregated>, crate::streams::BeneathWindow>> + Send;

    fn seal(&mut self, stream: crate::streams::Definition, track: &[String]) -> impl Future<Output = Position> + Send;
}

pub(crate) trait Recording<A: Application>: Send {
    fn resume(&self) -> Cursor;

    fn selections(&self) -> Vec<crate::streams::Definition>;

    fn drain(&mut self, batch: Batch<A>) -> Pin<Box<dyn Future<Output = Cursor> + Send + '_>>;

    fn flush(&mut self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

pub(crate) trait Serving<A: Application>: Recording<A> {
    #[allow(clippy::type_complexity)]
    fn reader<'read>(
        &'read mut self, stream: crate::streams::Definition, track: &'read [String], start: Bound<Position>, end: Bound<Position>,
        limits: crate::streams::ReadLimits,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn crate::streams::RowSource>, crate::streams::ReadError>> + Send + 'read>>;

    #[allow(clippy::type_complexity)]
    fn rows<'read>(
        &'read mut self, stream: crate::streams::Definition, track: &'read [String], start: Bound<Position>, end: Bound<Position>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<At<Arc<dyn Any + Send + Sync>>>, crate::streams::BeneathWindow>> + Send + 'read>>;

    #[allow(clippy::type_complexity)]
    fn buckets<'read>(
        &'read mut self, stream: crate::streams::Definition, track: &'read [String], start: Bound<u64>, end: Bound<u64>, budget: usize,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<crate::streams::Bucket>, crate::streams::BeneathWindow>> + Send + 'read>>;

    #[allow(clippy::type_complexity)]
    fn aggregate<'read>(
        &'read mut self, stream: crate::streams::Definition, track: &'read [String], start: Bound<u64>, end: Bound<u64>,
        kind: crate::Aggregate,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<crate::streams::Aggregated>, crate::streams::BeneathWindow>> + Send + 'read>>;

    fn seal<'read>(
        &'read mut self, stream: crate::streams::Definition, track: &'read [String],
    ) -> Pin<Box<dyn Future<Output = Position> + Send + 'read>>;
}

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

    pub fn select<S: crate::streams::Stream<A>>(&self) -> impl Iterator<Item = At<&S>> + '_ {
        self.rows.iter().filter_map(|(_, definition, position, row)| {
            if definition.id() != S::ID || definition.version() != S::VERSION {
                return None;
            }

            row.downcast_ref::<S>().map(|value| At { value, position: *position })
        })
    }

    pub fn select_shared<S: crate::streams::Stream<A>>(&self) -> impl Iterator<Item = At<Arc<S>>> + '_ {
        self.rows.iter().filter_map(|(_, definition, position, row)| {
            if definition.id() != S::ID || definition.version() != S::VERSION {
                return None;
            }
            Arc::clone(row).downcast::<S>().ok().map(|value| At { value, position: *position })
        })
    }
}

impl<A: Application, S: Series<A>> Serving<A> for S {
    #[allow(clippy::type_complexity)]
    fn reader<'read>(
        &'read mut self, stream: crate::streams::Definition, track: &'read [String], start: Bound<Position>, end: Bound<Position>,
        limits: crate::streams::ReadLimits,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn crate::streams::RowSource>, crate::streams::ReadError>> + Send + 'read>> {
        Box::pin(Series::reader(self, stream, track, start, end, limits))
    }

    fn rows<'read>(
        &'read mut self, stream: crate::streams::Definition, track: &'read [String], start: Bound<Position>, end: Bound<Position>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<At<Arc<dyn Any + Send + Sync>>>, crate::streams::BeneathWindow>> + Send + 'read>> {
        Box::pin(Series::rows(self, stream, track, start, end))
    }

    fn buckets<'read>(
        &'read mut self, stream: crate::streams::Definition, track: &'read [String], start: Bound<u64>, end: Bound<u64>, budget: usize,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<crate::streams::Bucket>, crate::streams::BeneathWindow>> + Send + 'read>> {
        Box::pin(Series::buckets(self, stream, track, start, end, budget))
    }

    fn aggregate<'read>(
        &'read mut self, stream: crate::streams::Definition, track: &'read [String], start: Bound<u64>, end: Bound<u64>,
        kind: crate::Aggregate,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<crate::streams::Aggregated>, crate::streams::BeneathWindow>> + Send + 'read>> {
        Box::pin(Series::aggregate(self, stream, track, start, end, kind))
    }

    fn seal<'read>(
        &'read mut self, stream: crate::streams::Definition, track: &'read [String],
    ) -> Pin<Box<dyn Future<Output = Position> + Send + 'read>> {
        Box::pin(Series::seal(self, stream, track))
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
