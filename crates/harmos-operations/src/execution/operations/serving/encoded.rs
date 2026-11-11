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
    grant: Option<crate::execution::plugin::Grant>,
    #[cfg(feature = "streaming")]
    streams: Option<crate::execution::operations::WireStreams>,
    initialized: bool,
    initializing: bool,
    closing: bool,
    finalization: Option<Finalization<Box<dyn NativeInstance>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl WireTable {
    pub(super) fn install_factory(
        &mut self, factory: crate::execution::operations::OperationFactory, lifecycle: bool,
    ) -> Result<(), ServingError> {
        factory.validate()?;
        factory.verify_resources(&self.resources, &self.provided)?;
        let contract = factory.contract()?;
        self.install_contract(factory, contract, lifecycle)
    }

    pub(super) fn install_contract(
        &mut self, factory: crate::execution::operations::OperationFactory, contract: Contract, lifecycle: bool,
    ) -> Result<(), ServingError> {
        let id = harmos_domain::QualifiedId::qualify(&self.owner, contract.operation().id.as_str())
            .map_err(|_| ServingError::Catalog)?
            .as_str()
            .to_owned();
        if self.closed || self.ordinary_closed {
            return Err(ServingError::Closing);
        }
        if self.handlers.contains_key(&id) || self.services.contains_key(&id) {
            return Err(ServingError::Catalog);
        }
        let builtin = lifecycle || factory.builtin;
        let context = ExecutionContext::new(self.trusted_resources.clone(), Default::default());
        let resources = context.with_resources(self.resources.clone()).resources().clone();
        #[cfg(feature = "streaming")]
        let limits = self.stream_limits;
        #[cfg(not(feature = "streaming"))]
        let limits = None;
        let route = Route {
            callees: factory.operations,
            lifecycle,
            builtin,
            resident: factory.builtin,
            definition: contract.operation().clone(),
            generation: None,
            closing: false,
            contract: Some(contract),
            prepare: Box::new(move |payload, protobuf| {
                let prepared = factory.prepare_envelope(resources.clone(), &payload, protobuf, limits)?;
                Ok(Box::new(State {
                    instance: Some(prepared.instance),
                    grant: prepared.grant,
                    #[cfg(feature = "streaming")]
                    streams: prepared.streams,
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
                    grant: None,
                    #[cfg(feature = "streaming")]
                    streams: None,
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
    fn grant(&self) -> Option<harmos_domain::plugin::Grant> {
        self.grant.clone()
    }
    #[cfg(feature = "streaming")]
    fn streams(&self) -> Option<crate::execution::operations::WireStreams> {
        self.streams.clone()
    }

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
            guarded(instance.initialize(context.clone())).await.map_err(|error| failure(&**instance, error))?;
            self.initialized = true;
            Ok(())
        })
    }

    fn attempt<'a>(&'a mut self, context: &'a ExecutionContext, epoch: Option<u64>) -> Answer<'a> {
        Box::pin(async move {
            if self.closing {
                return Err(ServingError::Closing);
            }
            if !self.initialized {
                return Err(ServingError::Uninitialized);
            }
            let instance = self.instance.as_mut().ok_or(ServingError::Closing)?;
            #[cfg(feature = "streaming")]
            let response = match epoch {
                Some(epoch) => guarded(instance.run_stream(context.clone(), epoch)).await.map_err(|error| failure(&**instance, error))?,
                None => guarded(instance.run(context.clone())).await.map_err(|error| failure(&**instance, error))?,
            };
            #[cfg(not(feature = "streaming"))]
            let response = {
                let _ = epoch;
                guarded(instance.run(context.clone())).await.map_err(|error| failure(&**instance, error))?
            };
            response.restore::<Vec<u8>>().map(|response| (*response).clone()).ok_or(ServingError::Catalog)
        })
    }

    fn finalize(&mut self) -> Finished<'_> {
        self.closing = true;
        if self.finalization.is_none() {
            let mut instance = self.instance.take().expect("original encoded instance for cleanup");
            self.finalization = Some(Box::pin(async move {
                let result = guarded(async {
                    instance.stop().await.map_err(|error| failure(&*instance, error))?;
                    instance.cleanup().await.map_err(|error| failure(&*instance, error))
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

fn failure(instance: &dyn NativeInstance, error: ServingError) -> ServingError {
    match error {
        ServingError::Native(source) => {
            ServingError::Handler(crate::execution::operations::StartupError::failure(source.as_ref(), instance.classify(source.as_ref())))
        }
        error => error,
    }
}
