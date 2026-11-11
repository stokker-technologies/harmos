/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Runtime-Defined Canonical Unary Operations Use The Existing Wire Lifecycle.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{Answer, Finalization, Finished, Prepared, Route, ServingError, WireTable, guarded};
use crate::execution::operations::{Contract, ExecutionContext, NativeInstance};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct State {
    instance: Option<Box<dyn NativeInstance>>,
    initialized: bool,
    initializing: bool,
    closing: bool,
    finalization: Option<Finalization<Box<dyn NativeInstance>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl WireTable {
    // The Returned NativeInstance Uses Vec<u8> As Its Encoded Response Payload.
    pub fn encoded(
        &mut self, contract: Contract, prepare: impl Fn(&[u8]) -> Result<Box<dyn NativeInstance>, ServingError> + Send + Sync + 'static,
    ) -> Result<(), ServingError> {
        if self.closed || self.ordinary_closed {
            return Err(ServingError::Closing);
        }
        if contract.input().is_some() || contract.output().is_some() || contract.grpc().is_some() {
            return Err(ServingError::Catalog);
        }
        let id = crate::execution::QualifiedId::qualify(&self.owner, contract.operation().id.as_str())
            .map_err(|_| ServingError::Catalog)?
            .as_str()
            .to_owned();
        if self.handlers.contains_key(&id) || self.services.contains_key(&id) {
            return Err(ServingError::Catalog);
        }
        let route = Route {
            callees: &[],
            lifecycle: false,
            builtin: false,
            resident: false,
            definition: contract.operation().clone(),
            generation: None,
            closing: false,
            contract: Some(contract),
            prepare: Box::new(move |payload, alternate_codec| {
                if alternate_codec {
                    return Err(ServingError::Catalog);
                }
                let instance = prepare(&payload)?;
                Ok(Box::new(State {
                    instance: Some(instance),
                    initialized: false,
                    initializing: false,
                    closing: false,
                    finalization: None,
                }))
            }),
        };
        self.handlers.insert(id, route);
        Ok(())
    }
}

impl Prepared for State {
    fn bind<'a>(&'a mut self, context: &'a ExecutionContext) -> Finished<'a> {
        Box::pin(async move {
            if self.closing {
                return Err(ServingError::Closing);
            }
            if self.initialized {
                return Ok(());
            }
            if self.initializing {
                return Err(ServingError::InitializationStarted);
            }
            self.initializing = true;
            let instance = self.instance.as_mut().ok_or(ServingError::Closing)?;
            guarded(instance.initialize(context.clone())).await?;
            self.initialized = true;
            Ok(())
        })
    }

    fn attempt<'a>(&'a mut self, context: &'a ExecutionContext) -> Answer<'a> {
        Box::pin(async move {
            if self.closing {
                return Err(ServingError::Closing);
            }
            if !self.initialized {
                return Err(ServingError::Uninitialized);
            }
            let instance = self.instance.as_mut().ok_or(ServingError::Closing)?;
            let response = guarded(instance.run(context.clone())).await?;
            response.value::<Vec<u8>>().cloned().ok_or(ServingError::Catalog)
        })
    }

    fn finalize(&mut self) -> Finished<'_> {
        self.closing = true;
        if self.finalization.is_none() {
            let mut instance = self.instance.take().expect("original encoded instance for cleanup");
            self.finalization = Some(Box::pin(async move {
                let result = guarded(async {
                    instance.stop().await?;
                    instance.cleanup().await
                })
                .await;
                (instance, result)
            }));
        }
        Box::pin(async move {
            let (instance, result) = self.finalization.as_mut().expect("original cleanup future").await;
            self.instance = Some(instance);
            self.finalization = None;
            result
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
