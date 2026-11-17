/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Budgeted Exact Order Statistics with Independent Clone Prefixes

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::cmp::Ordering;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::num::NonZeroU128;
use std::sync::{Arc, Mutex};

// Third Party Imports
use tempfile::NamedTempFile;

// Local Imports
use crate::Result;
use crate::config::QueryAccumulatorSettings;
use crate::scalar::{ExactNumber, ScalarValue};
use crate::spill;
use crate::transform::invalid;

/* === Constants === */

const SLOT: usize = 40;
const BUFFER: usize = 65536;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub(crate) struct Values {
    file: Option<Arc<Mutex<BufWriter<File>>>>,
    memory: Vec<[u8; SLOT]>,
    capacity: usize,
    pub length: usize,
    memory_bytes: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Values {
    pub fn new(settings: QueryAccumulatorSettings) -> Self {
        Self {
            file: None,
            memory: Vec::new(),
            capacity: (settings.memory_bytes - (settings.memory_bytes / 4).min(BUFFER)) / SLOT,
            length: 0,
            memory_bytes: settings.memory_bytes,
        }
    }

    pub fn push(&mut self, value: &ScalarValue) -> Result<()> {
        let slot = encode(value)?;
        let length = self.length.checked_add(1).ok_or_else(|| invalid("order statistic count overflow"))?;
        if self.file.is_none() && self.length < self.capacity {
            if self.memory.len() == self.memory.capacity() {
                let capacity = self.memory.capacity().saturating_mul(2).max(1).min(self.capacity);
                self.memory.reserve_exact(capacity - self.memory.len());
            }
            self.memory.push(slot);
            self.length = length;
            return Ok(());
        }
        if self.file.as_ref().is_some_and(|file| Arc::strong_count(file) > 1) {
            let mut source = self.file.as_ref().unwrap().lock().map_err(|_| invalid("recreate the poisoned order statistic spool"))?;
            source.flush().map_err(spill::io)?;
            source.get_mut().seek(SeekFrom::Start(0)).map_err(spill::io)?;
            let mut replacement = BufWriter::with_capacity(self.buffer_capacity(), tempfile::tempfile().map_err(spill::io)?);
            io::copy(source.get_mut(), &mut replacement).map_err(spill::io)?;
            drop(source);
            self.file = Some(Arc::new(Mutex::new(replacement)));
        }
        if self.file.is_none() {
            let mut file = BufWriter::with_capacity(self.buffer_capacity(), tempfile::tempfile().map_err(spill::io)?);
            for slot in &self.memory {
                file.write_all(slot).map_err(spill::io)?;
            }
            self.file = Some(Arc::new(Mutex::new(file)));
            self.memory = Vec::new();
        }
        let mut file = self.file.as_ref().unwrap().lock().map_err(|_| invalid("recreate the poisoned order statistic spool"))?;
        file.write_all(&slot).map_err(spill::io)?;
        self.length = length;
        Ok(())
    }

    fn buffer_capacity(&self) -> usize {
        (self.memory_bytes / 4).min(BUFFER)
    }

    pub fn ranks(&mut self, ranks: &[usize], check: &mut impl FnMut() -> Result<()>) -> Result<Vec<ScalarValue>> {
        check()?;
        if ranks.iter().any(|rank| *rank >= self.length) {
            return Err(invalid("select available order statistic ranks"));
        }
        if ranks.is_empty() {
            return Ok(Vec::new());
        }
        if self.file.is_none() {
            sort(&mut self.memory, check)?;
            return ranks.iter().map(|rank| decode(&self.memory[*rank])).collect();
        }

        // Sort Budget-Sized Runs Before Merging Sequentially.
        let buffer_capacity = self.buffer_capacity();
        let width = ((self.memory_bytes - 3 * buffer_capacity) / SLOT).max(1);
        let mut source = NamedTempFile::new().map_err(spill::io)?;
        let mut original = self.file.as_ref().unwrap().lock().map_err(|_| invalid("recreate the poisoned order statistic spool"))?;
        original.flush().map_err(spill::io)?;
        original.get_mut().seek(SeekFrom::Start(0)).map_err(spill::io)?;
        let mut reader = BufReader::with_capacity(buffer_capacity, original.get_mut());
        let mut writer = BufWriter::with_capacity(buffer_capacity, source.as_file_mut());
        let mut chunk = Vec::with_capacity(width.min(self.length));
        let mut remaining = self.length;
        while remaining > 0 {
            check()?;
            chunk.clear();
            for _ in 0..width.min(remaining) {
                chunk.push(read(&mut reader)?);
            }
            sort(&mut chunk, check)?;
            for slot in &chunk {
                writer.write_all(slot).map_err(spill::io)?;
            }
            remaining -= chunk.len();
        }
        writer.flush().map_err(spill::io)?;
        drop(writer);
        drop(reader);
        drop(original);
        drop(chunk);

        let mut width = width;
        while width < self.length {
            check()?;
            let next = NamedTempFile::new().map_err(spill::io)?;
            let mut writer = BufWriter::with_capacity(buffer_capacity, next.reopen().map_err(spill::io)?);
            let mut left = BufReader::with_capacity(buffer_capacity, source.reopen().map_err(spill::io)?);
            let mut right = BufReader::with_capacity(buffer_capacity, source.reopen().map_err(spill::io)?);
            let mut start = 0;
            while start < self.length {
                let middle = start.saturating_add(width).min(self.length);
                let end = middle.saturating_add(width).min(self.length);
                left.seek(SeekFrom::Start(position(start)?)).map_err(spill::io)?;
                right.seek(SeekFrom::Start(position(middle)?)).map_err(spill::io)?;
                let (mut a, mut b) = (start, middle);
                let mut l = if a < middle { Some(read(&mut left)?) } else { None };
                let mut r = if b < end { Some(read(&mut right)?) } else { None };
                while l.is_some() || r.is_some() {
                    if (a + b).is_multiple_of(1024) {
                        check()?;
                    }
                    let first = match (&l, &r) {
                        (Some(l), Some(r)) => !compare(l, r)?.is_gt(),
                        (Some(_), None) => true,
                        _ => false,
                    };
                    if first {
                        writer.write_all(l.as_ref().unwrap()).map_err(spill::io)?;
                        a += 1;
                        l = if a < middle { Some(read(&mut left)?) } else { None };
                    } else {
                        writer.write_all(r.as_ref().unwrap()).map_err(spill::io)?;
                        b += 1;
                        r = if b < end { Some(read(&mut right)?) } else { None };
                    }
                }
                start = end;
            }
            writer.flush().map_err(spill::io)?;
            drop(writer);
            source = next;
            width = width.saturating_mul(2);
        }
        // Collect All Ranks In One Sequential Read Of The Sorted Output.
        let mut requested = ranks.to_vec();
        requested.sort_unstable();
        requested.dedup();
        source.as_file_mut().seek(SeekFrom::Start(0)).map_err(spill::io)?;
        let mut reader = BufReader::with_capacity(buffer_capacity, source.as_file_mut());
        let mut selected = Vec::with_capacity(requested.len());
        let mut next = 0;
        for index in 0..=*requested.last().unwrap() {
            if index.is_multiple_of(1024) {
                check()?;
            }
            let slot = read(&mut reader)?;
            if index == requested[next] {
                selected.push(decode(&slot)?);
                next += 1;
            }
        }
        Ok(ranks.iter().map(|rank| selected[requested.binary_search(rank).unwrap()].clone()).collect())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn compare(left: &[u8; SLOT], right: &[u8; SLOT]) -> Result<Ordering> {
    if left[0] == right[0] {
        let left_bytes = left[1..9].try_into().unwrap();
        let right_bytes = right[1..9].try_into().unwrap();
        match left[0] {
            1 => return Ok(i64::from_le_bytes(left_bytes).cmp(&i64::from_le_bytes(right_bytes))),
            2 => return Ok(u64::from_le_bytes(left_bytes).cmp(&u64::from_le_bytes(right_bytes))),
            3 => return Ok(f64::from_le_bytes(left_bytes).total_cmp(&f64::from_le_bytes(right_bytes))),
            _ => {}
        }
    }
    Ok(decode(left)?.compare(&decode(right)?)?)
}

fn sort(values: &mut [[u8; SLOT]], check: &mut impl FnMut() -> Result<()>) -> Result<()> {
    let mut failure = None;
    let mut comparisons = 0usize;
    values.sort_unstable_by(|left, right| {
        comparisons += 1;
        if comparisons.is_multiple_of(1024) && failure.is_none() {
            if let Err(error) = check() {
                failure = Some(error);
            }
        }
        let result = compare(left, right);
        match result {
            Ok(order) => order,
            Err(error) => {
                if failure.is_none() {
                    failure = Some(error);
                }
                left.cmp(right)
            }
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    check()
}

fn position(index: usize) -> Result<u64> {
    (index as u64).checked_mul(SLOT as u64).ok_or_else(|| invalid("order statistic offset overflow"))
}

fn read(file: &mut impl Read) -> Result<[u8; SLOT]> {
    let mut bytes = [0; SLOT];
    file.read_exact(&mut bytes).map_err(spill::io)?;
    Ok(bytes)
}

fn encode(value: &ScalarValue) -> Result<[u8; SLOT]> {
    let mut bytes = [0; SLOT];
    match value {
        ScalarValue::Signed(value) => {
            bytes[0] = 1;
            bytes[1..9].copy_from_slice(&value.to_le_bytes());
        }
        ScalarValue::Unsigned(value) => {
            bytes[0] = 2;
            bytes[1..9].copy_from_slice(&value.to_le_bytes());
        }
        ScalarValue::Float(value) => {
            bytes[0] = 3;
            bytes[1..9].copy_from_slice(&value.to_le_bytes());
        }
        ScalarValue::Exact(value) => {
            bytes[0] = 4;
            bytes[1] = u8::from(value.negative);
            bytes[2..18].copy_from_slice(&value.numerator.to_le_bytes());
            bytes[18..34].copy_from_slice(&value.denominator.get().to_le_bytes());
        }
        _ => return Err(invalid("order statistic requires a numeric scalar")),
    }
    Ok(bytes)
}

fn decode(bytes: &[u8; SLOT]) -> Result<ScalarValue> {
    Ok(match bytes[0] {
        1 => ScalarValue::Signed(i64::from_le_bytes(bytes[1..9].try_into().unwrap())),
        2 => ScalarValue::Unsigned(u64::from_le_bytes(bytes[1..9].try_into().unwrap())),
        3 => ScalarValue::Float(f64::from_le_bytes(bytes[1..9].try_into().unwrap())),
        4 => ScalarValue::Exact(ExactNumber::new(
            bytes[1] != 0,
            u128::from_le_bytes(bytes[2..18].try_into().unwrap()),
            NonZeroU128::new(u128::from_le_bytes(bytes[18..34].try_into().unwrap()))
                .ok_or_else(|| invalid("invalid spilled exact denominator"))?,
        )),
        _ => return Err(invalid("invalid spilled numeric scalar")),
    })
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_capacity_respects_the_spill_buffer_reservation() {
        let settings = QueryAccumulatorSettings { memory_bytes: 100 * SLOT };
        let mut values = Values::new(settings);
        let count = values.capacity;
        for value in (0..count).rev() {
            values.push(&ScalarValue::Unsigned(value as u64)).unwrap();
            assert!(values.file.is_none());
            assert!(values.memory.capacity() * SLOT + values.buffer_capacity() <= settings.memory_bytes);
        }
        let expected = vec![ScalarValue::Unsigned((count / 2) as u64)];
        assert_eq!(values.ranks(&[count / 2], &mut || Ok(())).unwrap(), expected);
    }
}
