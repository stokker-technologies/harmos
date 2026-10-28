/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Checked Decoding Memory Arithmetic And Exact-Capacity Containers

use super::MessageError;

pub fn add(left: usize, right: usize) -> Result<usize, MessageError> {
    left.checked_add(right).ok_or_else(|| MessageError::borrowed("decoding memory bound overflow"))
}

pub fn multiply(left: usize, right: usize) -> Result<usize, MessageError> {
    left.checked_mul(right).ok_or_else(|| MessageError::borrowed("decoding memory bound overflow"))
}

#[allow(unsafe_code, reason = "transfer the fixed boxed allocation to the original decoder vector")]
pub fn vector<T>(capacity: usize) -> Result<Vec<T>, MessageError> {
    std::alloc::Layout::array::<T>(capacity).map_err(|_| MessageError::borrowed("decoding container exceeds address space"))?;
    let allocation = Box::<[T]>::new_uninit_slice(capacity);
    let pointer = Box::into_raw(allocation) as *mut std::mem::MaybeUninit<T> as *mut T;
    // The Box and Vec layouts share T's size/alignment and the exact element
    // count. No elements are initialized; Vec's length is zero. Vec takes sole
    // allocation ownership, including the aligned dangling zero-sized case.
    Ok(unsafe { Vec::from_raw_parts(pointer, 0, capacity) })
}
