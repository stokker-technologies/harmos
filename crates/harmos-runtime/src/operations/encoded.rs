/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Runtime-Owned Codecs Enter The Existing Native Admission Path.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::{Bindings, Contract, Lane};
use harmos_operations::execution::__private::{PreparedNative, ResourceSet};
use harmos_operations::execution::operations::{NativeInstance, ServingError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Prepare = dyn Fn(ResourceSet, &[u8]) -> Result<Box<dyn NativeInstance>, ServingError> + Send + Sync;
type DecodingMemory = dyn Fn(&[u8]) -> Result<usize, crate::MessageError> + Send + Sync;

/* === Structs === */

#[derive(Clone)]
pub struct EncodedBinding {
    pub(super) contract: Contract,
    prepare: Arc<Prepare>,
    decoding_memory: Arc<DecodingMemory>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl EncodedBinding {
    pub fn contract(&self) -> &Contract {
        &self.contract
    }

    // The Codec Owns Payload Validation And Its Decoded Allocation Estimate.
    pub fn new(
        contract: Contract, decoding_memory: impl Fn(&[u8]) -> Result<usize, crate::MessageError> + Send + Sync + 'static,
        prepare: impl Fn(ResourceSet, &[u8]) -> Result<Box<dyn NativeInstance>, ServingError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            contract,
            prepare: Arc::new(prepare),
            decoding_memory: Arc::new(decoding_memory),
        }
    }

    pub(super) fn decoding_memory(&self, payload: &[u8]) -> Result<usize, crate::MessageError> {
        (self.decoding_memory)(payload)
    }

    pub(super) fn prepare(&self, resources: ResourceSet, payload: &[u8]) -> Result<PreparedNative, ServingError> {
        Ok(PreparedNative {
            instance: (self.prepare)(resources, payload)?,
            grant: None,

            streams: None,
        })
    }
}

impl Bindings<'_> {
    pub fn encoded(&mut self, binding: EncodedBinding, lane: impl Lane) -> crate::Result<()> {
        let contract = &binding.contract;
        let validated = Contract::new_with_settings(
            contract.operation().clone(),
            contract.fingerprint(),
            contract.codec(),
            contract.schema().clone(),
            &self.operations.schema_settings,
        )
        .map_err(|source| crate::Error::OperationFailed {
            operation: contract.operation().id.to_string(),
            source: Arc::new(source),
        })?;
        if validated.input().is_some() || validated.output().is_some() || contract.grpc().is_some() {
            return Err(crate::Error::InvalidInput {
                field: "encoded binding",
                reason: "runtime bindings require a unary canonical contract without a gRPC projection",
            });
        }
        let definition = contract.operation().clone();
        let contract = contract.clone();
        let mut entry = self.operations.binding_erased(
            self.owner,
            lane.name(),
            Arc::new(binding),
            definition,
            Some(super::wire::dispatch_native),
            self.scope.cloned(),
        )?;
        Arc::get_mut(&mut entry).expect("unpublished encoded entry has one owner").contract = Some(contract);
        self.entries.push(entry);
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
