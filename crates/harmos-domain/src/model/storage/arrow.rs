/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Arrow Data On The Wire: One IPC Message. The Runtime Decodes It With Its
// Allocation Preflight; The Domain Holds Only The Bounded Bytes.

use std::sync::Arc;

use super::codec::{Bytes, product};
use crate::MemorySize;

product!(ArrowIpc { ipc: Bytes => 1 });

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrowIpc {
    ipc: Vec<u8>,
}

impl ArrowIpc {
    pub fn new(ipc: Vec<u8>) -> Self {
        Self { ipc }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.ipc
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.ipc
    }
}

impl From<Arc<Vec<u8>>> for ArrowIpc {
    fn from(ipc: Arc<Vec<u8>>) -> Self {
        Self { ipc: Arc::unwrap_or_clone(ipc) }
    }
}

impl MemorySize for ArrowIpc {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.ipc.capacity())
    }
}
