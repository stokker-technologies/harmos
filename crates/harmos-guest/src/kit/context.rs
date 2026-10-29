/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::UNKNOWN;

#[cfg(target_arch = "wasm32")]
static CHILD: fn(Vec<u8>) -> Child = |payload| Box::pin(super::plugin::__bindings::harmos::guest::operation_context::child(payload));

#[cfg(target_arch = "wasm32")]
type Child =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, super::plugin::__bindings::harmos::guest::capabilities::Refusal>>>>;

#[derive(Clone)]
pub struct Context {
    pub(super) context: Option<[u8; 32]>,
    pub(super) generation_context: Option<[u8; 32]>,
    pub(super) assignments: harmos_sdk::operations::OperationAssignments,
    pub(super) call: std::sync::Arc<crate::operations::CallContext>,
}

impl harmos_sdk::lifecycle::Context for Context {
    type Configuration = harmos_sdk::__private::WireTable;
}

impl harmos_sdk::plugin::Context for Context {
    fn assign_operations(&self, bytes: &[u8]) -> Result<(), harmos_sdk::operations::ServingError> {
        self.assignments.install(bytes)
    }

    fn with_generation(&self, token: [u8; 32]) -> Self {
        Self {
            context: self.context,
            generation_context: Some(token),
            assignments: self.assignments.clone(),
            call: self.call.clone(),
        }
    }
}

impl Context {
    pub fn context(&self) -> &crate::operations::CallContext {
        &self.call
    }

    pub fn with_value<T: crate::operations::ContextValue>(&self, value: T) -> Result<Self, crate::operations::ContextError> {
        let mut child = self.clone();
        child.call = std::sync::Arc::new(self.call.with_value(value)?);
        Ok(child)
    }

    pub(super) fn calling_context(&self) -> Result<[u8; 32], crate::OperationRefusal> {
        if self.context.is_none() && self.generation_context.is_none() {
            return Err(crate::OperationRefusal::new(UNKNOWN, "no guest calling context"));
        }
        Ok(self.context.unwrap_or([0; 32]))
    }
}

pub(super) fn retain() {
    // Keep The Fixed Async Import Even For An Empty Selected Catalog
    #[cfg(target_arch = "wasm32")]
    std::hint::black_box(&CHILD);
}
