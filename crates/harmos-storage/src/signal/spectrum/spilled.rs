/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Bounded Disk FFT Workspace

use std::io::{Read, Seek, SeekFrom, Write};

use super::bounded::{Buffer, transform};
use crate::signal::{Error, Result};

const PAGE_VALUES: usize = 4096;
const PAGE_BYTES: usize = PAGE_VALUES * 16;
const CACHE_PAGES: usize = 4;

/// Temporary Complex Samples With A Fixed 256 KiB Page Cache.
pub struct FftBuffer {
    file: tempfile::NamedTempFile,
    length: usize,
    pages: Vec<Page>,
    clock: u64,
}

struct Page {
    number: usize,
    bytes: Vec<u8>,
    dirty: bool,
    touched: u64,
}

impl FftBuffer {
    pub fn new(length: usize) -> Result<Self> {
        if !length.is_power_of_two() {
            return Err(Error::InvalidInput {
                reason: "FFT requires a power-of-two buffer",
            });
        }
        let pages = length.div_ceil(PAGE_VALUES);
        let bytes = pages.checked_mul(PAGE_BYTES).and_then(|bytes| u64::try_from(bytes).ok());
        let bytes = bytes.ok_or(Error::InvalidInput {
            reason: "FFT buffer exceeds file address range",
        })?;
        let file = tempfile::NamedTempFile::new()?;
        file.as_file().set_len(bytes)?;
        Ok(Self {
            file,
            length,
            pages: Vec::new(),
            clock: 0,
        })
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn get(&mut self, index: usize) -> Result<(f64, f64)> {
        let page = self.page(index)?;
        let offset = (index % PAGE_VALUES) * 16;
        let real = f64::from_le_bytes(page.bytes[offset..offset + 8].try_into().expect("sample width"));
        let imag = f64::from_le_bytes(page.bytes[offset + 8..offset + 16].try_into().expect("sample width"));
        Ok((real, imag))
    }

    pub fn set(&mut self, index: usize, value: (f64, f64)) -> Result<()> {
        let page = self.page(index)?;
        let offset = (index % PAGE_VALUES) * 16;
        page.bytes[offset..offset + 8].copy_from_slice(&value.0.to_le_bytes());
        page.bytes[offset + 8..offset + 16].copy_from_slice(&value.1.to_le_bytes());
        page.dirty = true;
        Ok(())
    }

    pub fn transform(&mut self, check: impl FnMut() -> bool) -> Result<()> {
        transform(self, check)
    }

    fn page(&mut self, index: usize) -> Result<&mut Page> {
        if index >= self.length {
            return Err(Error::InvalidInput {
                reason: "FFT buffer index is out of range",
            });
        }
        self.clock += 1;
        let number = index / PAGE_VALUES;
        if let Some(slot) = self.pages.iter().position(|page| page.number == number) {
            self.pages[slot].touched = self.clock;
            return Ok(&mut self.pages[slot]);
        }

        let slot = if self.pages.len() < CACHE_PAGES {
            self.pages.push(Page {
                number,
                bytes: vec![0; PAGE_BYTES],
                dirty: false,
                touched: 0,
            });
            self.pages.len() - 1
        } else {
            self.pages.iter().enumerate().min_by_key(|(_, page)| page.touched).expect("cache is populated").0
        };
        let page = &mut self.pages[slot];
        let file = self.file.as_file_mut();
        if page.dirty {
            file.seek(SeekFrom::Start((page.number * PAGE_BYTES) as u64))?;
            file.write_all(&page.bytes)?;
        }
        file.seek(SeekFrom::Start((number * PAGE_BYTES) as u64))?;
        file.read_exact(&mut page.bytes)?;
        page.number = number;
        page.touched = self.clock;
        page.dirty = false;
        Ok(page)
    }
}

impl Buffer for FftBuffer {
    fn len(&self) -> usize {
        self.length
    }
    fn get(&mut self, index: usize) -> Result<(f64, f64)> {
        self.get(index)
    }
    fn set(&mut self, index: usize, value: (f64, f64)) -> Result<()> {
        self.set(index, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spilled_fft_matches_memory_across_evicted_pages() {
        let mut real: Vec<f64> = (0..32768).map(|index| (f64::from(index) * 0.031).sin()).collect();
        let mut imaginary = vec![0.0; real.len()];
        let mut disk = FftBuffer::new(real.len()).unwrap();
        for (index, value) in real.iter().enumerate() {
            disk.set(index, (*value, 0.0)).unwrap();
        }
        super::super::fft_in_place(&mut real, &mut imaginary, || true).unwrap();
        disk.transform(|| true).unwrap();
        for (index, (real, imaginary)) in real.into_iter().zip(imaginary).enumerate() {
            assert_eq!(disk.get(index).unwrap(), (real, imaginary));
        }
        assert_eq!(disk.pages.len(), CACHE_PAGES);
        assert!(matches!(disk.transform(|| false), Err(Error::Interrupted)));
        assert!(disk.get(disk.len()).is_err());
    }
}
