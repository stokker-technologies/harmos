/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::any::Any;
use std::ops::Bound;
use std::sync::Arc;

use harmos_domain::Aggregate;
use harmos_storage::signal::Summary;
use tokio::sync::{mpsc, oneshot};

use super::BeneathWindow;
use crate::aggregate;
use crate::{state::At, state::Position, streams::Definition};

pub(crate) type Served = At<Arc<dyn Any + Send + Sync>>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub axis: u64,
    pub value: f64,
}

// One plot bucket of a field: the points a chart draws, and the running
// summary every mergeable aggregate is answered from.
#[derive(Clone, Debug, PartialEq)]
pub struct Bucket {
    pub field: String,
    pub first: Point,
    pub last: Point,
    pub min: Point,
    pub max: Point,
    summary: Summary,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Aggregated {
    pub field: String,
    pub value: f64,
}

pub(crate) struct Opened {
    pub(crate) source: Result<Box<dyn crate::streams::RowSource>, crate::streams::ReadError>,
    pub(crate) permit: tokio::sync::OwnedSemaphorePermit,
}

#[derive(Clone)]
pub(crate) struct Tier {
    sender: mpsc::Sender<Request>,
    readers: Arc<tokio::sync::Semaphore>,
}

pub(crate) enum Request {
    Reader {
        stream: Definition,
        track: Vec<String>,
        start: Bound<Position>,
        end: Bound<Position>,
        limits: crate::streams::ReadLimits,
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

impl Bucket {
    pub fn point(field: impl Into<String>, point: Point) -> Self {
        let mut summary = Summary::new();
        summary.push(point.value);

        Self {
            field: field.into(),
            first: point,
            last: point,
            min: point,
            max: point,
            summary,
        }
    }

    pub fn merge(&self, later: &Self) -> Self {
        assert_eq!(self.field, later.field, "a bucket merges only with a bucket of its own field");

        Self {
            field: self.field.clone(),
            first: earliest(self.first, later.first),
            last: latest(self.last, later.last),
            min: lower(self.min, later.min),
            max: higher(self.max, later.max),
            summary: self.summary.merge(&later.summary),
        }
    }

    pub fn count(&self) -> u64 {
        self.summary.count()
    }

    pub fn summary(&self) -> &Summary {
        &self.summary
    }

    // Mergeable members only: a bucket keeps no values, so median and
    // percentiles answer `None` and are asked of the values instead.
    pub fn aggregate(&self, kind: Aggregate) -> Option<f64> {
        aggregate::of_summary(&self.summary, kind)
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

    pub(crate) fn reader_permit(&self) -> Result<tokio::sync::OwnedSemaphorePermit, crate::streams::ReadError> {
        self.readers.clone().try_acquire_owned().map_err(|_| crate::streams::ReadError::Busy)
    }

    pub(crate) async fn reader(
        &self, stream: Definition, track: Vec<String>, start: Bound<Position>, end: Bound<Position>, limits: crate::streams::ReadLimits,
        permit: tokio::sync::OwnedSemaphorePermit,
    ) -> Result<Opened, crate::streams::ReadError> {
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
            .map_err(|_| crate::streams::ReadError::Closed)?;
        answer.await.map_err(|_| crate::streams::ReadError::Closed)
    }

    pub(crate) async fn rows(
        &self, stream: Definition, track: Vec<String>, start: Bound<Position>, end: Bound<Position>,
    ) -> Result<Vec<Served>, BeneathWindow> {
        let (reply, answer) = oneshot::channel();
        self.sender.send(Request::Rows { stream, track, start, end, reply }).await.map_err(|_| BeneathWindow::Unserved)?;

        answer.await.map_err(|_| BeneathWindow::Unserved)?
    }

    pub(crate) async fn buckets(
        &self, stream: Definition, track: Vec<String>, start: Bound<u64>, end: Bound<u64>, budget: usize,
    ) -> Result<Vec<Bucket>, BeneathWindow> {
        let (reply, answer) = oneshot::channel();
        self.sender.send(Request::Buckets { stream, track, start, end, budget, reply }).await.map_err(|_| BeneathWindow::Unserved)?;

        answer.await.map_err(|_| BeneathWindow::Unserved)?
    }

    pub(crate) async fn aggregate(
        &self, stream: Definition, track: Vec<String>, start: Bound<u64>, end: Bound<u64>, kind: Aggregate,
    ) -> Result<Vec<Aggregated>, BeneathWindow> {
        let (reply, answer) = oneshot::channel();
        self.sender.send(Request::Aggregate { stream, track, start, end, kind, reply }).await.map_err(|_| BeneathWindow::Unserved)?;

        answer.await.map_err(|_| BeneathWindow::Unserved)?
    }

    pub(crate) async fn seal(&self, stream: Definition, track: Vec<String>) -> Position {
        let (reply, answer) = oneshot::channel();

        if self.sender.send(Request::Seal { stream, track, reply }).await.is_err() {
            return Position::ORIGIN;
        }

        answer.await.unwrap_or(Position::ORIGIN)
    }
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
