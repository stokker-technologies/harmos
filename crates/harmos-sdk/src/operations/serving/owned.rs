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
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::{Answer, Finalization, Finished, Handler, Prepare, Prepared, ServingError, WireTable, guarded};
use crate::Message;
use crate::operations::{EndpointSet, Operation, OperationDeclaration, WireDeclaration};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct State<T: Operation> {
    physical: Option<Physical<T>>,
    #[cfg(feature = "streaming")]
    streams: Option<super::WireStreams>,
    initialization: Initialization,
    closing: bool,
    finalization: Option<Finalization<Physical<T>>>,
    #[cfg(feature = "grpc")]
    protobuf: Option<super::protobuf::Codec>,
    // The grant the decoded request needs; the host checks it before any attempt.
    grant: Option<crate::plugin::Grant>,
}

struct Physical<T: Operation> {
    resources: Arc<T::Resources>,
    operation: Option<T>,
    request: T::Declaration,
    endpoints: T::Endpoints,
    stopped: bool,
}

struct Endpoints<E> {
    endpoints: E,
    #[cfg(feature = "streaming")]
    streams: Option<super::WireStreams>,
}

/* === Enums === */
enum Initialization {
    Pending,
    Started,
    Ready,
}

/* === Traits === */

/* === Implementations === */
impl<C: Send + Sync + 'static> WireTable<C> {
    pub fn bind_operation<T>(&mut self, resources: Arc<T::Resources>) -> Result<(), ServingError>
    where
        T: for<'a> Operation<Context<'a> = &'a C, Endpoints = ()>,
    {
        if self.closed || self.ordinary_closed {
            return Err(ServingError::Closing);
        }
        let contract = WireDeclaration::of::<T::Declaration>().contract()?;
        if contract.input().is_some() || contract.output().is_some() {
            return Err(ServingError::Streaming { operation: T::Declaration::ID.to_owned() });
        }
        #[cfg(feature = "grpc")]
        let protobuf = T::Declaration::GRPC.is_some().then(super::protobuf::Codec::of::<T::Declaration>);
        self.bind_owned_contract::<T>(
            resources,
            contract,
            false,
            false,
            || {
                Ok(Endpoints {
                    endpoints: (),
                    #[cfg(feature = "streaming")]
                    streams: None,
                })
            },
            #[cfg(feature = "grpc")]
            protobuf,
        )
    }

    // Bind a resident control: it runs until the host cancels it and is not
    // charged against the ordinary invocation capacity.
    pub fn bind_resident_operation<T>(&mut self, resources: Arc<T::Resources>) -> Result<(), ServingError>
    where
        T: for<'a> Operation<Context<'a> = &'a C, Endpoints = ()>,
    {
        let policy = T::Declaration::POLICY;
        if policy.timeout != crate::operations::UNBOUNDED || policy.attempt_timeout != crate::operations::UNBOUNDED {
            return Err(ServingError::Catalog);
        }
        self.bind_operation::<T>(resources)?;
        if let Some(handler) = self.handlers.get_mut(T::Declaration::ID) {
            handler.resident = true;
        }
        Ok(())
    }

    // Bind the built-in observe control a sidecar serves for its observers.
    pub fn bind_observe_control<T>(&mut self, resources: Arc<T::Resources>) -> Result<(), ServingError>
    where
        T: for<'a> Operation<Context<'a> = &'a C, Endpoints = (), Declaration = crate::observers::Control>,
    {
        if self.closed || self.ordinary_closed {
            return Err(ServingError::Closing);
        }
        self.bind_owned_contract::<T>(
            resources,
            crate::observers::contract()?,
            false,
            true,
            || {
                Ok(Endpoints {
                    endpoints: (),
                    #[cfg(feature = "streaming")]
                    streams: None,
                })
            },
            #[cfg(feature = "grpc")]
            None,
        )?;
        if let Some(handler) = self.handlers.get_mut(crate::observers::Control::ID) {
            handler.resident = true;
        }
        Ok(())
    }

    #[cfg(feature = "streaming")]
    pub fn bind_streaming_operation<T>(&mut self, resources: Arc<T::Resources>, limits: super::WireStreamLimits) -> Result<(), ServingError>
    where
        T: for<'a> Operation<Context<'a> = &'a C>,
        T::Endpoints: super::WireEndpoints,
    {
        use super::WireEndpoints;
        if self.closed || self.ordinary_closed {
            return Err(ServingError::Closing);
        }
        if T::Endpoints::item_types() == (None, None) {
            return Err(ServingError::StreamUnsupported);
        }
        limits.validate()?;
        let contract = WireDeclaration::of::<T::Declaration>().contract()?;
        #[cfg(feature = "grpc")]
        let protobuf = T::Declaration::GRPC.is_some().then(super::protobuf::Codec::of::<T::Declaration>);
        self.bind_owned_contract::<T>(
            resources,
            contract,
            false,
            false,
            move || {
                let (endpoints, streams) = T::Endpoints::bind(limits)?;
                Ok(Endpoints { endpoints, streams: Some(streams) })
            },
            #[cfg(feature = "grpc")]
            protobuf,
        )
    }

    #[cfg(feature = "streaming")]
    pub(super) fn bind_catalog_operation<T>(&mut self, resources: Arc<T::Resources>) -> Result<(), ServingError>
    where
        T: for<'a> Operation<Context<'a> = &'a C>,
        T::Endpoints: super::WireEndpoints,
    {
        use super::WireEndpoints;
        let limits = self.stream_limits;
        if T::Endpoints::item_types() != (None, None) && limits.is_none() {
            return Err(ServingError::StreamLimits);
        }
        let contract = WireDeclaration::of::<T::Declaration>().contract()?;
        #[cfg(feature = "grpc")]
        let protobuf = T::Declaration::GRPC.is_some().then(super::protobuf::Codec::of::<T::Declaration>);
        self.bind_owned_contract::<T>(
            resources,
            contract,
            false,
            false,
            move || {
                if let Some(endpoints) = T::Endpoints::empty() {
                    return Ok(Endpoints { endpoints, streams: None });
                }
                let (endpoints, streams) = T::Endpoints::bind(limits.ok_or(ServingError::StreamLimits)?)?;
                Ok(Endpoints { endpoints, streams: Some(streams) })
            },
            #[cfg(feature = "grpc")]
            protobuf,
        )
    }

    pub(crate) fn bind_owned_lifecycle<T>(
        &mut self, resources: Arc<T::Resources>, descriptor: crate::operations::lifecycle::Descriptor,
    ) -> Result<(), ServingError>
    where
        T: for<'a> Operation<Context<'a> = &'a C, Endpoints = ()>,
        T::Declaration: crate::operations::lifecycle::SdkOperation,
    {
        use crate::operations::lifecycle::SdkOperation;
        if self.closed || self.ordinary_closed {
            return Err(ServingError::Closing);
        }
        if descriptor.action() != T::Declaration::ACTION {
            return Err(ServingError::Catalog);
        }
        self.bind_owned_contract::<T>(
            resources,
            descriptor.contract().clone(),
            true,
            true,
            || {
                Ok(Endpoints {
                    endpoints: (),
                    #[cfg(feature = "streaming")]
                    streams: None,
                })
            },
            #[cfg(feature = "grpc")]
            None,
        )
    }

    fn bind_owned_contract<T>(
        &mut self, resources: Arc<T::Resources>, contract: crate::operations::Contract, lifecycle: bool, builtin: bool,
        endpoints: impl Fn() -> Result<Endpoints<T::Endpoints>, ServingError> + Send + Sync + 'static,
        #[cfg(feature = "grpc")] protobuf: Option<super::protobuf::Codec>,
    ) -> Result<(), ServingError>
    where
        T: for<'a> Operation<Context<'a> = &'a C>,
    {
        let id = T::Declaration::ID;
        let (input, output) = T::Endpoints::item_types();
        if (input, output) != crate::operations::item_types::<T::Declaration>()
            || input.is_some() != contract.input().is_some()
            || output.is_some() != contract.output().is_some()
        {
            return Err(ServingError::Catalog);
        }
        if self.handlers.contains_key(id) {
            return Err(crate::Error::DuplicateOperation { id: id.to_owned() }.into());
        }
        let marked = contract.grpc().is_some();
        #[cfg(not(feature = "grpc"))]
        if marked {
            return Err(ServingError::Codec(crate::MessageError::borrowed("gRPC operation requires provider protobuf support")));
        }
        #[cfg(feature = "grpc")]
        if marked && protobuf.is_none() {
            return Err(ServingError::Codec(crate::MessageError::borrowed("gRPC operation has no provider protobuf projection")));
        }
        let prepare: Prepare<C> = Box::new(move |payload, grpc| {
            if grpc && !marked {
                return Err(ServingError::Codec(crate::MessageError::borrowed("operation has no gRPC contract")));
            }
            #[cfg(not(feature = "grpc"))]
            if grpc {
                return Err(ServingError::Codec(crate::MessageError::borrowed("provider protobuf codec is disabled")));
            }
            #[cfg(feature = "grpc")]
            let selected = if grpc {
                Some(protobuf.ok_or_else(|| ServingError::Codec(crate::MessageError::borrowed("provider has no protobuf codec")))?)
            } else {
                None
            };
            #[cfg(feature = "grpc")]
            let request = if let Some(codec) = selected {
                let mut request = None::<T::Declaration>;
                (codec.decode)(&payload, &mut request).map_err(ServingError::Codec)?;
                request.ok_or(ServingError::Catalog)?
            } else {
                T::Declaration::decode(&payload).map_err(ServingError::Codec)?
            };
            #[cfg(not(feature = "grpc"))]
            let request = T::Declaration::decode(&payload).map_err(ServingError::Codec)?;
            let grant = <T::Declaration as OperationDeclaration>::grant(&request);
            let endpoints = endpoints()?;
            #[cfg(all(feature = "grpc", feature = "streaming"))]
            let endpoints = Endpoints {
                endpoints: endpoints.endpoints,
                streams: match (endpoints.streams, selected) {
                    (Some(streams), Some(codec)) => Some(streams.protobuf(codec)?),
                    (streams, _) => streams,
                },
            };
            let state = State::<T> {
                #[cfg(feature = "streaming")]
                streams: endpoints.streams,
                physical: Some(Physical {
                    resources: resources.clone(),
                    operation: None,
                    request,
                    endpoints: endpoints.endpoints,
                    stopped: false,
                }),
                initialization: Initialization::Pending,
                closing: false,
                finalization: None,
                #[cfg(feature = "grpc")]
                protobuf: selected,
                grant,
            };
            Ok(Box::new(state))
        });
        let handler =
            Handler { lifecycle, builtin, resident: false, definition: contract.operation().clone(), contract: Some(contract), prepare };
        self.handlers.insert(id.to_owned(), handler);
        Ok(())
    }
}

impl<T, C> Prepared<C> for State<T>
where
    C: Send + Sync + 'static,
    T: for<'a> Operation<Context<'a> = &'a C>,
{
    #[cfg(feature = "streaming")]
    fn streams(&self) -> Option<super::WireStreams> {
        self.streams.clone()
    }

    fn grant(&self) -> Option<crate::plugin::Grant> {
        self.grant.clone()
    }

    fn initialize<'a>(&'a mut self, context: &'a C) -> Finished<'a> {
        Box::pin(async move {
            if self.closing {
                return Err(ServingError::Closing);
            }
            match self.initialization {
                Initialization::Ready => return Ok(()),
                Initialization::Started => return Err(ServingError::InitializationStarted),
                Initialization::Pending => self.initialization = Initialization::Started,
            }
            let physical = self.physical.as_mut().expect("original operation before cleanup");
            let resources = Arc::clone(&physical.resources);
            let operation = guarded(async { T::initialize(resources, context).await.map_err(operation_error::<T>) }).await?;
            physical.operation = Some(operation);
            self.initialization = Initialization::Ready;
            Ok(())
        })
    }

    fn attempt<'a>(&'a mut self, context: &'a C) -> Answer<'a> {
        Box::pin(async move {
            if self.closing {
                return Err(ServingError::Closing);
            }
            if !matches!(self.initialization, Initialization::Ready) {
                return Err(ServingError::Uninitialized);
            }
            let physical = self.physical.as_mut().expect("original initialized operation");
            guarded(async {
                let operation = physical.operation.as_mut().expect("initialized operation");
                let response =
                    operation.run(context, &mut physical.request, &mut physical.endpoints).await.map_err(operation_error::<T>)?;
                #[cfg(feature = "grpc")]
                if let Some(codec) = self.protobuf {
                    return (codec.encode)(&*response).map_err(ServingError::Codec);
                }
                Ok(response.encode_to_vec())
            })
            .await
        })
    }

    fn finalize(&mut self) -> Finished<'_> {
        self.closing = true;
        if self.finalization.is_none() {
            let mut physical = self.physical.take().expect("original operation for cleanup");
            let future = async move {
                let result = guarded(async {
                    // An Operation That Never Initialized Owns Nothing To Stop
                    let Some(operation) = physical.operation.as_mut() else {
                        return Ok(());
                    };
                    if !physical.stopped {
                        operation.stop().await.map_err(operation_error::<T>)?;
                        physical.stopped = true;
                    }
                    operation.cleanup().await.map_err(operation_error::<T>)
                })
                .await;
                (physical, result)
            };
            self.finalization = Some(Box::pin(future));
        }
        Box::pin(async move {
            let (physical, result) = self.finalization.as_mut().expect("original cleanup future").await;
            self.physical = Some(physical);
            self.finalization = None;
            result
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */
// The handler's error becomes the failure's cause here, in the plugin process
// where it originated; every later carrier forwards the same value.
fn operation_error<T: Operation>(error: T::Error) -> ServingError {
    let failure = crate::Failure::originate(T::classify(&error), &error, crate::failure::Origin::Plugin);
    ServingError::Handler(failure)
}

/* === Tests === */
