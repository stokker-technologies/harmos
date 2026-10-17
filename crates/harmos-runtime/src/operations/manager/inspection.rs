/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use super::*;

/* === Implementations === */
impl OperationManager {
    pub fn definition(&self, id: &str) -> Result<Definition> {
        self.entry(id).map(|entry| entry.definition.clone())
    }

    pub fn definition_from(&self, owner: &str, id: &str) -> Result<Definition> {
        let entry = self.capture_provider(id, Some(owner))?;
        entry.available()?;
        Ok(entry.definition.clone())
    }

    pub fn submit_from<O: OperationDeclaration>(
        &self,
        owner: &str,
        request: O,
    ) -> Result<OperationInstance<O>> {
        let submitted = executor::now();
        let entry = self.capture_provider(O::ID, Some(owner))?;
        self.admit_entry::<O, ()>(
            (request.into(), ()),
            None,
            &[],
            &Source::Host,
            Submission {
                submitted,
                entry,
                cause: None,
                preparation: None,
            },
        )
    }

    pub fn definitions(&self) -> Vec<Definition> {
        self.registry
            .state
            .lock()
            .expect("operation registry lock is healthy")
            .definitions()
    }

    #[cfg(feature = "grpc")]
    pub(crate) fn reserve_grpc_service(&self, name: &str) -> Result<()> {
        let mut state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if state.closing {
            return Err(Error::Closing {
                domain: "operations",
            });
        }
        let prefix = format!("/{name}/");
        if state.grpc_services.contains(name)
            || state.grpc.keys().any(|path| path.starts_with(&prefix))
        {
            return Err(Error::AlreadyRegistered {
                domain: "gRPC service",
                name: name.to_owned(),
            });
        }
        state.grpc_services.insert(name.to_owned());
        Ok(())
    }

    #[cfg(feature = "grpc")]
    pub fn grpc_methods(&self) -> Vec<crate::operations::GrpcMethod> {
        let state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if state.closing {
            return Vec::new();
        }
        state
            .grpc
            .values()
            .filter_map(Weak::upgrade)
            .filter(|entry| entry.active.load(Ordering::Acquire) && entry.available().is_ok())
            .filter_map(|entry| {
                Some(crate::operations::GrpcMethod {
                    owner: entry.owner.clone(),
                    generation: entry.generation,
                    contract: entry.contract.clone()?,
                })
            })
            .collect()
    }

    #[cfg(feature = "grpc")]
    pub(crate) fn grpc_binding(&self, path: &str) -> Result<crate::operations::GrpcBinding> {
        let state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if state.closing {
            return Err(Error::Closing {
                domain: "operations",
            });
        }
        let entry = state
            .grpc
            .get(path)
            .and_then(Weak::upgrade)
            .ok_or_else(|| Error::NotRegistered {
                domain: "gRPC method",
                name: path.to_owned(),
            })?;
        if !entry.active.load(Ordering::Acquire) {
            return Err(Error::Closing {
                domain: "operation",
            });
        }
        entry.available()?;
        Ok(crate::operations::GrpcBinding {
            operations: self.clone(),
            entry,
        })
    }

    pub fn owner(&self, id: &str) -> Result<Identifier> {
        self.entry(id).map(|entry| entry.owner.clone())
    }
}
