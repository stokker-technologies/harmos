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

use harmos_protocol::host::InvocationsCreateRequest;
use harmos_protocol::plugin::ChildRequest;

// Standard Library Imports
use std::sync::{Arc, Weak};

// Third Party Imports
use harmos_protocol::model;

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
    manager: Manager,
    replacing: Option<String>,
    previous: Vec<Arc<Entry>>,
    staged: bool,
    sealed: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Admission {
    pub(crate) fn commit(&mut self) -> Result<()> {
        if self.staged {
            self.previous = self.manager.reserve_inner(&self.entries, self.replacing.as_deref(), true)?;
            self.staged = false;
        }
        Ok(())
    }
    pub(crate) fn seal(&mut self) {
        self.sealed = true;
        for entry in &self.previous {
            entry.active.store(false, std::sync::atomic::Ordering::Release);
            for control in entry.pending.lock().expect("operation settlement lock is healthy").values() {
                control.request_cancel();
            }
        }
    }

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
        if !self.sealed
            && !self.staged
            && let Some(registry) = self.registry.upgrade()
        {
            let mut state = registry.state.lock().expect("operation registry lock is healthy");
            for entry in &self.entries {
                if state.owned(entry.id(), entry.owner.as_str()).is_some_and(|current| Arc::ptr_eq(current, entry)) {
                    state.remove_owned(entry.id(), entry.owner.as_str());
                }
            }
            for entry in &self.previous {
                state.entries.entry(entry.id().to_owned()).or_default().insert(entry.owner.to_string(), entry.clone());
            }
            return;
        }
        self.remove();
    }
}

impl Manager {
    pub(crate) fn register_grpc_clock(&self) -> Result<()> {
        let mut entries = Vec::new();
        use harmos_operations::execution::operations::OperationFactory;
        let sample_tick = OperationFactory::native::<crate::plugin::grpc::samples::Tick>().map_err(|source| Error::OperationFailed {
            operation: "samples.tick".into(),
            source: Arc::new(source),
        })?;
        self.register_native(crate::operations::Registration {
            batch: crate::operations::Catalog {
                name: "harmos.samples",
                operations: vec![sample_tick],
                services: vec![],
            },
            lane: None,
            defaults: std::collections::BTreeMap::new(),
        })?;
        let factories = [
            OperationFactory::protobuf::<harmos_operations::clock::Sleep>(),
            OperationFactory::protobuf::<harmos_operations::clock::Get>(),
        ];
        for factory in factories {
            let factory = factory.map_err(|source| Error::OperationFailed {
                operation: "Time".into(),
                source: Arc::new(source),
            })?;
            let contract = factory.contract_with_settings(&self.schema_settings).map_err(|source| Error::OperationFailed {
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
        self.prepare_grpc_admission(activation, lane, false)
    }
    pub(crate) fn prepare_grpc_admission(
        &self, activation: &Activation, lane: &'static str, replace: bool,
    ) -> std::result::Result<Admission, PlacementError> {
        let catalog = activation.description.declarations.as_ref().unwrap();
        let mut entries = Vec::new();
        for contract in &catalog.operations {
            let definition = OperationDefinition {
                id: Identifier::new(&contract.id).map_err(|error| PlacementError::Invalid(error.to_string()))?,
                description: contract.description.clone(),
                visibility: match contract.visibility() {
                    model::Visibility::Protected => Visibility::Protected,
                    model::Visibility::Public => Visibility::Public,
                    _ => return Err(PlacementError::Invalid(format!("Invalid Visibility for {}", contract.id))),
                },
                policy: policy(contract.policy.as_ref()).map_err(|error| PlacementError::Invalid(error.to_string()))?,
                replay: if contract.replay_safe { ReplaySafety::Idempotent } else { ReplaySafety::Unknown },
                fingerprint: harmos_domain::Fingerprint(contract.fingerprint.clone().try_into().unwrap()),
            };
            let binding = Arc::new(Binding::new(activation, contract.clone()));
            let mut entry = self
                .binding_erased(
                    activation.owner(),
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
        self.reserve_inner(&entries, replace.then_some(activation.owner()), !replace)
            .map_err(|error| PlacementError::Invalid(error.to_string()))?;
        Ok(Admission {
            registry: Arc::downgrade(&self.registry),
            entries,
            manager: self.clone(),
            replacing: replace.then(|| activation.owner().to_owned()),
            previous: Vec::new(),
            staged: replace,
            sealed: !replace,
        })
    }
}

impl crate::operations::Context<'_> {
    pub(crate) async fn invoke_client(&self, request: InvocationsCreateRequest) -> Result<Vec<u8>> {
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
                field: "Create Invocation fingerprint",
                reason: "Create Invocation requires the admitted operation fingerprint",
            });
        }
        self.submit_encoded(&request.operation, request.payload)?.await
    }

    pub(crate) fn submit_grpc_child(&self, caller: &crate::plugin::grpc::CallingPlugin, child: ChildRequest) -> Result<ChildInvocation> {
        let owner = caller.owner();
        let requires = &caller.description.declarations.as_ref().expect("admitted description").requires;
        if child.background && child.operation != "harmos.host.v1.Time/Sleep" {
            return Err(Error::InvalidInput {
                field: "child policy timer",
                reason: "Call Plugin Child permits background waits only for Time.Sleep",
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
        if !plugin.is_some_and(|binding| caller.same_activation(&binding.caller)) && entry.definition.visibility != Visibility::Public {
            return Err(denied());
        }
        if !entry.plugin_visible() {
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
            let mut resources = self.operations.resources_for(&entry);
            resources.replace(Arc::new(caller.clone()));
            resources.replace(Arc::new(harmos_operations::execution::operations::access::CallingPlugin { name: caller.name.clone() }));
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
        let invocation = self.operations.dispatch_grpc_prepared(entry, prepared, self, caller, override_policy)?;
        Ok(ChildInvocation { invocation, streams })
    }
}

impl Manager {
    fn dispatch_grpc_prepared(
        &self, entry: Arc<Entry>, prepared: harmos_operations::execution::__private::PreparedNative,
        context: &crate::operations::Context<'_>, caller: &crate::plugin::grpc::CallingPlugin, override_policy: Option<OperationPolicy>,
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
            Source::GrpcPlugin(caller.clone()),
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
    #[cfg(not(feature = "grpc"))]
    return Err(Error::InvalidInput {
        field: "host contract",
        reason: "native protobuf projections require the grpc feature",
    });
    #[cfg(feature = "grpc")]
    {
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
}

fn policy(policy: Option<&model::OperationPolicy>) -> Result<OperationPolicy> {
    OperationPolicy::try_from(policy).map_err(|_| Error::InvalidInput {
        field: "operation policy",
        reason: "invalid operation policy",
    })
}

/* === Tests === */
