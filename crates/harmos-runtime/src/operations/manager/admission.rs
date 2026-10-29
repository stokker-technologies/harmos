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
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Manager {
    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub fn validate_requirements(&self, candidate: &crate::plugin::PluginDefinitions) -> Result<()> {
        let state = self.registry.state.lock().expect("operation registry lock is healthy");
        for (definition, required) in candidate.host_requirements() {
            let id = required.contract.operation().id.as_str();
            let selected = state.host_provider(id, &required.contract).map_err(|error| match error {
                Error::NotRegistered { domain, .. } => Error::NotRegistered {
                    domain,
                    name: format!("{}/{id}", definition.id),
                },
                other => other,
            })?;
            selected.available()?;
        }
        Ok(())
    }

    pub(in crate::operations) fn reserve(&self, entries: &[Arc<Entry>]) -> Result<()> {
        let mut identities = std::collections::BTreeSet::new();
        for entry in entries {
            if !identities.insert((entry.id(), entry.owner.as_str())) {
                return Err(Error::AlreadyRegistered {
                    domain: "operation",
                    name: entry.id().to_owned(),
                });
            }
        }
        #[cfg(feature = "grpc")]
        for entry in entries {
            let Some(contract) = &entry.contract else {
                continue;
            };
            let Some(binding) = contract.grpc() else {
                continue;
            };
            crate::grpc::descriptor(&binding.package, &binding.service, &[(&binding.method, contract)]).map_err(|source| {
                Error::OperationFailed {
                    operation: entry.id().to_owned(),
                    source: Arc::new(source),
                }
            })?;
        }
        let (result, retired) = {
            let mut state = self.registry.state.lock().expect("operation registry lock is healthy");
            let retired = state.retired();
            for caller in entries {
                let Some(factory) = caller.binding.downcast_ref::<harmos_sdk::operations::OperationFactory>() else {
                    continue;
                };
                for required in factory.operations {
                    let id = required.operation.id;
                    let target = entries
                        .iter()
                        .find(|entry| entry.id() == id && entry.owner == caller.owner)
                        .or_else(|| state.owned(id, caller.owner.as_str()))
                        .or_else(|| entries.iter().find(|entry| entry.id() == id))
                        .map(Ok)
                        .unwrap_or_else(|| state.select(id, None))?;
                    let visibility = target.definition.visibility;
                    let runtime_caller = matches!(Source::for_binding(caller), Source::Host);
                    let runtime_target = matches!(Source::for_binding(target), Source::Host);
                    let same_owner = caller.owner == target.owner && runtime_caller == runtime_target;
                    let permitted = same_owner
                        || visibility == crate::operations::Visibility::Shared
                        || (runtime_caller && visibility == crate::operations::Visibility::Host);
                    if !permitted {
                        return Err(Error::PermissionDenied);
                    }
                    if target.definition.fingerprint
                        != (required.operation.fingerprint)().map_err(|source| Error::OperationDeclaration { source })?
                    {
                        return Err(Error::InvalidInput {
                            field: "declared operation",
                            reason: "callee fingerprint differs",
                        });
                    }
                }
            }
            let result = if state.closing || !self.instances.is_accepting() {
                Err(Error::Closing { domain: "operations" })
            } else if let Some(entry) = entries.iter().find(|entry| {
                state.owned(entry.id(), entry.owner.as_str()).is_some()
                    || self.resident.contains(&format!("{}.{}", entry.owner, entry.id()))
            }) {
                Err(Error::AlreadyRegistered {
                    domain: "operation",
                    name: entry.id().to_owned(),
                })
            } else {
                #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
                let gate = entries
                    .iter()
                    .find_map(|entry| entry.plugin.as_ref())
                    .map_or(Ok(()), |scope| if scope.is_closed() { Err(Error::Closing { domain: "plugin scope" }) } else { Ok(()) });
                #[cfg(not(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar")))]
                let gate: Result<()> = Ok(());
                #[cfg(feature = "grpc")]
                let gate = gate.and_then(|()| {
                    let mut staged = BTreeMap::new();
                    for entry in entries {
                        let Some(binding) = entry.contract.as_ref().and_then(|contract| contract.grpc()) else {
                            continue;
                        };
                        let path = binding.path();
                        if state.grpc_services.contains(&format!("{}.{}", binding.package, binding.service))
                            || state.grpc.get(&path).and_then(Weak::upgrade).is_some()
                            || staged.insert(path.clone(), Arc::downgrade(entry)).is_some()
                        {
                            return Err(Error::AlreadyRegistered { domain: "gRPC method", name: path });
                        }
                    }
                    state.grpc.extend(staged);
                    Ok(())
                });
                if gate.is_ok() {
                    for entry in entries {
                        state.entries.entry(entry.id().to_owned()).or_default().insert(entry.owner.to_string(), Arc::clone(entry));
                    }
                }
                gate
            };
            (result, retired)
        };
        // Resource owners removed by opportunistic retirement never drop under a registry guard.
        drop(retired);
        result
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
