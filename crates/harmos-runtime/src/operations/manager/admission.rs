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
    #[cfg(feature = "plugin-guest")]
    pub fn validate_requirements(&self, candidate: &crate::guest::host::plugin::PluginDefinitions) -> Result<()> {
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
        self.reserve_inner(entries, None, true).map(|_| ())
    }
    pub(in crate::operations) fn reserve_inner(
        &self, entries: &[Arc<Entry>], replacing: Option<&str>, apply: bool,
    ) -> Result<Vec<Arc<Entry>>> {
        let mut replaced = Vec::new();
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
            for entry in entries {
                if let Some(previous) = state.entries.get(entry.id()).and_then(|providers| {
                    providers.values().filter(|previous| replacing.is_none_or(|owner| !previous.managed_by(owner))).find(|previous| {
                        entry.binding.is::<crate::plugin::grpc::invoke::Binding>()
                            || previous.binding.is::<crate::plugin::grpc::invoke::Binding>()
                    })
                }) {
                    let previous_owner = previous
                        .binding
                        .downcast_ref::<crate::plugin::grpc::invoke::Binding>()
                        .map_or(previous.owner.as_str(), |binding| {
                            binding.caller.name.as_deref().unwrap_or(&binding.caller.description.id)
                        });
                    let next_owner = entry
                        .binding
                        .downcast_ref::<crate::plugin::grpc::invoke::Binding>()
                        .map_or(entry.owner.as_str(), |binding| binding.caller.name.as_deref().unwrap_or(&binding.caller.description.id));
                    return Err(Error::AlreadyRegistered {
                        domain: "operation",
                        name: format!("{} (owners {} and {})", entry.id(), previous_owner, next_owner),
                    });
                }
            }
            for caller in state
                .entries
                .values()
                .flat_map(|providers| providers.values())
                .filter(|entry| replacing.is_none_or(|owner| !entry.managed_by(owner)))
                .chain(entries)
            {
                let Some(binding) = caller.binding.downcast_ref::<crate::plugin::grpc::invoke::Binding>() else {
                    continue;
                };
                for required in &binding.caller.description.catalog.as_ref().expect("admitted description").requires {
                    let target = entries
                        .iter()
                        .find(|entry| entry.id() == required.id)
                        .or_else(|| state.entries.get(&required.id).and_then(|providers| providers.values().next()));
                    // Absent Callees Do Not Impose An Arrival Order.
                    let Some(target) = target else { continue };
                    if plugin_grpc::fingerprint(target)? != required.fingerprint {
                        return Err(Error::OperationPayload {
                            operation: required.id.clone(),
                            source: crate::MessageError::borrowed(
                                "Admit Plugin Requirement: Callee Fingerprint Differs from the Declaration",
                            ),
                        });
                    }
                }
            }
            for caller in entries {
                let Some(factory) = caller.binding.downcast_ref::<harmos_operations::execution::operations::OperationFactory>() else {
                    continue;
                };
                let dependencies = (factory.operations)().map_err(|source| match source {
                    harmos_operations::execution::ServingError::Declaration(source) => Error::OperationDeclaration { source },
                    source => Error::OperationFailed {
                        operation: factory.definition.id.to_string(),
                        source: Arc::new(source),
                    },
                })?;
                for required in dependencies {
                    let id = required.id.as_str();
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
                    let permitted = same_owner || runtime_caller || visibility == crate::operations::Visibility::Public;
                    if !permitted {
                        return Err(Error::PermissionDenied);
                    }
                    if target.definition.fingerprint != required.fingerprint {
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
                (Some(entry.owner.as_str()) != replacing && state.owned(entry.id(), entry.owner.as_str()).is_some())
                    || self.resident.contains(&format!("{}.{}", entry.owner, entry.id()))
            }) {
                Err(Error::AlreadyRegistered {
                    domain: "operation",
                    name: entry.id().to_owned(),
                })
            } else {
                #[cfg(feature = "plugin-guest")]
                let gate = entries
                    .iter()
                    .find_map(|entry| entry.plugin.as_ref())
                    .map_or(Ok(()), |scope| if scope.is_closed() { Err(Error::Closing { domain: "plugin scope" }) } else { Ok(()) });

                #[cfg(not(feature = "plugin-guest"))]
                let gate = Ok(());
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
                    if apply {
                        state.grpc.extend(staged);
                    }
                    Ok(())
                });
                if gate.is_ok() && apply {
                    if let Some(owner) = replacing {
                        let ids = state
                            .entries
                            .iter()
                            .filter(|(_, providers)| providers.get(owner).is_some_and(|entry| entry.managed_by(owner)))
                            .map(|(id, _)| id.clone())
                            .collect::<Vec<_>>();
                        for id in ids {
                            if let Some(entry) = state.remove_owned(&id, owner) {
                                replaced.push(entry);
                            }
                        }
                    }
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
        result.map(|()| replaced)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
