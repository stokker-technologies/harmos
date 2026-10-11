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
use std::ops::Bound;
use std::sync::Arc;
// Third Party Imports
use tokio::sync::{mpsc, oneshot};
// Local Imports
use super::BeneathWindow;
use crate::{At, Definition, Position};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub(crate) type Served = At<Arc<dyn Any + Send + Sync>>;

/* === Structs === */
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub axis: u64,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bucket {
    pub field: String,
    pub first: Point,
    pub last: Point,
    pub min: Point,
    pub max: Point,
    pub count: u64,
    pub sum: f64,
    pub sum_sq: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Aggregated {
    pub field: String,
    pub value: f64,
}

pub(crate) struct Opened {
    pub(crate) source: Result<Box<dyn crate::RowSource>, crate::ReadError>,
    pub(crate) permit: tokio::sync::OwnedSemaphorePermit,
}

#[derive(Clone)]
pub(crate) struct Tier {
    sender: mpsc::Sender<Request>,
    readers: Arc<tokio::sync::Semaphore>,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Aggregate {
    Min,
    Max,
    Mean,
    Sum,
    Count,
    Percentile(u8),
}

pub(crate) enum Request {
    Reader {
        stream: Definition,
        track: Vec<String>,
        start: Bound<Position>,
        end: Bound<Position>,
        limits: crate::ReadLimits,
        permit: tokio::sync::OwnedSemaphorePermit,
        reply: oneshot::Sender<Opened>,
    },
    Rows {
        stream: Definition,
        track: Vec<String>,
        start: Bound<Position>,
        end: Bound<Position>,
        reply: oneshot::Sender<Result<Vec<Served>, BeneathWindow>>,
    },
    Buckets {
        stream: Definition,
        track: Vec<String>,
        start: Bound<u64>,
        end: Bound<u64>,
        budget: usize,
        reply: oneshot::Sender<Result<Vec<Bucket>, BeneathWindow>>,
    },
    Aggregate {
        stream: Definition,
        track: Vec<String>,
        start: Bound<u64>,
        end: Bound<u64>,
        kind: Aggregate,
        reply: oneshot::Sender<Result<Vec<Aggregated>, BeneathWindow>>,
    },
    Seal {
        stream: Definition,
        track: Vec<String>,
        reply: oneshot::Sender<Position>,
    },
}

/* === Traits === */

/* === Implementations === */
impl Bucket {
    pub fn merge(&self, later: &Self) -> Self {
        assert_eq!(
            self.field, later.field,
            "a bucket merges only with a bucket of its own field"
        );

        Self {
            field: self.field.clone(),
            first: earliest(self.first, later.first),
            last: latest(self.last, later.last),
            min: lower(self.min, later.min),
            max: higher(self.max, later.max),
            count: self.count + later.count,
            sum: self.sum + later.sum,
            sum_sq: self.sum_sq + later.sum_sq,
        }
    }
}

impl Aggregate {
    pub fn name(&self) -> String {
        match self {
            Self::Min => "min".to_owned(),
            Self::Max => "max".to_owned(),
            Self::Mean => "mean".to_owned(),
            Self::Sum => "sum".to_owned(),
            Self::Count => "count".to_owned(),
            Self::Percentile(rank) => format!("p{rank}"),
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        let named = match name {
            "min" => Self::Min,
            "max" => Self::Max,
            "mean" => Self::Mean,
            "sum" => Self::Sum,
            "count" => Self::Count,
            _ => Self::Percentile(name.strip_prefix('p')?.parse().ok()?),
        };

        named.answerable().then_some(named)
    }

    pub fn folds_levels(&self) -> bool {
        !matches!(self, Self::Percentile(_))
    }

    pub fn folded(&self, bucket: &Bucket) -> Option<f64> {
        Some(match self {
            Self::Min => bucket.min.value,
            Self::Max => bucket.max.value,
            Self::Mean => bucket.sum / (bucket.count as f64),
            Self::Sum => bucket.sum,
            Self::Count => bucket.count as f64,
            Self::Percentile(_) => return None,
        })
    }

    pub fn scanned(&self, values: &mut [f64]) -> Option<f64> {
        if values.is_empty() {
            return None;
        }
        let count = values.len();

        Some(match self {
            Self::Min => values.iter().copied().fold(f64::INFINITY, f64::min),
            Self::Max => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            Self::Mean => values.iter().sum::<f64>() / (count as f64),
            Self::Sum => values.iter().sum(),
            Self::Count => count as f64,
            Self::Percentile(rank) => {
                values.sort_by(f64::total_cmp);
                values[nearest_rank(count, *rank)]
            }
        })
    }

    fn answerable(self) -> bool {
        match self {
            Self::Percentile(rank) => rank <= 100,
            _ => true,
        }
    }

    pub(crate) fn checked(self) -> Self {
        assert!(
            self.answerable(),
            "a percentile rank is a percentage, and {self:?} names none"
        );
        self
    }
}

impl Tier {
    pub(crate) fn channel(capacity: usize) -> (Self, mpsc::Receiver<Request>) {
        let (sender, receiver) = mpsc::channel(capacity.max(1));

        (
            Self {
                sender,
                readers: Arc::new(tokio::sync::Semaphore::new(capacity.max(1))),
            },
            receiver,
        )
    }

    pub(crate) fn reader_permit(
        &self,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, crate::ReadError> {
        self.readers
            .clone()
            .try_acquire_owned()
            .map_err(|_| crate::ReadError::Busy)
    }

    pub(crate) async fn reader(
        &self,
        stream: Definition,
        track: Vec<String>,
        start: Bound<Position>,
        end: Bound<Position>,
        limits: crate::ReadLimits,
        permit: tokio::sync::OwnedSemaphorePermit,
    ) -> Result<Opened, crate::ReadError> {
        let (reply, answer) = oneshot::channel();
        self.sender
            .send(Request::Reader {
                stream,
                track,
                start,
                end,
                limits,
                permit,
                reply,
            })
            .await
            .map_err(|_| crate::ReadError::Closed)?;
        answer.await.map_err(|_| crate::ReadError::Closed)
    }

    pub(crate) async fn rows(
        &self,
        stream: Definition,
        track: Vec<String>,
        start: Bound<Position>,
        end: Bound<Position>,
    ) -> Result<Vec<Served>, BeneathWindow> {
        let (reply, answer) = oneshot::channel();
        self.sender
            .send(Request::Rows {
                stream,
                track,
                start,
                end,
                reply,
            })
            .await
            .map_err(|_| BeneathWindow::Unserved)?;

        answer.await.map_err(|_| BeneathWindow::Unserved)?
    }

    pub(crate) async fn buckets(
        &self,
        stream: Definition,
        track: Vec<String>,
        start: Bound<u64>,
        end: Bound<u64>,
        budget: usize,
    ) -> Result<Vec<Bucket>, BeneathWindow> {
        let (reply, answer) = oneshot::channel();
        self.sender
            .send(Request::Buckets {
                stream,
                track,
                start,
                end,
                budget,
                reply,
            })
            .await
            .map_err(|_| BeneathWindow::Unserved)?;

        answer.await.map_err(|_| BeneathWindow::Unserved)?
    }

    pub(crate) async fn aggregate(
        &self,
        stream: Definition,
        track: Vec<String>,
        start: Bound<u64>,
        end: Bound<u64>,
        kind: Aggregate,
    ) -> Result<Vec<Aggregated>, BeneathWindow> {
        let (reply, answer) = oneshot::channel();
        self.sender
            .send(Request::Aggregate {
                stream,
                track,
                start,
                end,
                kind,
                reply,
            })
            .await
            .map_err(|_| BeneathWindow::Unserved)?;

        answer.await.map_err(|_| BeneathWindow::Unserved)?
    }

    pub(crate) async fn seal(&self, stream: Definition, track: Vec<String>) -> Position {
        let (reply, answer) = oneshot::channel();

        if self
            .sender
            .send(Request::Seal {
                stream,
                track,
                reply,
            })
            .await
            .is_err()
        {
            return Position::ORIGIN;
        }

        answer.await.unwrap_or(Position::ORIGIN)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn nearest_rank(count: usize, rank: u8) -> usize {
    (usize::from(rank) * count)
        .div_ceil(100)
        .saturating_sub(1)
        .min(count - 1)
}

fn earliest(held: Point, next: Point) -> Point {
    if next.axis < held.axis { next } else { held }
}

fn latest(held: Point, next: Point) -> Point {
    if next.axis >= held.axis { next } else { held }
}

fn lower(held: Point, next: Point) -> Point {
    if next.value < held.value { next } else { held }
}

fn higher(held: Point, next: Point) -> Point {
    if next.value > held.value { next } else { held }
}

/* === Tests === */
