/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Rust allocator counters for isolated, sequential benchmark regions.
#![allow(unsafe_code, reason = "the benchmark allocator forwards unchanged layouts and pointers to System")]

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

// Third Party Imports

// Local Imports

/* === Constants === */

static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static LIVE: AtomicU64 = AtomicU64::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Install as the benchmark executable's global allocator before measuring.
pub struct TrackingAllocator;

pub(super) struct Region {
    calls: u64,
    bytes: u64,
    live: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

// SAFETY: Every operation forwards the caller's allocation contract to System.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: GlobalAlloc's caller supplies a valid layout.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: GlobalAlloc's caller supplies a valid layout.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: The pointer and layout came from this allocator.
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: The caller supplies a live pointer, its layout and a valid new size.
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() {
            LIVE.fetch_sub(layout.size() as u64, Ordering::Relaxed);
            allocated(size);
        }
        result
    }
}

impl Region {
    pub(super) fn start() -> Self {
        let live = LIVE.load(Ordering::Relaxed);
        PEAK.store(live, Ordering::Relaxed);
        Self {
            calls: CALLS.load(Ordering::Relaxed),
            bytes: BYTES.load(Ordering::Relaxed),
            live,
        }
    }
    pub(super) fn finish(self) -> (u64, u64, u64) {
        (
            CALLS.load(Ordering::Relaxed) - self.calls,
            BYTES.load(Ordering::Relaxed) - self.bytes,
            PEAK.load(Ordering::Relaxed).saturating_sub(self.live),
        )
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn allocated(size: usize) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    BYTES.fetch_add(size as u64, Ordering::Relaxed);
    let live = LIVE.fetch_add(size as u64, Ordering::Relaxed) + size as u64;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

/* === Tests === */
