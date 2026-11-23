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
use prost_reflect::{DescriptorPool, MessageDescriptor};

// Local Imports
use super::*;
use crate::operations::{OperationPolicy, ReplaySafety, Visibility};
use crate::operations::{json, lane};
use crate::plugin::grpc::{self, Activation, invoke::Binding};
use harmos_operations::clock::WindDown;
use harmos_protocol::host::InvocationsCreateRequest;
use harmos_protocol::model;
use harmos_protocol::plugin::ChildRequest;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct ChildInvocation {
    pub invocation: crate::operations::EncodedInvocation,
    pub streams: Option<harmos_operations::execution::operations::WireStreams>,
}

pub(crate) struct GrpcAdmission {
    registry: Weak<Registry>,
    entries: Vec<Arc<Entry>>,
    lifecycle: Vec<Arc<Entry>>,
    manager: Manager,
    replacing: Option<String>,
    previous: Vec<Arc<Entry>>,
    staged: bool,
    sealed: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl GrpcAdmission {
    pub(crate) async fn lifecycle(&self, name: &str) -> Result<()> {
        let Some(entry) = self.lifecycle.iter().find(|entry| entry.id().ends_with(name)) else {
            return Ok(());
        };
        let binding = entry.binding.downcast_ref::<Binding>().expect("plugin lifecycle binding");
        let prepared = harmos_operations::execution::__private::PreparedNative {
            instance: binding.prepare(Vec::new()),
            grant: None,
            streams: None,
        };
        let invocation = crate::operations::wire::dispatch_prepared(
            &self.manager,
            Submission {
                submitted: self.manager.clock().now(),
                entry: entry.clone(),
                cause: None,
                preparation: None,
            },
            Source::Host,
            None,
            prepared,
            crate::operations::binding::Request::from(Vec::new()),
        )?;
        let settled = invocation.settle();
        let result = invocation.await;
        let settlement = settled.await;
        result.map(|_| ()).and(settlement)
    }

    pub(crate) fn close(&self) {
        for entry in &self.entries {
            entry.lane.close();
        }
        for entry in self.entries.iter().chain(&self.lifecycle) {
            let pending = entry.pending.lock().expect("operation settlement lock is healthy");
            if !entry.retirement_control() {
                entry.active.store(false, Ordering::Release);
            }
            for control in pending.values() {
                control.request_cancel();
            }
        }
    }

    pub(crate) async fn settle(&self) -> Result<()> {
        self.close();
        let mut failure = None;
        for entry in self.entries.iter().chain(&self.lifecycle) {
            if let Err(error) = super::settle(entry).await {
                failure.get_or_insert(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }

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
            entry.lane.close();
            entry.active.store(false, Ordering::Release);
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

impl Drop for GrpcAdmission {
    fn drop(&mut self) {
        self.close();
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
        let sample_window =
            OperationFactory::native::<crate::plugin::grpc::samples::Window>().map_err(|source| Error::OperationFailed {
                operation: "samples.window".into(),
                source: Arc::new(source),
            })?;
        let plugin_load = OperationFactory::native::<crate::plugin::grpc::Load>().map_err(|source| Error::OperationFailed {
            operation: "plugins.load".into(),
            source: Arc::new(source),
        })?;
        let mut plugin_calls = Vec::new();
        use crate::plugin::grpc::facade;
        let factories = [
            OperationFactory::native_with_policy::<facade::Load>(self.registry.plugin_load),
            OperationFactory::native_with_policy::<facade::Inspect>(self.registry.plugin_load),
            OperationFactory::native::<facade::Bind>(),
            OperationFactory::native_with_policy::<facade::StartSamples>(self.registry.plugin_load),
            OperationFactory::native::<facade::StopSamples>(),
            OperationFactory::native::<facade::Settle>(),
        ];
        for factory in factories {
            let factory = factory.map_err(|source| Error::OperationFailed {
                operation: "Plugin Facade".into(),
                source: Arc::new(source),
            })?;
            plugin_calls.push(factory);
        }
        plugin_calls.extend([sample_window, plugin_load]);
        // A Load Waits on Its Plugin, Never on a Lane Slot Its Own Caller May Hold.
        let resident = crate::operations::Lane::new(crate::operations::lane::RESIDENT, crate::operations::Capacity::unbounded());
        self.register_native(crate::operations::Registration {
            batch: crate::operations::Catalog {
                name: "harmos.samples",
                operations: plugin_calls,
                services: vec![],
            },
            lane: Some(resident),
            defaults: std::collections::BTreeMap::from([("plugins.load", resident)]),
        })?;
        let factories = [
            OperationFactory::protobuf::<harmos_operations::clock::Sleep>()
                .and_then(OperationFactory::with_native::<harmos_operations::host::declarations::clock::Sleep>),
            OperationFactory::protobuf::<harmos_operations::clock::Get>()
                .and_then(OperationFactory::with_native::<harmos_operations::host::declarations::clock::Get>),
        ];
        for factory in factories
            .into_iter()
            .chain(harmos_operations::facilities::factories())
            .chain([OperationFactory::protobuf::<crate::streams::subscription::Subscribe>()])
        {
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
                self.instances.lane(crate::operations::lane::RESIDENT).expect("resident lane exists"),
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

    pub(crate) fn admit_grpc(&self, activation: &Activation) -> std::result::Result<GrpcAdmission, grpc::Error> {
        self.prepare_grpc_admission(activation, false)
    }
    pub(crate) fn prepare_grpc_admission(&self, activation: &Activation, replace: bool) -> std::result::Result<GrpcAdmission, grpc::Error> {
        let catalog = activation.description.declarations.as_ref().unwrap();
        harmos_protocol::schema::plugin_lanes(&activation.description.id, catalog)
            .map_err(|error| grpc::Error::Invalid(error.to_string()))?;
        let mut lanes = BTreeMap::new();
        lanes.insert(String::new(), lane::Admission::new("Open", usize::MAX));
        for lane in &catalog.lanes {
            let width = match lane.capacity.as_ref().expect("validated capacity") {
                model::lane_declaration::Capacity::Serial(_) => 1,
                model::lane_declaration::Capacity::Concurrent(width) => *width as usize,
                model::lane_declaration::Capacity::Unbounded(_) => usize::MAX,
            };
            lanes.insert(lane.name.clone(), lane::Admission::new(lane.name.as_str(), width));
        }
        let mut entries = Vec::new();
        let mut lifecycle = Vec::new();
        for contract in &catalog.operations {
            let hook = grpc::lifecycle_operation(&activation.description.id, &contract.id);
            if hook
                && (contract.request != "google.protobuf.Empty"
                    || contract.response != "google.protobuf.Empty"
                    || contract.visibility() != model::Visibility::Protected
                    || contract.replay_safe)
            {
                return Err(grpc::Error::Invalid(format!(
                    "Admit {}: Declare a Protected Lifecycle Operation with Empty Messages and No Replay",
                    contract.id
                )));
            }
            let definition = OperationDefinition {
                id: Identifier::new(&contract.id).map_err(|error| grpc::Error::Invalid(error.to_string()))?,
                description: contract.description.clone(),
                visibility: match contract.visibility() {
                    model::Visibility::Private => Visibility::Private,
                    model::Visibility::Protected => Visibility::Protected,
                    model::Visibility::Public => Visibility::Public,
                    _ => return Err(grpc::Error::Invalid(format!("Invalid Visibility for {}", contract.id))),
                },
                policy: policy(contract.policy.as_ref()).map_err(|error| grpc::Error::Invalid(error.to_string()))?,
                replay: if contract.replay_safe { ReplaySafety::Idempotent } else { ReplaySafety::Unknown },
                fingerprint: harmos_domain::Fingerprint(contract.fingerprint.clone().try_into().unwrap()),
            };
            let binding = Arc::new(Binding::new(activation, contract.clone()));
            let mut entry = self
                .binding_erased(
                    activation.owner(),
                    if hook {
                        self.instances.lane(crate::operations::lane::RESIDENT).expect("resident lane exists")
                    } else {
                        lanes.get(&contract.lane).expect("validated lane").clone()
                    },
                    binding,
                    definition,
                    Some(crate::operations::wire::dispatch_native),
                    #[cfg(feature = "plugin-guest")]
                    None,
                )
                .map_err(|error| grpc::Error::Invalid(error.to_string()))?;
            use prost::Message;
            let types = catalog.types.as_ref().expect("admitted catalog types");
            let protobuf = harmos_operations::operations::Protobuf {
                descriptors: harmos_protocol::schema::operation_types(types, &[&contract.request, &contract.response])
                    .map_err(|error| grpc::Error::Invalid(error.to_string()))?
                    .encode_to_vec(),
                request: contract.request.clone(),
                response: contract.response.clone(),
                input: None,
                output: None,
            };
            let metadata = crate::operations::Contract::from_protobuf(entry.definition.clone(), protobuf, None, &self.schema_settings)
                .map_err(|error| grpc::Error::Invalid(error.to_string()))?;
            Arc::get_mut(&mut entry).expect("unpublished plugin entry").contract = Some(metadata);
            if hook {
                lifecycle.push(entry);
            } else {
                entries.push(entry);
            }
        }
        self.reserve_inner(&entries, replace.then_some(activation.owner()), !replace)
            .map_err(|error| grpc::Error::Invalid(error.to_string()))?;
        Ok(GrpcAdmission {
            registry: Arc::downgrade(&self.registry),
            entries,
            lifecycle,
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

    pub(crate) fn submit_grpc_child(
        &self, caller: &crate::plugin::grpc::CallingPlugin, mut child: ChildRequest,
    ) -> Result<ChildInvocation> {
        if child.background && child.operation != "harmos.plugin.v1.Time/Sleep" {
            return Err(Error::InvalidInput {
                field: "child policy timer",
                reason: "Call Plugin Child permits background waits only for Time.Sleep",
            });
        }
        let entry = self.operations.capture_entry(&child.operation).map_err(|error| {
            if child.json.is_some() && matches!(error, Error::NotRegistered { .. }) { json::unserved(&child.operation) } else { error }
        })?;
        entry.available().map_err(|error| if child.json.is_some() { json::unserved(&child.operation) } else { error })?;
        entry.authorize_grpc_plugin(caller)?;
        let plugin = entry.binding.downcast_ref::<Binding>();
        if child.json.is_none() && entry.require_grpc_plugin(caller)? != child.fingerprint {
            return Err(Error::InvalidInput {
                field: "child fingerprint",
                reason: "callee differs from the declared requirement; refresh the plugin declarations",
            });
        }
        let response = if let Some(text) = &child.json {
            if !child.payload.is_empty() || !child.fingerprint.is_empty() || child.watch {
                return Err(Error::InvalidInput {
                    field: "JSON operation call",
                    reason: "Send JSON Only, Without Protobuf Payload, Fingerprint or Watch",
                });
            }
            let (request, response) = json_descriptors(&entry)?;
            child.payload = json::decode(&child.operation, request, text)?;
            Some(response)
        } else {
            None
        };
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
            if child.background {
                // Policy Deadlines Remain Active While Explicit Scope Waits Wind Down.
                let clock = self
                    .operations
                    .call_context()
                    .get::<Arc<crate::operations::clock::Unit>>()
                    .map_or_else(|| self.operations.clock().clone(), |unit| unit.clock.clone());
                resources.replace(Arc::new(harmos_operations::clock::ClockAccess(Arc::new(crate::operations::clock::Registered::new(
                    clock,
                    None,
                    WindDown::default(),
                )))));
            }
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
        let override_policy = child.policy.as_ref().map(|value| child_policy(entry.declared().policy, value)).transpose()?;
        let mut invocation =
            self.operations.dispatch_grpc_prepared(entry, prepared, self, caller, override_policy, child.json.is_some())?;
        if let Some(response) = response {
            let outcome = invocation.outcome;
            invocation.outcome = Box::pin(async move {
                let payload = outcome.await?;
                json::encode(&child.operation, response, &payload)
            });
        }
        Ok(ChildInvocation { invocation, streams })
    }
}

impl Manager {
    fn dispatch_grpc_prepared(
        &self, entry: Arc<Entry>, prepared: harmos_operations::execution::__private::PreparedNative,
        context: &crate::operations::Context<'_>, caller: &crate::plugin::grpc::CallingPlugin, override_policy: Option<OperationPolicy>,
        untyped: bool,
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
            Source::GrpcPlugin {
                caller: Arc::new(caller.clone()),
                untyped,
            },
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

fn child_policy(declared: OperationPolicy, override_policy: &model::OperationPolicy) -> Result<OperationPolicy> {
    let mut wire = override_policy.clone();
    let retry = wire.retries.take();
    let parsed = policy(Some(&wire))?;
    let mut applied = declared;
    if wire.timeout.is_some() {
        applied.timeout = parsed.timeout;
    }
    if wire.attempt_timeout.is_some() {
        applied.attempt_timeout = parsed.attempt_timeout;
    }
    if wire.cancellation_timeout.is_some() {
        applied.cancellation_timeout = parsed.cancellation_timeout;
    }
    if let Some(retry) = retry {
        applied.retries = match (retry.total_attempts, declared.retries) {
            (1, _) => crate::RetryPolicy::Never,
            (attempts, crate::RetryPolicy::OnTransient { backoff, .. }) if attempts > 1 => crate::RetryPolicy::OnTransient {
                total_attempts: usize::try_from(attempts).map_err(|_| Error::InvalidInput {
                    field: "child retries",
                    reason: "Choose an Attempt Count That Fits the Host Address Space",
                })?,
                backoff,
            },
            _ => {
                return Err(Error::InvalidInput {
                    field: "child retries",
                    reason: "Declare the Callee Backoff Before Enabling Retries and Choose a Positive Attempt Count",
                });
            }
        };
    }
    Ok(applied)
}

fn json_descriptors(entry: &Entry) -> Result<(MessageDescriptor, MessageDescriptor)> {
    let failure = |reason: &str| Error::OperationFailed {
        operation: entry.id().into(),
        source: Arc::new(tonic::Status::invalid_argument(format!("Call {}: {reason}", entry.id()))),
    };
    if let Some(binding) = entry.binding.downcast_ref::<Binding>() {
        let declarations = binding.caller.description.declarations.as_ref().expect("admitted declarations");
        let pool = DescriptorPool::from_file_descriptor_set(declarations.types.clone().unwrap_or_default())
            .map_err(|_| failure("Refresh the Plugin's Invalid Protobuf Descriptors"))?;
        return Ok((
            pool.get_message_by_name(&binding.contract.request).ok_or_else(|| failure("Declare the Request Descriptor"))?,
            pool.get_message_by_name(&binding.contract.response).ok_or_else(|| failure("Declare the Response Descriptor"))?,
        ));
    }
    let contract = entry.contract.as_ref().ok_or_else(|| failure("Serve a Protobuf Contract for This Operation"))?;
    if let Some(protobuf) = contract.protobuf() {
        let pool =
            DescriptorPool::decode(protobuf.descriptors.as_slice()).map_err(|_| failure("Refresh the Served Protobuf Descriptors"))?;
        return Ok((
            pool.get_message_by_name(&protobuf.request).ok_or_else(|| failure("Declare the Request Descriptor"))?,
            pool.get_message_by_name(&protobuf.response).ok_or_else(|| failure("Declare the Response Descriptor"))?,
        ));
    }
    json_projected_descriptors(entry, contract)
}

#[cfg(feature = "grpc")]
fn json_projected_descriptors(entry: &Entry, contract: &crate::operations::Contract) -> Result<(MessageDescriptor, MessageDescriptor)> {
    let binding = contract.grpc().ok_or_else(|| json::unserved(entry.id()))?;
    let service = crate::grpc::descriptor(&binding.package, &binding.service, &[(&binding.method, contract)]).map_err(|source| {
        Error::OperationFailed {
            operation: entry.id().into(),
            source: Arc::new(source),
        }
    })?;
    let method = service.methods().find(|method| method.name() == binding.method.as_ref()).expect("projected method");
    Ok((method.input(), method.output()))
}

#[cfg(not(feature = "grpc"))]
fn json_projected_descriptors(entry: &Entry, _: &crate::operations::Contract) -> Result<(MessageDescriptor, MessageDescriptor)> {
    Err(json::unserved(entry.id()))
}

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
    let (request, response) = json_projected_descriptors(entry, contract)?;
    harmos_protocol::schema::fingerprint(
        entry.id(),
        &prost_types::FileDescriptorSet {
            file: request.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        },
        request.full_name(),
        response.full_name(),
    )
    .map_err(|source| Error::OperationFailed {
        operation: entry.id().into(),
        source: Arc::new(source),
    })
}

fn policy(policy: Option<&model::OperationPolicy>) -> Result<OperationPolicy> {
    OperationPolicy::try_from(policy).map_err(|_| Error::InvalidInput {
        field: "operation policy",
        reason: "invalid operation policy",
    })
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn retry_override_keeps_the_callee_backoff_and_other_policy_fields() {
        let backoff = crate::operations::Backoff::Fixed {
            delay: Duration::from_secs(7),
            jitter: crate::operations::Jitter::None,
        };
        let declared = OperationPolicy {
            timeout: Duration::from_secs(30),
            attempt_timeout: Duration::from_secs(10),
            cancellation_timeout: Duration::from_secs(2),
            retries: crate::RetryPolicy::OnTransient { total_attempts: 2, backoff },
        };
        let applied = child_policy(
            declared,
            &model::OperationPolicy {
                retries: Some(model::RetryPolicy {
                    total_attempts: 4,
                    initial: Some(prost_types::Duration { seconds: 99, nanos: 0 }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(applied.timeout, declared.timeout);
        assert_eq!(applied.attempt_timeout, declared.attempt_timeout);
        assert_eq!(applied.cancellation_timeout, declared.cancellation_timeout);
        assert_eq!(applied.retries, crate::RetryPolicy::OnTransient { total_attempts: 4, backoff });
    }
}
