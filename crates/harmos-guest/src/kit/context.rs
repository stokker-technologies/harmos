/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use super::UNKNOWN;

/* === Constants === */
#[cfg(target_arch = "wasm32")]
static CHILD: fn(Vec<u8>) -> Child =
    |payload| Box::pin(super::plugin::__bindings::harmos::guest::operation_context::child(payload));

/* === Macros === */

/* === Type Aliases === */
#[cfg(target_arch = "wasm32")]
type Child = std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<
                    Vec<u8>,
                    super::plugin::__bindings::harmos::guest::capabilities::Refusal,
                >,
            >,
    >,
>;

/* === Structs === */
#[derive(Clone)]
pub struct Context {
    pub(super) context: Option<[u8; 32]>,
    pub(super) generation_context: Option<[u8; 32]>,
    pub(super) call: std::sync::Arc<crate::operations::CallContext>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos_contract::plugin::Context for Context {
    fn with_generation(&self, token: [u8; 32]) -> Self {
        Self {
            context: self.context,
            generation_context: Some(token),
            call: self.call.clone(),
        }
    }
}

impl Context {
    pub fn context(&self) -> &crate::operations::CallContext {
        &self.call
    }

    pub fn with_value<T: crate::operations::ContextValue>(
        &self,
        value: T,
    ) -> Result<Self, crate::operations::ContextError> {
        let mut child = self.clone();
        child.call = std::sync::Arc::new(self.call.with_value(value)?);
        Ok(child)
    }

    pub(super) fn calling_context(&self) -> Result<[u8; 32], crate::OperationRefusal> {
        if self.context.is_none() && self.generation_context.is_none() {
            return Err(crate::OperationRefusal::new(
                UNKNOWN,
                "no guest calling context",
            ));
        }
        Ok(self.context.unwrap_or([0; 32]))
    }
}

/* === Public Functions === */
pub(super) fn retain() {
    // Keep The Fixed Async Import Even For An Empty Selected Catalog
    #[cfg(target_arch = "wasm32")]
    std::hint::black_box(&CHILD);
}

/* === Private Functions === */

/* === Tests === */
