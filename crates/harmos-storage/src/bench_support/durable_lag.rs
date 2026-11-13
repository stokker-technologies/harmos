/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Backend-independent acceptance-to-durability measurements. No read requests a flush.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

// Third Party Imports

// Local Imports
use crate::{Identifier, Message, RecordInput, Result, SchemaKey, StorageCursor, StorageManager, StreamInput};

/* === Constants === */

pub const P95_BOUND_MS: f64 = 250.0;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug)]
pub struct Payload {
    pub bytes: Vec<u8>,
}

#[derive(Debug, serde::Serialize)]
pub struct Measurement {
    pub load: &'static str,
    pub batches: usize,
    pub bytes: u64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub maximum_ms: f64,
    pub maximum_backlog_bytes: usize,
    pub maximum_backlog_positions: u64,
    pub admission_wait_ms: f64,
    pub stages: crate::PersistenceWork,
}

/* === Enums === */

#[derive(Clone, Copy, Debug)]
pub enum Load {
    SmallHighRate,
    MultiMegabyte,
    Mixed,
}

/* === Traits === */

/* === Implementations === */

impl Load {
    pub fn name(self) -> &'static str {
        match self {
            Self::SmallHighRate => "small_high_rate",
            Self::MultiMegabyte => "multi_megabyte",
            Self::Mixed => "mixed",
        }
    }
}

/* === Public Functions === */

/// Run on a started manager with an already persisted schema. Consumers supply their own
/// backend, worker owner, settings and durability mode. Workloads are fixed across backends.
/// Lag starts at admission return; passive frontier notifications end each observation.
/// Scheduler observation delay is included; admission backpressure is reported separately.
pub async fn run(storage: &StorageManager, schema: &SchemaKey, load: Load) -> Result<Measurement> {
    storage.inner.state.lock().unwrap().persistence = crate::PersistenceWork::default();
    let batches = if matches!(load, Load::MultiMegabyte) { 128 } else { 1000 };
    let (accepted, mut admitted) = tokio::sync::mpsc::unbounded_channel();
    let producer = async {
        let mut maximum_backlog_bytes = 0;
        let mut maximum_backlog_positions = 0;
        let mut bytes = 0;
        let mut admission_wait = Duration::ZERO;
        let stream = Identifier::new("durable-lag").expect("valid benchmark identifier");
        let mut sequence = 0;
        for batch in 0..batches {
            // Admit The Fixed Load Without Asking For Persistence.
            let large = matches!(load, Load::MultiMegabyte) || matches!(load, Load::Mixed) && batch % 16 == 0;
            let (rows, payload_bytes) = if large { (64, 64 * 1024) } else { (1, 32) };
            let input = (0..rows)
                .map(|_| {
                    sequence += 1;
                    StreamInput {
                        record: RecordInput {
                            schema: schema.clone(),
                            scope: None,
                            item: stream.clone(),
                            source: None,
                            source_time: None,
                            payload: Payload { bytes: vec![0x5a; payload_bytes] }.encode(),
                        },
                        stream: stream.clone(),
                        sequence,
                    }
                })
                .collect::<Vec<_>>();
            // Honor Existing Admission Bounds Without Changing The Offered Logical Batch.
            let mut remaining = input.as_slice();
            while !remaining.is_empty() {
                let length = storage.stream_batch_len(remaining)?;
                let admitted = Instant::now();
                let position = storage.append_samples(remaining[..length].to_vec()).await?;
                admission_wait += admitted.elapsed();
                accepted.send((position, Instant::now())).expect("live lag observer");
                remaining = &remaining[length..];
            }
            bytes += (rows * payload_bytes) as u64;
            let status = storage.status();
            maximum_backlog_bytes = maximum_backlog_bytes.max(status.pending_bytes);
            maximum_backlog_positions = maximum_backlog_positions.max(status.accepted.sequence - status.saved.sequence);

            // A 1 kHz Offered Small-Sample Rate; Large Batches Are Offered Continuously.
            if !matches!(load, Load::MultiMegabyte) {
                tokio::time::sleep(Duration::from_millis(1)).await;
            } else {
                tokio::task::yield_now().await;
            }
        }
        drop(accepted);
        Ok::<_, crate::Error>((bytes, maximum_backlog_bytes, maximum_backlog_positions, admission_wait))
    };
    let observer_storage = storage.clone();
    let observer = tokio::spawn(async move {
        let mut pending = VecDeque::<(StorageCursor, Instant)>::new();
        let mut lags = Vec::with_capacity(batches);
        let mut finished = false;
        loop {
            let saved = observer_storage.get_saved_position();
            observe(&mut pending, &mut lags, saved.sequence);
            if finished && pending.is_empty() {
                break;
            }
            tokio::select! {
                next = admitted.recv(), if !finished => {
                    match next {
                        Some(next) => pending.push_back(next),
                        None => finished = true,
                    }
                }
                result = observer_storage.wait_saved_after(&saved), if !pending.is_empty() => { result?; }
            }
        }
        Ok::<_, crate::Error>(lags)
    });
    let (bytes, maximum_backlog_bytes, maximum_backlog_positions, admission_wait) = match producer.await {
        Ok(measurement) => measurement,
        Err(error) => {
            observer.abort();
            let _ = observer.await;
            return Err(error);
        }
    };
    let mut lags = observer.await.map_err(|source| crate::Error::OperationFailed {
        operation: "observe durable lag".into(),
        source: std::sync::Arc::new(source),
    })??;
    lags.sort_by(f64::total_cmp);
    Ok(Measurement {
        load: load.name(),
        batches,
        bytes,
        p50_ms: percentile(&lags, 50),
        p95_ms: percentile(&lags, 95),
        maximum_ms: lags.last().copied().unwrap_or(0.0),
        maximum_backlog_bytes,
        maximum_backlog_positions,
        admission_wait_ms: admission_wait.as_secs_f64() * 1000.0,
        stages: storage.inner.state.lock().unwrap().persistence,
    })
}

/* === Private Functions === */

fn observe(pending: &mut VecDeque<(StorageCursor, Instant)>, lags: &mut Vec<f64>, saved: u64) {
    let now = Instant::now();
    while pending.front().is_some_and(|(position, _)| position.sequence <= saved) {
        let (_, accepted) = pending.pop_front().expect("observed pending admission");
        lags.push(now.duration_since(accepted).as_secs_f64() * 1000.0);
    }
}

fn percentile(values: &[f64], rank: usize) -> f64 {
    values[(values.len() * rank).div_ceil(100).saturating_sub(1)]
}

/* === Tests === */
