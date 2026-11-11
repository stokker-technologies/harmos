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
use std::{
    sync::{Arc, Weak},
    time::Duration,
};

// Third Party Imports
use harmos_protocol::v1;

// Local Imports
use super::*;
use crate::operations::{Backoff, Jitter, OperationPolicy, ReplaySafety, RetryPolicy, Visibility};
use crate::plugin::grpc::{Activation, Error as PlacementError, invoke::Binding};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

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
        for contract in crate::plugin::grpc::clock::contracts() {
            let definition = OperationDefinition {
                id: Identifier::new(&contract.id).unwrap(),
                description: String::new(),
                visibility: Visibility::Public,
                policy: policy(None)?,
                replay: ReplaySafety::Unknown,
                fingerprint: harmos_domain::Fingerprint(contract.fingerprint.clone().try_into().unwrap()),
            };
            let binding = Arc::new(Binding::clock(contract, self.clock().clone()));
            entries.push(self.binding_erased(
                "host",
                crate::operations::lane::RESIDENT,
                binding,
                definition,
                Some(crate::operations::wire::dispatch_native),
                None,
            )?);
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
                    v1::Visibility::Private => Visibility::Private,
                    v1::Visibility::Public => Visibility::Public,
                    _ => return Err(PlacementError::Invalid(format!("Invalid Visibility for {}", contract.id))),
                },
                policy: policy(contract.policy.as_ref()).map_err(|error| PlacementError::Invalid(error.to_string()))?,
                replay: ReplaySafety::Unknown,
                fingerprint: harmos_domain::Fingerprint(contract.fingerprint.clone().try_into().unwrap()),
            };
            let binding = Arc::new(Binding::new(activation, contract.clone()));
            let entry = self
                .binding_erased(&activation.description.id, lane, binding, definition, Some(crate::operations::wire::dispatch_native), None)
                .map_err(|error| PlacementError::Invalid(error.to_string()))?;
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
    pub(crate) fn submit_grpc_child(
        &self, owner: &str, requires: &[v1::Requirement], child: v1::ChildRequest,
    ) -> Result<crate::operations::EncodedInvocation> {
        let entry = self.operations.capture_entry(&child.operation)?;
        entry.available()?;
        let denied = || Error::OperationUnauthorized {
            caller: owner.into(),
            owner: entry.owner.to_string(),
            operation: child.operation.clone(),
        };
        let plugin = entry.binding.downcast_ref::<Binding>();
        if plugin.is_some_and(|binding| binding.owner != owner) && entry.definition.visibility != Visibility::Public {
            return Err(denied());
        }
        let required = requires.iter().find(|required| required.id == child.operation).ok_or_else(denied)?;
        if required.fingerprint != child.fingerprint || fingerprint(&entry)? != child.fingerprint {
            return Err(Error::InvalidInput {
                field: "child fingerprint",
                reason: "callee differs from the declared requirement",
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
            factory
                .prepare_grpc(
                    self.operations.resources_for(&entry),
                    &child.payload,
                    harmos_operations::execution::operations::WireStreamLimits {
                        input: crate::operations::StreamSettings {
                            items: std::num::NonZeroUsize::MIN,
                            bytes: std::num::NonZeroUsize::MIN,
                        },
                        output: crate::operations::StreamSettings {
                            items: std::num::NonZeroUsize::MIN,
                            bytes: std::num::NonZeroUsize::MIN,
                        },
                    },
                )
                .map_err(|source| Error::OperationFailed {
                    operation: child.operation.clone(),
                    source: Arc::new(source),
                })?
        };
        if let Some(override_policy) = child.policy {
            return self.operations.dispatch_grpc_prepared(entry, prepared, self, Some(policy(Some(&override_policy))?));
        }
        self.operations.dispatch_grpc_prepared(entry, prepared, self, None)
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
    if contract.input().is_some() || contract.output().is_some() {
        return Err(Error::InvalidInput {
            field: "child operation",
            reason: "child calls are unary",
        });
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
    let duration = |duration: Option<prost_types::Duration>, default| -> Result<Duration> {
        match duration {
            None => Ok(default),
            Some(value) if value.seconds >= 0 && (0..1_000_000_000).contains(&value.nanos) => {
                Ok(Duration::new(value.seconds as u64, value.nanos as u32))
            }
            _ => Err(Error::InvalidInput {
                field: "operation policy",
                reason: "invalid duration",
            }),
        }
    };
    let wire = policy.cloned().unwrap_or_default();
    let result = OperationPolicy {
        timeout: duration(wire.timeout, harmos_domain::operations::UNBOUNDED)?,
        attempt_timeout: duration(wire.attempt_timeout, harmos_domain::operations::UNBOUNDED)?,
        cancellation_timeout: duration(wire.cancellation_timeout, Duration::ZERO)?,
        retries: if wire.attempts <= 1 {
            RetryPolicy::Never
        } else {
            RetryPolicy::OnTransient {
                max_retries: wire.attempts as usize - 1,
                backoff: Backoff::Fixed {
                    delay: duration(wire.backoff, Duration::ZERO)?,
                    jitter: Jitter::None,
                },
            }
        },
    };
    result.validate().map_err(|_| Error::InvalidInput {
        field: "operation policy",
        reason: "invalid operation policy",
    })?;
    Ok(result)
}

/* === Tests === */
