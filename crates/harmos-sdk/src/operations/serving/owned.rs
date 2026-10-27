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
use super::{Answer, Finalization, Finished, Prepare, Prepared, Route, ServingError, WireTable, guarded};
use crate::Message;
use crate::QualifiedId;
use crate::operations::{Dependencies, EndpointSet, Handler, ExecutionContext, OperationDeclaration};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct State<T: Handler> {
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

struct Physical<T: Handler> {
    dependencies: Dependencies,
    operation: Option<T>,
    request: T::Request,
    endpoints: T::StreamEndpoints,
    stopped: bool,
}

struct StreamEndpoints<E> {
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

impl WireTable {
    pub(crate) fn install<D, T>(&mut self, dependencies: Dependencies, contract: crate::operations::Contract) -> Result<(), ServingError>
    where
        D: OperationDeclaration,
        T: Handler<Request = D::Request, Response = D::Response>,
    {
        #[cfg(feature = "grpc")]
        let protobuf = D::GRPC.is_some().then(super::protobuf::Codec::of::<D>);
        #[cfg(feature = "streaming")]
        let limits = self.stream_limits;
        #[cfg(feature = "streaming")]
        if T::StreamEndpoints::item_types() != (None, None) && limits.is_none() {
            return Err(ServingError::StreamLimits);
        }
        #[cfg(not(feature = "streaming"))]
        if T::StreamEndpoints::item_types() != (None, None) {
            return Err(ServingError::StreamUnsupported);
        }
        let observe = std::any::TypeId::of::<D>() == std::any::TypeId::of::<crate::observers::Control>();
        self.bind_owned_contract::<D, T>(
            dependencies,
            contract,
            false,
            observe,
            move || {
                if let Some(endpoints) = T::StreamEndpoints::empty() {
                    return Ok(StreamEndpoints {
                        endpoints,
                        #[cfg(feature = "streaming")]
                        streams: None,
                    });
                }
                #[cfg(feature = "streaming")]
                {
                    let (endpoints, streams) = T::StreamEndpoints::bind_wire(limits.ok_or(ServingError::StreamLimits)?)?;
                    Ok(StreamEndpoints { endpoints, streams: Some(streams) })
                }
                #[cfg(not(feature = "streaming"))]
                Err(ServingError::StreamUnsupported)
            },
            #[cfg(feature = "grpc")]
            protobuf,
        )?;
        if observe {
            let id = QualifiedId::qualify(&self.owner, D::ID).map_err(|_| ServingError::Catalog)?;
            self.handlers.get_mut(id.as_str()).expect("installed observe control").resident = true;
        }
        Ok(())
    }

    pub fn install_lifecycle<D, T>(
        &mut self, dependencies: Dependencies, descriptor: harmos_domain::model::operations::lifecycle::Descriptor,
    ) -> Result<(), ServingError>
    where
        D: crate::OperationDeclaration,
        T: Handler<Request = D::Request, Response = D::Response, StreamEndpoints = ()>,
    {
        if self.closed || self.ordinary_closed {
            return Err(ServingError::Closing);
        }
        if descriptor.action().id() != D::ID
            || descriptor.contract().fingerprint()
                != (harmos_domain::operations::Entry::of::<D>().fingerprint)().map_err(|_| ServingError::Catalog)?
        {
            return Err(ServingError::Catalog);
        }
        self.bind_owned_contract::<D, T>(
            dependencies,
            descriptor.into(),
            true,
            true,
            || {
                Ok(StreamEndpoints {
                    endpoints: (),
                    #[cfg(feature = "streaming")]
                    streams: None,
                })
            },
            #[cfg(feature = "grpc")]
            None,
        )
    }

    fn bind_owned_contract<D, T>(
        &mut self, dependencies: Dependencies, contract: crate::operations::Contract, lifecycle: bool, builtin: bool,
        endpoints: impl Fn() -> Result<StreamEndpoints<T::StreamEndpoints>, ServingError> + Send + Sync + 'static,
        #[cfg(feature = "grpc")] protobuf: Option<super::protobuf::Codec>,
    ) -> Result<(), ServingError>
    where
        D: OperationDeclaration,
        T: Handler<Request = D::Request, Response = D::Response>,
    {
        let qualified = QualifiedId::qualify(&self.owner, D::ID).map_err(|_| ServingError::Catalog)?;
        let id = qualified.as_str();
        let (input, output) = T::StreamEndpoints::item_types();
        if (input, output) != crate::operations::item_types::<D>()
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
        let prepare: Prepare = Box::new(move |payload, grpc| {
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
                let mut request = None::<T::Request>;
                (codec.decode)(&payload, &mut request).map_err(ServingError::Codec)?;
                request.ok_or(ServingError::Catalog)?
            } else {
                T::Request::decode(&payload).map_err(ServingError::Codec)?
            };
            #[cfg(not(feature = "grpc"))]
            let request = T::Request::decode(&payload).map_err(ServingError::Codec)?;
            let grant = <D as OperationDeclaration>::grant(&request);
            let endpoints = endpoints()?;
            #[cfg(all(feature = "grpc", feature = "streaming"))]
            let endpoints = StreamEndpoints {
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
                    dependencies: dependencies.clone(),
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
        let handler = Route {
            callees: D::OPERATIONS,
            generation: None,
            lifecycle,
            builtin,
            resident: false,
            closing: false,
            definition: contract.operation().clone(),
            contract: Some(contract),
            prepare,
        };
        self.handlers.insert(id.to_owned(), handler);
        Ok(())
    }
}

impl<T: Handler> Prepared for State<T> {
    #[cfg(feature = "streaming")]
    fn streams(&self) -> Option<super::WireStreams> {
        self.streams.clone()
    }

    fn grant(&self) -> Option<crate::plugin::Grant> {
        self.grant.clone()
    }

    fn initialize<'a>(&'a mut self, context: &'a ExecutionContext) -> Finished<'a> {
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
            let context = context.for_registration(physical.dependencies.clone());
            let operation = guarded(async { T::initialize(context).await.map_err(operation_error::<T>) }).await?;
            physical.operation = Some(operation);
            self.initialization = Initialization::Ready;
            Ok(())
        })
    }

    fn attempt<'a>(&'a mut self, context: &'a ExecutionContext) -> Answer<'a> {
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
                let response = operation
                    .run(context.for_registration(physical.dependencies.clone()), &mut physical.request, &mut physical.endpoints)
                    .await
                    .map_err(operation_error::<T>)?;
                #[cfg(feature = "grpc")]
                if let Some(codec) = self.protobuf {
                    return (codec.encode)(&*response).map_err(ServingError::Codec);
                }
                Ok(response.encode())
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
fn operation_error<T: Handler>(error: T::Error) -> ServingError {
    let failure = crate::Failure::originate(T::classify(&error), &error, crate::failure::Origin::Plugin);
    ServingError::Handler(failure)
}

/* === Tests === */
