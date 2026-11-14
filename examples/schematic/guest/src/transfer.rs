/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Unary Guest Transfers

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos::plugin::{Context, Operation, Plugin};
use harmos::{Error, Visibility};
use schematic_protocol::{Batch, FileHash, FilesHashRequest, Hash, Put, Receipt, Record, Sample, Take};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn plugin() -> Plugin {
    let store = Arc::new(Mutex::new(BTreeMap::<u64, VecDeque<Batch>>::new()));
    let writes = Arc::clone(&store);
    let put = Operation::new("benchmark.put", async move |_: &Context, request: Put| {
        let batch = request.batch.ok_or_else(|| Error::Invalid("Batch Is Required".into()))?;
        let mut stores = writes.lock().unwrap();
        if request.first {
            stores.remove(&request.transfer);
        }
        let retained: usize = stores.values().flatten().map(memory).sum();
        if retained.saturating_add(memory(&batch)) > 64 * 1024 * 1024 {
            return Err(Error::Invalid("Transfer Storage Is Full".into()));
        }
        let receipt = Receipt {
            records: batch.records.len() as u64,
            samples: batch.samples.len() as u64,
            bytes: batch.bytes.len() as u64,
        };
        stores.entry(request.transfer).or_default().push_back(batch);
        Ok(receipt)
    })
    .visibility(Visibility::Public)
    .policy(super::POLICY);
    let take = Operation::new("benchmark.take", async move |_: &Context, request: Take| {
        let mut stores = store.lock().unwrap();
        let Some(batches) = stores.get_mut(&request.transfer) else {
            return Ok(Batch::default());
        };
        let batch = batches.pop_front().unwrap_or_default();
        if batches.is_empty() {
            stores.remove(&request.transfer);
        }
        Ok(batch)
    })
    .visibility(Visibility::Public)
    .policy(super::POLICY);
    let hash = Operation::new("benchmark.hash.file", async |context: &Context, request: Hash| {
        let request = request.request.ok_or_else(|| Error::Invalid("Hash Through Plugin: Missing Request".into()))?;
        let response = context.call(request).await?;
        Ok(FileHash {
            read_bytes: response.read_bytes,
            sha256: response.sha256,
        })
    })
    .visibility(Visibility::Public)
    .policy(super::POLICY);
    super::operations::plugin().operation(put).operation(take).operation(hash).requires::<FilesHashRequest>()
}

/* === Private Functions === */

fn memory(batch: &Batch) -> usize {
    batch
        .bytes
        .capacity()
        .saturating_add(batch.records.iter().map(|record| record.payload.capacity() + std::mem::size_of::<Record>()).sum::<usize>())
        .saturating_add(batch.samples.capacity() * std::mem::size_of::<Sample>())
}

/* === Tests === */
