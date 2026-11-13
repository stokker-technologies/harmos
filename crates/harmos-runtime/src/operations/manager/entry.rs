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

impl Entry {
    pub(super) fn client_visible(&self) -> bool {
        use harmos_protocol::schema::{OperationFace, operation_identity};
        let face = operation_identity(self.id());
        let instance = matches!(face, Ok(OperationFace::Plugin)) && self.id().contains('/') && self.host_provided();
        self.definition.visibility == crate::operations::Visibility::Public
            && matches!(face, Ok(OperationFace::Client) | Ok(OperationFace::Plugin) if !instance)
    }

    pub(super) fn context_visible(&self) -> bool {
        use harmos_protocol::schema::{OperationFace, operation_identity};
        let face = operation_identity(self.id());
        let instance = matches!(face, Ok(OperationFace::Plugin)) && self.id().contains('/') && self.host_provided();
        matches!(face, Ok(OperationFace::Context) | Ok(OperationFace::Plugin) if !instance)
            && !self
                .contract
                .as_ref()
                .and_then(|contract| contract.grpc())
                .is_some_and(|grpc| grpc.package == "harmos.v1" || grpc.package.starts_with("harmos.v1."))
    }

    // Move a resident control onto the unbounded resident lane.

    // A resident control and its children outlive the committed plugin's
    // admission until the host cancels them.
    pub(in crate::operations) fn retires_late(&self) -> bool {
        false
    }

    fn host_provided(&self) -> bool {
        #[cfg(feature = "plugin-guest")]
        if self.plugin.is_some() {
            return false;
        }
        self.binding.is::<harmos_operations::execution::operations::OperationFactory>()
    }

    pub(in crate::operations) fn declared(&self) -> &OperationDefinition {
        &self.definition
    }

    pub(in crate::operations) fn id(&self) -> &str {
        self.definition.id.as_str()
    }

    pub(super) fn lifecycle_control(&self) -> bool {
        #[cfg(feature = "plugin-guest")]
        if self.sdk.is_some() {
            return true;
        }
        self.retirement_control()
    }

    pub(in crate::operations) fn retirement_control(&self) -> bool {
        #[cfg(feature = "plugin-guest")]
        if self.sdk == Some(harmos_operations::lifecycle::Action::Shutdown) {
            return true;
        }
        false
    }

    pub(in crate::operations) fn available_to(&self, origin: &Origin) -> Result<()> {
        #[cfg(feature = "plugin-guest")]
        if origin.has_lifecycle_loan() {
            return self.available_for_lifecycle();
        }

        let _ = origin;
        self.available()
    }

    #[cfg(feature = "plugin-guest")]
    pub(in crate::operations) fn admitted_shutdown(&self, control: &Arc<Instance>) -> bool {
        self.sdk == Some(harmos_operations::lifecycle::Action::Shutdown)
            && self.active.load(Ordering::Acquire)
            && !control.is_settled()
            && self.pending.lock().expect("operation settlement lock is healthy").values().any(|original| Arc::ptr_eq(original, control))
    }

    #[cfg(feature = "plugin-guest")]
    pub(in crate::operations) fn available_for_lifecycle(&self) -> Result<()> {
        if self.sdk.is_some() || self.active.load(Ordering::Acquire) {
            return self.available();
        }
        if self.retirement.load(Ordering::Acquire) != Retirement::Runtime as u8 {
            return Err(Error::Closing { domain: "operation" });
        }
        if self.plugin.as_ref().is_some_and(|scope| !scope.has_committed()) {
            return Err(Error::NotReady { domain: "operation plugin" });
        }
        Ok(())
    }

    pub(in crate::operations) fn available(&self) -> Result<()> {
        if !self.publication.load(Ordering::Acquire) {
            return Err(Error::NotReady { domain: "operation batch" });
        }
        let retiring = self.retires_late();
        if !self.active.load(Ordering::Acquire) && !retiring {
            return Err(Error::Closing { domain: "operation" });
        }
        #[cfg(feature = "plugin-guest")]
        if let Some(scope) = &self.plugin {
            if let Some(action) = self.sdk {
                use harmos_operations::lifecycle::Action;
                return match action {
                    Action::Initialize if scope.is_closed() => Err(Error::Closing { domain: "SDK initialization" }),
                    Action::Initialize if scope.has_committed() => Err(Error::InvalidInput {
                        field: "SDK initialization",
                        reason: "generation is already committed",
                    }),
                    Action::Initialize | Action::Shutdown => Ok(()),
                };
            }
            if scope.is_closed() && !retiring {
                return Err(Error::Closing { domain: "operation plugin" });
            }
            if !scope.is_ready() && !retiring {
                return Err(Error::NotReady { domain: "operation plugin" });
            }
        }
        Ok(())
    }

    #[cfg(feature = "plugin-guest")]
    pub(in crate::operations) fn plugin(&self) -> Option<&PluginScope> {
        self.plugin.as_ref()
    }

    pub(super) fn grant_authority(
        &self, _origin: &Origin, external: Option<crate::operations::authorization::Authorization>,
    ) -> GrantAuthority {
        GrantAuthority {
            owner: self.owner.to_string(),
            operation: self.id().to_owned(),
            #[cfg(feature = "plugin-guest")]
            plugin: _origin
                .scope
                .as_ref()
                .filter(|caller| {
                    !self.plugin.as_ref().is_some_and(|scope| scope.owner() == caller.owner() && scope.generation() == caller.generation())
                })
                .map(|caller| (caller.owner().to_owned(), caller.requirement(&self.definition.id).cloned())),
            external,
        }
    }

    pub(in crate::operations) fn authorize(&self, _origin: &Origin) -> Result<()> {
        #[cfg(feature = "grpc")]
        if matches!(_origin.identity, super::super::caller::Identity::External(_)) && !self.client_visible() {
            return Err(Error::OperationUnauthorized {
                caller: "external".to_owned(),
                owner: self.owner.to_string(),
                operation: self.id().to_owned(),
            });
        }
        #[cfg(feature = "plugin-guest")]
        if let Some(caller) = &_origin.scope {
            if self.sdk.is_some() || !self.context_visible() {
                return Err(Error::OperationUnauthorized {
                    caller: caller.owner().to_owned(),
                    owner: self.owner.to_string(),
                    operation: self.id().to_owned(),
                });
            }
            let own = caller.owner() == self.owner.as_str()
                && self.plugin.as_ref().is_some_and(|scope| scope.generation() == caller.generation());
            let dependency = !own
                && self.definition.visibility == crate::operations::Visibility::Public
                && caller.allows_operation(self.owner.as_str(), self.contract.as_ref(), &self.definition.id, self.plugin.is_none());
            if !own && !dependency {
                return Err(Error::OperationUnauthorized {
                    caller: caller.owner().to_owned(),
                    owner: self.owner.to_string(),
                    operation: self.id().to_owned(),
                });
            }
        }
        Ok(())
    }

    pub(in crate::operations) fn settled(&self) -> bool {
        self.pending.lock().expect("operation settlement lock is healthy").values().all(|settled| settled.is_settled())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
