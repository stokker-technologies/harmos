/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod buffer;
mod transform;

/* === Re-Exports === */
pub use transform::{ReducedSeries, Spectrum, Transform};

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::SharedQueryValue;
use super::frame::{Expr, Frame, FrameQuery, FrameResult, GroupBy, ScalarField, Scan};
use super::manager::MemoryReservation;
use super::query::sealed;
use super::reduce::{Agg, ScalarKind, ScalarValue};
use super::{
    Query, QueryOptions, QueryValue, ReadKind, ReadLimits, StorageManager, StorageView, StoredRow,
    StreamQuery, StreamQueryResult,
};
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Projection<T> = dyn Fn(&StoredRow<T>, NonZeroUsize) -> Result<SeriesPoint> + Send + Sync;

/* === Structs === */
pub struct SeriesQuery<T> {
    query: StreamQuery<T>,
    fields: [ScalarField; 2],
    samples: NonZeroUsize,
    project: Box<Projection<T>>,
}
pub struct Series<T> {
    query: StreamQueryResult<T>,
    source: Scan,
    options: QueryOptions,
    max_samples: NonZeroUsize,
    project: Box<Projection<T>>,
    pages: Vec<QueryValue<Vec<SeriesPoint>>>,
    samples: usize,
    complete: bool,
    _reservation: MemoryReservation,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SeriesPoint {
    pub axis: ScalarValue,
    pub value: ScalarValue,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<T> StreamQuery<T> {
    pub fn series(
        self,
        axis: ScalarField,
        value: ScalarField,
        samples: NonZeroUsize,
        project: impl Fn(&StoredRow<T>, NonZeroUsize) -> Result<SeriesPoint> + Send + Sync + 'static,
    ) -> Result<SeriesQuery<T>> {
        axis.check()?;
        value.check()?;
        if !matches!(
            axis.kind,
            ScalarKind::Signed | ScalarKind::Unsigned | ScalarKind::Float | ScalarKind::Datetime
        ) || !matches!(
            value.kind,
            ScalarKind::Signed | ScalarKind::Unsigned | ScalarKind::Float | ScalarKind::Exact
        ) {
            return Err(invalid(
                "series requires numeric value and numeric/time axis",
            ));
        }
        Ok(SeriesQuery {
            query: self,
            fields: [axis, value],
            samples,
            project: Box::new(project),
        })
    }
}
impl<T> SeriesQuery<T> {
    pub fn options_mut(&mut self) -> &mut QueryOptions {
        &mut self.query.options
    }
}
impl<T> sealed::Supported for SeriesQuery<T> {}
impl<C, T: MemorySize + 'static> Query<C> for SeriesQuery<T> {
    type Reader = Series<T>;
    fn options(&self) -> &QueryOptions {
        &self.query.options
    }
    fn preflight(&self, manager: &StorageManager<C>, limits: super::QueryLimits) -> Result<()> {
        self.query.preflight(manager, limits)
    }
    fn open(
        self,
        view: StorageView,
        page: ReadLimits,
        remaining: Duration,
    ) -> Result<Self::Reader> {
        let metadata = self
            .fields
            .iter()
            .map(MemorySize::memory_size)
            .fold(0usize, usize::saturating_add)
            .saturating_add(self.query.schema.memory_size())
            .saturating_add(self.query.scope.memory_size())
            .saturating_add(self.query.stream.memory_size())
            .saturating_add(self.query.item.memory_size())
            .saturating_add(std::mem::size_of_val(&*self.project))
            .saturating_add(std::mem::size_of::<Series<T>>())
            .saturating_add(512);
        let bytes = self
            .samples
            .get()
            .checked_mul(std::mem::size_of::<QueryValue<Vec<SeriesPoint>>>())
            .and_then(|bytes| bytes.checked_add(metadata))
            .ok_or(invalid("series handle memory overflow"))?;
        if bytes
            > view
                .execution_limits()?
                .ok_or(Error::ViewExpired)?
                .memory_bytes
                .get()
        {
            return Err(invalid(
                "series metadata exceeds original working allowance",
            ));
        }
        let reservation = view.manager().reserve_read_now(bytes, 1)?;
        let mut selection = self.query.options.selection(ReadKind::Stream);
        selection.schema = Some(self.query.schema.clone());
        selection.source_time = self.query.source_time;
        selection.scope = self.query.scope.clone();
        selection.stream = self.query.stream.clone();
        selection.item = self.query.item.clone();
        let options = self.query.options.clone();
        let source = Scan {
            selection,
            columns: self.fields.into_iter().collect(),
        };
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(self.samples.get())
            .map_err(|_| invalid("series handle allocation failed"))?;
        if pages.capacity() != self.samples.get() {
            return Err(invalid("series handle capacity exceeds reservation"));
        }
        let mut query = <StreamQuery<T> as Query<C>>::open(self.query, view, page, remaining)?;
        query.retain_view()?;
        Ok(Series {
            query,
            source,
            options,
            max_samples: self.samples,
            project: self.project,
            pages,
            samples: 0,
            complete: false,
            _reservation: reservation,
        })
    }
}
impl<T: MemorySize + 'static> Series<T> {
    pub fn view(&self) -> Result<&StorageView> {
        self.query.view().ok_or(Error::ViewExpired)
    }
    pub fn aggregate(&self, aggs: Vec<Agg>) -> Result<FrameResult> {
        if aggs.is_empty() || aggs.len() > 64 {
            return Err(invalid("series aggregate requires 1..64 aggregates"));
        }
        let view = self.view()?;

        // Charge Construction Before Cloning The Owned Scan And Aggregates.
        let bytes = self
            .source
            .memory_size()
            .saturating_add(
                aggs.len()
                    .saturating_mul(std::mem::size_of::<(Expr, Agg)>()),
            )
            .saturating_add(std::mem::size_of::<FrameQuery>() + 512);
        if bytes
            > view
                .execution_limits()?
                .ok_or(Error::ViewExpired)?
                .memory_bytes
                .get()
        {
            return Err(invalid(
                "series aggregate metadata exceeds original allowance",
            ));
        }
        let _construction = view.manager().reserve_read_now(bytes, 1)?;

        let mut frame = Frame::new(self.max_samples, NonZeroUsize::MIN);
        let scan = frame.scan(self.source.selection.clone(), self.source.columns.clone());
        let group_by = aggs.into_iter().fold(GroupBy::new(scan), |group_by, agg| {
            group_by.agg(Expr::Col(1), agg)
        });
        frame.group_by(group_by);
        let query = FrameQuery {
            frame,
            options: QueryOptions {
                after: 0,
                through: None,
                order: super::ReadOrder::Ascending,
                ..self.options.clone()
            },
        };
        view.manager().query_in(view, query)
    }
    pub fn transform(&mut self) -> Transform<'_, T> {
        Transform { series: self }
    }
}
impl MemorySize for SeriesPoint {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.axis
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<ScalarValue>()),
            )
            .saturating_add(
                self.value
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<ScalarValue>()),
            )
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput {
        field: "storage series",
        reason,
    }
}

/* === Tests === */
