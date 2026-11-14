/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exact Linear Percentiles over a Bounded Numeric Spool

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use std::io::Error as IoError;

// Standard Library Imports
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
};

// Third Party Imports

// Local Imports
use crate::{BackendFailure, Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// Exact Linear Percentiles With Caller-Budgeted Numeric Storage And Rank Scratch.
#[derive(Debug)]
pub struct Quantiles {
    memory: Vec<u64>,
    file: Option<BufWriter<File>>,
    capacity: usize,
    buffer_capacity: usize,
    count: u64,
}

struct Rank {
    key: u64,
    ordinal: u64,
    prefix: u64,
    bins: [u64; 256],
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Quantiles {
    // Bounds Numeric Payload Storage, Including The Spill Writer Buffer, In Bytes.
    pub fn new(memory_bytes: usize) -> Self {
        let buffer_capacity = (memory_bytes / 2).min(65536);
        Self {
            memory: Vec::new(),
            file: None,
            capacity: (memory_bytes - buffer_capacity) / 8,
            buffer_capacity,
            count: 0,
        }
    }

    pub fn push(&mut self, value: f64) -> Result<()> {
        if !value.is_finite() {
            return Err(invalid("numeric spool requires finite values"));
        }
        let count = self.count.checked_add(1).ok_or_else(|| invalid("numeric spool count overflow"))?;
        let bits = ordered(value);
        if self.file.is_none() && self.memory.len() < self.capacity {
            if self.memory.len() == self.memory.capacity() {
                let next = self.memory.capacity().saturating_mul(2).max(1).min(self.capacity);
                self.memory.reserve_exact(next - self.memory.len());
            }
            self.memory.push(bits);
        } else {
            if self.file.is_none() {
                let mut file = BufWriter::with_capacity(self.buffer_capacity, tempfile::tempfile().map_err(io)?);
                for bits in &self.memory {
                    file.write_all(&bits.to_le_bytes()).map_err(io)?;
                }
                self.memory = Vec::new();
                self.file = Some(file);
            }
            self.file.as_mut().expect("opened spool").write_all(&bits.to_le_bytes()).map_err(io)?;
        }
        self.count = count;
        Ok(())
    }

    pub fn size(&self) -> usize {
        std::mem::size_of::<Self>() + self.memory.capacity() * 8 + self.file.as_ref().map_or(0, |file| file.capacity())
    }

    pub fn spilled(&self) -> bool {
        self.file.is_some()
    }

    // Consumes The Spool, Bounding Rank/Result Maps And Read Buffers By `scratch_bytes`.
    // The Caller Check Runs Before Work And At Most Every 1,024 Numeric Inputs.
    pub fn values(mut self, percentiles: &[u8], scratch_bytes: usize, check: &mut impl FnMut() -> Result<()>) -> Result<BTreeMap<u8, f64>> {
        check()?;
        if percentiles.iter().any(|percentile| *percentile > 100) {
            return Err(invalid("percentile must be in 0..=100"));
        }
        if self.count == 0 || percentiles.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut requested = [false; 101];
        for percentile in percentiles {
            requested[usize::from(*percentile)] = true;
        }
        let results = requested.into_iter().filter(|requested| *requested).count();
        // Reserve Node Slack For The Result Map And Exact Vector Capacity For Rank Scratch.
        let mut scratch = (results + 10).saturating_mul(std::mem::size_of::<(u8, f64)>() + 128);
        scratch = scratch.saturating_add(results * 2 * std::mem::size_of::<u64>());
        if scratch > scratch_bytes {
            return Err(invalid("numeric rank scratch exceeds memory budget"));
        }
        let mut ordinals = Vec::with_capacity(results * 2);
        for (percentile, requested) in requested.into_iter().enumerate() {
            if !requested {
                continue;
            }
            let numerator = u128::from(self.count - 1) * percentile as u128;
            let lower = (numerator / 100) as u64;
            let upper = lower + u64::from(!numerator.is_multiple_of(100));
            for ordinal in [lower, upper] {
                ordinals.push(ordinal);
            }
        }
        ordinals.sort_unstable();
        ordinals.dedup();
        scratch = scratch.saturating_add(ordinals.len().saturating_mul(std::mem::size_of::<Rank>()));
        if scratch > scratch_bytes {
            return Err(invalid("numeric rank scratch exceeds memory budget"));
        }
        let mut ranks = Vec::with_capacity(ordinals.len());
        for ordinal in &ordinals {
            ranks.push(Rank {
                key: *ordinal,
                ordinal: *ordinal,
                prefix: 0,
                bins: [0; 256],
            });
        }
        if let Some(file) = &mut self.file {
            file.flush().map_err(io)?;
        }
        for depth in 0..8 {
            check()?;
            let mut push = |bits: u64| {
                for rank in &mut ranks {
                    if depth == 0 || bits >> (64 - depth * 8) == rank.prefix {
                        rank.bins[((bits >> (56 - depth * 8)) & 255) as usize] += 1;
                    }
                }
            };
            if let Some(file) = &mut self.file {
                file.get_mut().seek(SeekFrom::Start(0)).map_err(io)?;
                let mut reader = BufReader::with_capacity(scratch_bytes.saturating_sub(scratch).min(65536), file.get_mut());
                for index in 0..self.count {
                    if index.is_multiple_of(1024) {
                        check()?;
                    }
                    let mut bytes = [0; 8];
                    reader.read_exact(&mut bytes).map_err(io)?;
                    push(u64::from_le_bytes(bytes));
                }
            } else {
                for (index, bits) in self.memory.iter().enumerate() {
                    if index.is_multiple_of(1024) {
                        check()?;
                    }
                    push(*bits);
                }
            }
            for rank in &mut ranks {
                let child = rank
                    .bins
                    .iter()
                    .enumerate()
                    .find_map(|(child, count)| {
                        if rank.ordinal < *count {
                            Some(child)
                        } else {
                            rank.ordinal -= count;
                            None
                        }
                    })
                    .ok_or_else(|| Error::InvalidInput {
                        field: "percentile",
                        reason: "unavailable exact rank",
                    })?;
                rank.prefix = (rank.prefix << 8) | child as u64;
                rank.bins.fill(0);
            }
        }
        check()?;
        let mut result = BTreeMap::new();
        for percentile in percentiles {
            let numerator = u128::from(self.count - 1) * u128::from(*percentile);
            let lower = (numerator / 100) as u64;
            let upper = lower + u64::from(!numerator.is_multiple_of(100));
            let low = unordered(ranks[ranks.binary_search_by_key(&lower, |rank| rank.key).expect("requested lower rank")].prefix);
            let high = unordered(ranks[ranks.binary_search_by_key(&upper, |rank| rank.key).expect("requested upper rank")].prefix);
            let fraction = (numerator % 100) as f64 / 100.0;
            let value = if low == high {
                low
            } else if low.signum() == high.signum() {
                low + (high - low) * fraction
            } else {
                low * (1.0 - fraction) + high * fraction
            };
            result.insert(*percentile, value);
        }
        Ok(result)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn io(error: IoError) -> Error {
    Error::backend("numeric spool", BackendFailure::Transient, error)
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "percentile", reason }
}

fn ordered(value: f64) -> u64 {
    let bits = value.to_bits();
    if bits >> 63 != 0 { !bits } else { bits ^ (1 << 63) }
}

fn unordered(value: u64) -> f64 {
    f64::from_bits(if value >> 63 != 0 { value ^ (1 << 63) } else { !value })
}

/* === Tests === */
