/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Budgeted Exact Order Statistics With Independent Clone Prefixes

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::num::NonZeroU128;
use std::sync::{Arc, Mutex};

// Third Party Imports
use tempfile::NamedTempFile;

// Local Imports
use super::accumulate::sort_values;
use super::scalar::{ExactNumber, ScalarValue};
use super::spill::io;
use super::transform::invalid;
use crate::Result;
use crate::config::StorageAccumulatorSettings;

/* === Constants === */

const SLOT: usize = 40;
const BUFFER: usize = 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub(super) struct Values {
    file: Option<Arc<Mutex<File>>>,
    memory: Vec<[u8; SLOT]>,
    capacity: usize,
    pub length: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Values {
    pub fn new(settings: StorageAccumulatorSettings) -> Self {
        Self {
            file: None,
            memory: Vec::new(),
            capacity: settings.memory_bytes / SLOT,
            length: 0,
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
            let mut replacement = tempfile::tempfile().map_err(io)?;
            let mut source = self.file.as_ref().unwrap().lock().map_err(|_| invalid("order statistic file poisoned"))?;
            source.seek(SeekFrom::Start(0)).map_err(io)?;
            let mut buffer = vec![0; BUFFER];
            loop {
                let count = source.read(&mut buffer).map_err(io)?;
                if count == 0 {
                    break;
                }
                replacement.write_all(&buffer[..count]).map_err(io)?;
            }
            drop(source);
            self.file = Some(Arc::new(Mutex::new(replacement)));
        }
        if self.file.is_none() {
            let mut file = BufWriter::with_capacity(BUFFER, tempfile::tempfile().map_err(io)?);
            for slot in &self.memory {
                file.write_all(slot).map_err(io)?;
            }
            let file = file.into_inner().map_err(|error| io(error.into_error()))?;
            self.file = Some(Arc::new(Mutex::new(file)));
            self.memory = Vec::new();
        }
        let mut file = self.file.as_ref().unwrap().lock().map_err(|_| invalid("order statistic file poisoned"))?;
        file.seek(SeekFrom::End(0)).map_err(io)?;
        file.write_all(&slot).map_err(io)?;
        self.length = length;
        Ok(())
    }

    pub fn ranks(
        &mut self, lower: usize, upper: usize, mut compare: impl FnMut(&ScalarValue, &ScalarValue) -> Result<Ordering>,
        check: &mut impl FnMut() -> Result<()>,
    ) -> Result<(ScalarValue, ScalarValue)> {
        check()?;
        if lower > upper || upper >= self.length {
            return Err(invalid("invalid order statistic ranks"));
        }
        if self.file.is_none() {
            sort_values(&mut self.memory, |left, right| {
                check()?;
                compare(&decode(&left)?, &decode(&right)?)
            })?;
            return Ok((decode(&self.memory[lower])?, decode(&self.memory[upper])?));
        }
        let mut source = NamedTempFile::new().map_err(io)?;
        let mut original = self
            .file
            .as_ref()
            .ok_or_else(|| invalid("empty order statistic"))?
            .lock()
            .map_err(|_| invalid("order statistic file poisoned"))?;
        original.seek(SeekFrom::Start(0)).map_err(io)?;
        let mut buffer = vec![0; BUFFER];
        loop {
            check()?;
            let count = original.read(&mut buffer).map_err(io)?;
            if count == 0 {
                break;
            }
            source.write_all(&buffer[..count]).map_err(io)?;
        }
        drop(original);
        drop(buffer);
        let mut width = 1usize;
        while width < self.length {
            check()?;
            let next = NamedTempFile::new().map_err(io)?;
            let mut writer = BufWriter::with_capacity(BUFFER, next.reopen().map_err(io)?);
            let mut left = BufReader::with_capacity(BUFFER, source.reopen().map_err(io)?);
            let mut right = BufReader::with_capacity(BUFFER, source.reopen().map_err(io)?);
            let mut start = 0;
            while start < self.length {
                let middle = start.saturating_add(width).min(self.length);
                let end = middle.saturating_add(width).min(self.length);
                left.seek(SeekFrom::Start(position(start)?)).map_err(io)?;
                right.seek(SeekFrom::Start(position(middle)?)).map_err(io)?;
                let (mut a, mut b) = (start, middle);
                let mut l = if a < middle { Some(read(&mut left)?) } else { None };
                let mut r = if b < end { Some(read(&mut right)?) } else { None };
                while l.is_some() || r.is_some() {
                    check()?;
                    let first = match (&l, &r) {
                        (Some(l), Some(r)) => !compare(&decode(l)?, &decode(r)?)?.is_gt(),
                        (Some(_), None) => true,
                        _ => false,
                    };
                    if first {
                        writer.write_all(l.as_ref().unwrap()).map_err(io)?;
                        a += 1;
                        l = if a < middle { Some(read(&mut left)?) } else { None };
                    } else {
                        writer.write_all(r.as_ref().unwrap()).map_err(io)?;
                        b += 1;
                        r = if b < end { Some(read(&mut right)?) } else { None };
                    }
                }
                start = end;
            }
            writer.flush().map_err(io)?;
            drop(writer);
            source = next;
            width = width.saturating_mul(2);
        }
        let file = source.as_file_mut();
        file.seek(SeekFrom::Start(position(lower)?)).map_err(io)?;
        let lower = decode(&read(file)?)?;
        file.seek(SeekFrom::Start(position(upper)?)).map_err(io)?;
        let upper = decode(&read(file)?)?;
        Ok((lower, upper))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn position(index: usize) -> Result<u64> {
    (index as u64).checked_mul(SLOT as u64).ok_or_else(|| invalid("order statistic offset overflow"))
}

fn read(file: &mut impl Read) -> Result<[u8; SLOT]> {
    let mut bytes = [0; SLOT];
    file.read_exact(&mut bytes).map_err(io)?;
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
    fn budget_transition() {
        let settings = StorageAccumulatorSettings { memory_bytes: 100 * SLOT };
        let mut values = Values::new(settings);
        for value in (0..100).rev() {
            values.push(&ScalarValue::Unsigned(value)).unwrap();
            assert!(values.file.is_none());
            assert!(values.memory.capacity() * SLOT <= settings.memory_bytes);
        }
        let mut memory = values.clone();
        let expected = memory.ranks(49, 50, |a, b| Ok(a.compare(b)?), &mut || Ok(())).unwrap();
        values.push(&ScalarValue::Unsigned(100)).unwrap();
        assert!(values.file.is_some());
        assert_eq!(values.memory.capacity(), 0);
        assert_eq!(values.ranks(49, 50, |a, b| Ok(a.compare(b)?), &mut || Ok(())).unwrap(), expected);
        // Independent Prefixes Survive Both The Transition And Disk Copy-On-Write.
        let mut branch = values.clone();
        values.push(&ScalarValue::Unsigned(101)).unwrap();
        branch.push(&ScalarValue::Unsigned(102)).unwrap();
        assert_eq!(values.ranks(101, 101, |a, b| Ok(a.compare(b)?), &mut || Ok(())).unwrap().0, ScalarValue::Unsigned(101));
        assert_eq!(branch.ranks(101, 101, |a, b| Ok(a.compare(b)?), &mut || Ok(())).unwrap().0, ScalarValue::Unsigned(102));
        assert_eq!(memory.length, 100);
        assert!(memory.file.is_none());
    }
}
