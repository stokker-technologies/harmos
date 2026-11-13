/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Protocol Operation Admission and Routing

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::{Arc, Weak};

// Third Party Imports
use harmos_protocol::v1;

// Local Imports
use super::*;
use crate::operations::{OperationPolicy, ReplaySafety, Visibility};
use crate::plugin::grpc::{Activation, Error as PlacementError, invoke::Binding};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct ChildInvocation {
    pub invocation: crate::operations::EncodedInvocation,
    pub streams: Option<harmos_operations::execution::operations::WireStreams>,
}

pub(crate) struct Admission {
    registry: Weak<Registry>,
    entries: Vec<Arc<Entry>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Admission {
    pub(crate) fn remove(&self) {
        if let Some(registry) = self.registry.upgrade() {
            let mut state = registry.state.lock().expect("operation registry lock is healthy");
            for entry in &self.entries {
                if state.owned(entry.id(), entry.owner.as_str()).is_some_and(|current| Arc::ptr_eq(current, entry)) {
                    state.remove_owned(entry.id(), entry.owner.as_str());
                }
            }
        }
    }
}

impl Drop for Admission {
    fn drop(&mut self) {
        self.remove();
    }
}

impl Manager {
    pub(crate) fn register_grpc_clock(&self) -> Result<()> {
        let mut entries = Vec::new();
        use harmos_operations::execution::operations::OperationFactory;
        let factories = vec![
            OperationFactory::protobuf::<harmos_operations::clock::Sleep>(),
            OperationFactory::protobuf::<harmos_operations::clock::Get>(),
        ];
        #[cfg(feature = "grpc")]
        let factories = {
            let mut factories = factories;
            factories.push(OperationFactory::protobuf::<crate::operations::values::Get>());
            factories
        };
        let mut both_faces = Vec::new();
        for factory in factories {
            let factory = factory.map_err(|source| Error::OperationFailed {
                operation: "Time".into(),
                source: Arc::new(source),
            })?;
            if let Some(context) = factory
                .contract()
                .map_err(|source| Error::OperationFailed {
                    operation: "Time".into(),
                    source: Arc::new(source),
                })?
                .context(&self.schema_settings)
                .map_err(|source| Error::OperationFailed {
                    operation: "Time".into(),
                    source: Arc::new(source),
                })?
            {
                both_faces.push(factory.clone().with_contract(context));
            }
            both_faces.push(factory);
        }
        for factory in both_faces {
            let contract = factory.contract().map_err(|source| Error::OperationFailed {
                operation: factory.definition.id.to_string(),
                source: Arc::new(source),
            })?;
            let definition = factory.definition.clone();
            let mut entry = self.binding_erased(
                "host",
                crate::operations::lane::RESIDENT,
                Arc::new(factory),
                definition,
                Some(crate::operations::wire::dispatch_native),
                #[cfg(feature = "plugin-guest")]
                None,
            )?;
            Arc::get_mut(&mut entry).expect("unpublished clock entry").contract = Some(contract.clone());
            #[cfg(feature = "grpc")]
            {
                if contract.grpc().is_some() {
                    Arc::get_mut(&mut entry).expect("unpublished clock entry").grpc = Some(crate::grpc::native::serve);
                }
            }
            entries.push(entry);
        }
        self.reserve(&entries)
    }

    pub(crate) fn admit_grpc(&self, activation: &Activation, lane: &'static str) -> std::result::Result<Admission, PlacementError> {
        let catalog = activation.description.catalog.as_ref().unwrap();
        let mut entries = Vec::new();
        for contract in &catalog.operations {
            let definition = OperationDefinition {
                id: Identifier::new(&contract.id).map_err(|error| PlacementError::Invalid(error.to_string()))?,
                description: contract.description.clone(),
                visibility: match contract.visibility() {
                    v1::Visibility::Protected => Visibility::Protected,
                    v1::Visibility::Public => Visibility::Public,
                    _ => return Err(PlacementError::Invalid(format!("Invalid Visibility for {}", contract.id))),
                },
                policy: policy(contract.policy.as_ref()).map_err(|error| PlacementError::Invalid(error.to_string()))?,
                replay: if contract.replay_safe { ReplaySafety::Idempotent } else { ReplaySafety::Unknown },
                fingerprint: harmos_domain::Fingerprint(contract.fingerprint.clone().try_into().unwrap()),
            };
            let binding = Arc::new(Binding::new(activation, contract.clone()));
            let mut entry = self
                .binding_erased(
                    &activation.description.id,
                    lane,
                    binding,
                    definition,
                    Some(crate::operations::wire::dispatch_native),
                    #[cfg(feature = "plugin-guest")]
                    None,
                )
                .map_err(|error| PlacementError::Invalid(error.to_string()))?;
            use prost::Message;
            let types = catalog.types.as_ref().expect("admitted catalog types");
            let protobuf = harmos_operations::operations::Protobuf {
                descriptors: harmos_protocol::schema::operation_types(types, &[&contract.request, &contract.response])
                    .map_err(|error| PlacementError::Invalid(error.to_string()))?
                    .encode_to_vec(),
                request: contract.request.clone(),
                response: contract.response.clone(),
                input: None,
                output: None,
            };
            let metadata = crate::operations::Contract::from_protobuf(entry.definition.clone(), protobuf, None, &self.schema_settings)
                .map_err(|error| PlacementError::Invalid(error.to_string()))?;
            Arc::get_mut(&mut entry).expect("unpublished plugin entry").contract = Some(metadata);
            entries.push(entry);
        }
        self.reserve(&entries).map_err(|error| PlacementError::Invalid(error.to_string()))?;
        Ok(Admission {
            registry: Arc::downgrade(&self.registry),
            entries,
        })
    }
}

impl crate::operations::Context<'_> {
    pub(crate) async fn invoke_client(&self, request: v1::InvocationsInvokeRequest) -> Result<Vec<u8>> {
        let entry = self.operations.capture_entry(&request.operation)?;
        if !entry.client_visible() {
            return Err(Error::OperationUnauthorized {
                caller: "client".into(),
                owner: entry.owner.to_string(),
                operation: request.operation,
            });
        }
        if fingerprint(&entry)? != request.fingerprint {
            return Err(Error::InvalidInput {
                field: "Invoke fingerprint",
                reason: "Invoke requires the admitted operation fingerprint",
            });
        }
        self.submit_encoded(&request.operation, request.payload)?.await
    }

    pub(crate) fn submit_grpc_child(
        &self, owner: &str, requires: &[v1::Requirement], declarations: &[v1::SettingDeclaration], child: v1::ChildRequest,
    ) -> Result<ChildInvocation> {
        if child.background && child.operation != "harmos.context.v1.Time/Sleep" {
            return Err(Error::InvalidInput {
                field: "child policy timer",
                reason: "Invoke permits background waits only for context Time.Sleep",
            });
        }
        let entry = self.operations.capture_entry(&child.operation)?;
        entry.available()?;
        let denied = || Error::OperationUnauthorized {
            caller: owner.into(),
            owner: entry.owner.to_string(),
            operation: child.operation.clone(),
        };
        let plugin = entry.binding.downcast_ref::<Binding>();
        if entry.owner.as_str() != owner && entry.definition.visibility != Visibility::Public {
            return Err(denied());
        }
        if !entry.context_visible() {
            return Err(denied());
        }
        let required = requires.iter().find(|required| required.id == child.operation).ok_or_else(denied)?;
        if required.fingerprint != child.fingerprint || fingerprint(&entry)? != child.fingerprint {
            return Err(Error::InvalidInput {
                field: "child fingerprint",
                reason: "callee differs from the declared requirement",
            });
        }
        let output = entry.contract.as_ref().is_some_and(|contract| contract.output().is_some());
        let input = entry.contract.as_ref().is_some_and(|contract| contract.input().is_some());
        if input || output != child.watch || (child.watch && child.max_response_bytes == 0) {
            return Err(Error::InvalidInput {
                field: "child operation",
                reason: "Watch requires a declared server stream and a consumer item limit",
            });
        }
        let prepared = if let Some(binding) = plugin {
            harmos_operations::execution::__private::PreparedNative {
                instance: binding.prepare(child.payload),
                grant: None,
                streams: None,
            }
        } else {
            let factory =
                entry.binding.downcast_ref::<harmos_operations::execution::operations::OperationFactory>().ok_or(Error::InvalidInput {
                    field: "child operation",
                    reason: "host operation has no protobuf codec",
                })?;
            let resources = self.operations.resources_for(&entry);
            #[cfg(feature = "grpc")]
            let resources = {
                let mut resources = resources;
                resources.replace(Arc::new(crate::operations::values::Caller {
                    plugin: owner.into(),
                    declarations: declarations.to_vec(),
                }));
                resources
            };
            #[cfg(not(feature = "grpc"))]
            let _ = declarations;
            factory
                .prepare_grpc(
                    resources,
                    &child.payload,
                    harmos_operations::execution::operations::WireStreamLimits {
                        input: crate::operations::StreamSettings {
                            items: std::num::NonZeroUsize::MIN,
                            bytes: std::num::NonZeroUsize::MIN,
                        },
                        output: crate::operations::StreamSettings {
                            items: std::num::NonZeroUsize::MIN,
                            bytes: std::num::NonZeroUsize::new(
                                usize::try_from(child.max_response_bytes)
                                    .map_err(|_| Error::InvalidInput {
                                        field: "Watch item bytes",
                                        reason: "limit exceeds the address space",
                                    })?
                                    .max(1),
                            )
                            .unwrap(),
                        },
                    },
                )
                .map_err(|source| Error::OperationFailed {
                    operation: child.operation.clone(),
                    source: Arc::new(source),
                })?
        };
        let streams = prepared.streams.clone();
        let override_policy = child.policy.as_ref().map(|value| policy(Some(value))).transpose()?;
        let invocation = self.operations.dispatch_grpc_prepared(entry, prepared, self, override_policy)?;
        Ok(ChildInvocation { invocation, streams })
    }
}

impl Manager {
    fn dispatch_grpc_prepared(
        &self, entry: Arc<Entry>, prepared: harmos_operations::execution::__private::PreparedNative,
        context: &crate::operations::Context<'_>, override_policy: Option<OperationPolicy>,
    ) -> Result<crate::operations::EncodedInvocation> {
        let mut request = crate::operations::binding::Request::from(Vec::new());
        request.policy = override_policy;
        crate::operations::wire::dispatch_prepared(
            self,
            Submission {
                submitted: self.clock().now(),
                entry,
                cause: None,
                preparation: None,
            },
            Source::Host,
            Some(crate::operations::wire::Parent {
                scope: &context.scope,
                ancestors: context.admission.held(),
            }),
            prepared,
            request,
        )
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn fingerprint(entry: &Entry) -> Result<Vec<u8>> {
    if let Some(binding) = entry.binding.downcast_ref::<Binding>() {
        return Ok(binding.contract.fingerprint.clone());
    }
    let contract = entry.contract.as_ref().ok_or(Error::InvalidInput {
        field: "host contract",
        reason: "protobuf contract required",
    })?;
    if contract.input().is_some() {
        return Err(Error::InvalidInput {
            field: "child operation",
            reason: "child calls do not accept input streams",
        });
    }
    if contract.protobuf().is_some() {
        return Ok(contract.fingerprint().0.to_vec());
    }
    let binding = contract.grpc().ok_or(Error::InvalidInput {
        field: "host contract",
        reason: "child calls require the host operation's protobuf projection",
    })?;
    let (package, service, method) = (binding.package.as_ref(), binding.service.as_ref(), binding.method.as_ref());
    let service = crate::grpc::descriptor(package, service, &[(method, contract)]).map_err(|source| Error::OperationFailed {
        operation: entry.id().into(),
        source: Arc::new(source),
    })?;
    let method = service.methods().find(|candidate| candidate.name() == method).unwrap();
    harmos_protocol::schema::fingerprint(
        entry.id(),
        &prost_types::FileDescriptorSet {
            file: service.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        },
        method.input().full_name(),
        method.output().full_name(),
    )
    .map_err(|source| Error::OperationFailed {
        operation: entry.id().into(),
        source: Arc::new(source),
    })
}

fn policy(policy: Option<&v1::OperationPolicy>) -> Result<OperationPolicy> {
    OperationPolicy::try_from(policy).map_err(|_| Error::InvalidInput {
        field: "operation policy",
        reason: "invalid operation policy",
    })
}

/* === Tests === */
