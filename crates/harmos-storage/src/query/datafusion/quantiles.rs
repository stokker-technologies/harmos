/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exact Linear Percentiles Over A Bounded Numeric Spool

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
};

// Third Party Imports
use datafusion::error::{DataFusionError, Result};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Debug)]
pub struct Quantiles {
    memory: Vec<u64>,
    file: Option<BufWriter<File>>,
    capacity: usize,
    count: u64,
}

struct Rank {
    ordinal: u64,
    prefix: u64,
    bins: [u64; 256],
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Quantiles {
    pub fn new(memory_bytes: usize) -> Self {
        Self {
            memory: Vec::new(),
            file: None,
            capacity: memory_bytes / 8,
            count: 0,
        }
    }

    pub fn push(&mut self, value: f64) -> Result<()> {
        let bits = ordered(value);
        if self.file.is_none() && self.memory.len() < self.capacity {
            if self.memory.len() == self.memory.capacity() {
                let next = self.memory.capacity().saturating_mul(2).max(1).min(self.capacity);
                self.memory.reserve_exact(next - self.memory.len());
            }
            self.memory.push(bits);
        } else {
            if self.file.is_none() {
                let mut file = BufWriter::with_capacity(65536, tempfile::tempfile().map_err(io)?);
                for bits in &self.memory {
                    file.write_all(&bits.to_le_bytes()).map_err(io)?;
                }
                self.file = Some(file);
                self.memory = Vec::new();
            }
            self.file.as_mut().expect("opened spool").write_all(&bits.to_le_bytes()).map_err(io)?;
        }
        self.count += 1;
        Ok(())
    }

    pub fn size(&self) -> usize {
        std::mem::size_of::<Self>() + self.memory.capacity() * 8 + self.file.as_ref().map_or(0, |file| file.capacity())
    }

    pub fn spilled(&self) -> bool {
        self.file.is_some()
    }

    pub fn values(&mut self, percentiles: &[u8]) -> Result<BTreeMap<u8, f64>> {
        if self.count == 0 {
            return Ok(BTreeMap::new());
        }
        let mut ranks = BTreeMap::new();
        for percentile in percentiles {
            let numerator = u128::from(self.count - 1) * u128::from(*percentile);
            let lower = (numerator / 100) as u64;
            let upper = lower + u64::from(numerator % 100 != 0);
            for ordinal in [lower, upper] {
                ranks.entry(ordinal).or_insert(Rank { ordinal, prefix: 0, bins: [0; 256] });
            }
        }
        if let Some(file) = &mut self.file {
            file.flush().map_err(io)?;
        }
        for depth in 0..8 {
            let mut push = |bits: u64| {
                for rank in ranks.values_mut() {
                    if depth == 0 || bits >> (64 - depth * 8) == rank.prefix {
                        rank.bins[((bits >> (56 - depth * 8)) & 255) as usize] += 1;
                    }
                }
            };
            if let Some(file) = &mut self.file {
                file.get_mut().seek(SeekFrom::Start(0)).map_err(io)?;
                let mut reader = BufReader::with_capacity(65536, file.get_mut());
                for _ in 0..self.count {
                    let mut bytes = [0; 8];
                    reader.read_exact(&mut bytes).map_err(io)?;
                    push(u64::from_le_bytes(bytes));
                }
            } else {
                for bits in &self.memory {
                    push(*bits);
                }
            }
            for rank in ranks.values_mut() {
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
                    .ok_or_else(|| DataFusionError::Execution("unavailable exact rank".into()))?;
                rank.prefix = (rank.prefix << 8) | child as u64;
                rank.bins.fill(0);
            }
        }
        let mut result = BTreeMap::new();
        for percentile in percentiles {
            let numerator = u128::from(self.count - 1) * u128::from(*percentile);
            let lower = (numerator / 100) as u64;
            let upper = lower + u64::from(numerator % 100 != 0);
            let low = unordered(ranks[&lower].prefix);
            let high = unordered(ranks[&upper].prefix);
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

fn io(error: std::io::Error) -> DataFusionError {
    DataFusionError::External(Box::new(error))
}
fn ordered(value: f64) -> u64 {
    let bits = value.to_bits();
    if bits >> 63 != 0 { !bits } else { bits ^ (1 << 63) }
}
fn unordered(value: u64) -> f64 {
    f64::from_bits(if value >> 63 != 0 { value ^ (1 << 63) } else { !value })
}

/* === Tests === */
