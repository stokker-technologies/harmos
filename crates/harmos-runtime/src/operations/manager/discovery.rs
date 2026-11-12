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

use std::ops::Bound;
use std::sync::Arc;
use std::sync::atomic::Ordering;
// Third Party Imports
use harmos_protocol::v1 as host;
use prost::Message;
// Local Imports
use super::{Entry, Manager};
use crate::operations::Contract;
use crate::operations::caller::Source;
use crate::{Error, Identifier, Result};
use harmos_operations::plugin::Grant;
use harmos_protocol::v1::{CatalogEntriesListRequest, CatalogEntriesListResponse};

/* === Constants === */

const CURSOR_BYTES: usize = 4 * 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

type ProviderEntry = (String, String, Arc<Entry>);

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Manager {
    pub(in crate::operations) fn discover(
        &self, source: &Source, request: &CatalogEntriesListRequest,
    ) -> Result<CatalogEntriesListResponse> {
        let origin = source.check()?;
        let (rows, bytes) = harmos_operations::host::limits(request).map_err(|_| Error::InvalidInput {
            field: "catalog limits",
            reason: "rows must be 1..=256 and bytes must be 2..=4194304",
        })?;
        let after: Option<(String, String)> = request
            .page
            .as_ref()
            .map(|page| &page.page_token)
            .filter(|token| !token.is_empty())
            .map(|cursor| {
                if cursor.len() > CURSOR_BYTES {
                    return Err(Error::InvalidInput {
                        field: "discovery cursor",
                        reason: "cursor is too large",
                    });
                }
                let key: (String, String) = serde_json::from_str(cursor).map_err(|_| Error::InvalidInput {
                    field: "discovery cursor",
                    reason: "cursor must contain an operation and owner",
                })?;
                for value in [&key.0, &key.1] {
                    Identifier::new(value.as_str()).map_err(|_| Error::InvalidInput {
                        field: "discovery cursor",
                        reason: "cursor contains an invalid identifier",
                    })?;
                }
                Ok(key)
            })
            .transpose()?;

        let mut output = CatalogEntriesListResponse::default();
        let mut count = 0;
        let mut last = None;
        let mut position = after;
        while let Some((id, owner, entry)) = self.next_discovery(position.as_ref())? {
            position = Some((id.clone(), owner.clone()));
            if !entry.active.load(Ordering::Acquire) || entry.available().is_err() || entry.authorize(&origin).is_err() {
                continue;
            }
            #[cfg(feature = "plugin-guest")]
            if entry.sdk.is_some() || entry.plugin.as_ref().is_some_and(|scope| !scope.is_ready() || scope.is_closed()) {
                continue;
            }
            let Some(contract) = &entry.contract else {
                continue;
            };
            let authorization = match self.authorize_external(&entry, &origin) {
                Ok(authorization) => authorization,
                Err(Error::OperationUnauthorized { .. } | Error::PermissionDenied) => continue,
                Err(error) => return Err(error),
            };
            // The Callback Can Race Reload/Shutdown; Never Report The Retired Provider.
            source.check()?;
            if !entry.active.load(Ordering::Acquire) || entry.available().is_err() {
                continue;
            }
            let state = self.registry.state.lock().expect("operation registry lock is healthy");
            if state.closing || !self.instances.is_accepting() {
                return Err(Error::Closing { domain: "operations" });
            }
            if !state.entries.get(&id).and_then(|owners| owners.get(&owner)).is_some_and(|registered| Arc::ptr_eq(registered, &entry)) {
                continue;
            }
            #[cfg(not(feature = "grpc"))]
            let grpc_available = false;
            #[cfg(feature = "grpc")]
            let grpc_available = entry.grpc.is_some()
                && contract.grpc().is_some_and(|binding| {
                    state.grpc.get(&binding.path()).is_some_and(|registered| registered.as_ptr() == Arc::as_ptr(&entry))
                });
            drop(state);
            let required = match &authorization {
                Some(crate::operations::authorization::Authorization::Restricted(required)) => Some(required),
                _ => None,
            };
            let grants: &[Grant] = required.map_or(&[], |selected| selected.grants.as_slice());
            #[cfg(feature = "plugin-guest")]
            let grants = origin
                .scope
                .as_ref()
                .and_then(|caller| caller.requirement(&entry.definition.id))
                .map_or(grants, |required| required.grants.as_slice());
            let selected_grants;
            let grants = if let Some(required) = required {
                #[cfg(feature = "plugin-guest")]
                let plugin_required = origin.scope.as_ref().and_then(|caller| caller.requirement(&entry.definition.id));
                #[cfg(not(feature = "plugin-guest"))]
                let plugin_required: Option<&harmos_operations::inventory::contracts::Required> = None;

                selected_grants = required
                    .grants
                    .iter()
                    .filter(|grant| plugin_required.is_none_or(|selected| selected.grants(grant)))
                    .cloned()
                    .collect::<Vec<_>>();
                selected_grants.as_slice()
            } else {
                grants
            };
            let metadata = host::DiscoveredOperation {
                owner: entry.owner.to_string(),
                generation: entry.generation.value(),
                contract: Some(describe(contract)?),
                grants: grants
                    .iter()
                    .map(|grant| host::Grant {
                        operation: grant.operation.to_string(),
                        scope: grant.scope.as_ref().map(ToString::to_string),
                    })
                    .collect(),
                conditional_access: true,
                grpc_available,
            };
            let cursor = serde_json::to_string(&(id, owner)).expect("identifier cursor serializes");
            output.operations.push(metadata);
            output.next_page_token = cursor.clone();
            if count == rows || output.encoded_len() > bytes {
                output.operations.pop();
                output.next_page_token = last.take().unwrap_or_default();
                if count == 0 {
                    return Err(Error::ResourceExhausted { resource: "discovery schema bytes" });
                }
                return Ok(output);
            }
            last = Some(cursor);
            count += 1;
        }
        output.next_page_token.clear();
        Ok(output)
    }

    fn next_discovery(&self, after: Option<&(String, String)>) -> Result<Option<ProviderEntry>> {
        let state = self.registry.state.lock().expect("operation registry lock is healthy");
        if state.closing || !self.instances.is_accepting() {
            return Err(Error::Closing { domain: "operations" });
        }
        let start = after.map_or(Bound::Unbounded, |key| Bound::Included(key.0.as_str()));
        for (id, owners) in state.entries.range::<str, _>((start, Bound::Unbounded)) {
            let start = after.filter(|key| key.0 == *id).map_or(Bound::Unbounded, |key| Bound::Excluded(key.1.as_str()));
            if let Some((owner, entry)) = owners.range::<str, _>((start, Bound::Unbounded)).next() {
                return Ok(Some((id.clone(), owner.clone(), entry.clone())));
            }
        }
        Ok(None)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn describe(contract: &Contract) -> Result<host::DiscoveredContract> {
    use crate::operations::{Backoff, Jitter, ReplaySafety, RetryPolicy, Visibility};
    let definition = contract.operation();
    let policy = definition.policy;
    let retries = match policy.retries {
        RetryPolicy::Never => None,
        RetryPolicy::OnTransient { total_attempts, backoff } => {
            let (initial, maximum, jitter) = match backoff {
                Backoff::Fixed { delay, jitter } => (delay, None, jitter),
                Backoff::Exponential { initial, maximum, jitter } => (initial, Some(maximum), jitter),
            };
            Some(harmos_protocol::v1::RetryPolicy {
                total_attempts: total_attempts as u64,
                initial: Some(duration(initial)?),
                maximum: maximum.map(duration).transpose()?,
                deterministic_jitter: jitter == Jitter::Deterministic,
            })
        }
    };
    Ok(host::DiscoveredContract {
        id: definition.id.to_string(),
        description: definition.description.clone(),
        visibility: match definition.visibility {
            Visibility::Private => host::Visibility::Private,
            Visibility::Public => host::Visibility::Public,
        } as i32,
        policy: Some(harmos_protocol::v1::OperationPolicy {
            timeout: Some(duration(policy.timeout)?),
            attempt_timeout: Some(duration(policy.attempt_timeout)?),
            cancellation_timeout: Some(duration(policy.cancellation_timeout)?),
            retries,
        }),
        replay_safe: definition.replay != ReplaySafety::Unknown,
        fingerprint: definition.fingerprint.0.to_vec(),
        protobuf: descriptors(contract)?,
        grpc: contract.grpc().map(|binding| host::Grpc {
            package: binding.package.to_string(),
            service: binding.service.to_string(),
            method: binding.method.to_string(),
            managed: binding.managed,
        }),
    })
}
fn duration(value: std::time::Duration) -> Result<prost_types::Duration> {
    let seconds = i64::try_from(value.as_secs()).map_err(|_| Error::InvalidInput {
        field: "operation policy",
        reason: "duration exceeds protocol range",
    })?;
    Ok(prost_types::Duration {
        seconds,
        nanos: value.subsec_nanos() as i32,
    })
}
fn descriptors(contract: &Contract) -> Result<Option<host::Protobuf>> {
    if let Some(protobuf) = contract.protobuf() {
        return Ok(Some(protobuf.clone()));
    }
    projected_descriptors(contract)
}
#[cfg(feature = "grpc")]
fn projected_descriptors(contract: &Contract) -> Result<Option<host::Protobuf>> {
    let Some(binding) = contract.grpc() else { return Ok(None) };
    let service = crate::grpc::descriptor(&binding.package, &binding.service, &[(&binding.method, contract)]).map_err(|source| {
        Error::OperationFailed {
            operation: contract.operation().id.to_string(),
            source: Arc::new(source),
        }
    })?;
    let method = service.methods().find(|method| method.name() == binding.method).expect("declared method");
    let request = payload(method.input(), "request", binding.managed || contract.input().is_some());
    let response = if contract.output().is_some() && !binding.managed {
        let root = service
            .parent_pool()
            .get_message_by_name(&format!("{}.{}Messages.{}.Row0", binding.package, binding.service, binding.method))
            .expect("projected contract root");
        let prost_reflect::Kind::Message(response) = root.get_field_by_name("response").expect("response direction").kind() else {
            unreachable!("message response")
        };
        response.full_name().to_owned()
    } else {
        payload(method.output(), "response", binding.managed)
    };
    let input = contract.input().map(|_| payload(method.input(), "item", true));
    let output = contract.output().map(|_| payload(method.output(), "item", binding.managed));
    let types = prost_types::FileDescriptorSet {
        file: service.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
    };
    let roots: Vec<_> =
        [Some(&request), Some(&response), input.as_ref(), output.as_ref()].into_iter().flatten().map(String::as_str).collect();
    let types = harmos_protocol::schema::operation_types(&types, &roots).map_err(|source| Error::OperationFailed {
        operation: contract.operation().id.to_string(),
        source: Arc::new(source),
    })?;
    Ok(Some(host::Protobuf {
        descriptors: types.encode_to_vec(),
        request,
        response,
        input,
        output,
    }))
}
#[cfg(not(feature = "grpc"))]
fn projected_descriptors(_: &Contract) -> Result<Option<host::Protobuf>> {
    Ok(None)
}
#[cfg(feature = "grpc")]
fn payload(message: prost_reflect::MessageDescriptor, field: &str, streamed: bool) -> String {
    if !streamed {
        return message.full_name().to_owned();
    }
    let field = message.get_field_by_name(field).expect("stream envelope payload");
    match field.kind() {
        prost_reflect::Kind::Message(message) => message.full_name().to_owned(),
        _ => unreachable!("message payload"),
    }
}

/* === Tests === */
