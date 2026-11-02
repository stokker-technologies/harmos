/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Prepared Carrier Configuration And Publication

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::{ExecutionContext, ServingError, StartupError, WireTable};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl WireTable {
    pub(crate) fn resource<T: Send + Sync + 'static>(&mut self, value: impl Into<Arc<T>>) -> &mut Self {
        if self.resources_frozen {
            self.configuration_errors.push(StartupError::Binding {
                source: Box::new(ServingError::StoreFrozen),
            });
            return self;
        }
        if let Err(crate::operations::ResourceError::Duplicate { name }) = self.resources.insert(value.into()) {
            self.configuration_errors.push(StartupError::DuplicateResource { name });
        }
        self
    }

    #[doc(hidden)]
    pub fn owner_resources(&self) -> &crate::operations::ResourceSet {
        &self.resources
    }

    #[doc(hidden)]
    pub fn context(&self) -> ExecutionContext {
        let context = ExecutionContext::new(self.trusted_resources.clone(), crate::operations::CallContext::default());
        context.with_resources(self.resources.clone())
    }

    #[doc(hidden)]
    pub fn provide_resource<T: Send + Sync + 'static>(&mut self, value: Arc<T>) -> Result<(), crate::operations::ResourceError> {
        self.trusted_resources.insert(value)?;
        self.provide(std::any::TypeId::of::<T>());
        Ok(())
    }

    pub(crate) fn catalog(&mut self, catalog: &crate::operations::Catalog) -> &mut Self {
        if !self.catalogs.insert(catalog.identity()) {
            self.configuration_errors.push(StartupError::DuplicateCatalog { name: catalog.name });
            return self;
        }
        if let Err(source) = self.register(catalog.batch()) {
            self.configuration_errors.push(StartupError::Binding { source: Box::new(source) });
        }
        self
    }

    pub(crate) fn configuration(&self) -> Result<Self, ServingError> {
        let mut configuration = Self::new(&self.owner)?;
        configuration.capacity = self.capacity;
        configuration.provided.clone_from(&self.provided);
        #[cfg(feature = "streaming")]
        {
            configuration.stream_limits = self.stream_limits;
        }
        Ok(configuration)
    }

    pub(crate) fn install_configuration(&mut self, mut configuration: Self) -> Result<(), ServingError> {
        if self.resources_frozen
            || configuration
                .handlers
                .keys()
                .chain(configuration.services.keys())
                .any(|id| self.handlers.contains_key(id) || self.services.contains_key(id))
        {
            return Err(ServingError::Catalog);
        }
        self.resources = configuration.resources;
        self.trusted_resources = configuration.trusted_resources;
        self.catalogs = configuration.catalogs;
        self.capacity = configuration.capacity;
        self.handlers.append(&mut configuration.handlers);
        self.services.append(&mut configuration.services);
        self.resources_frozen = true;
        #[cfg(feature = "streaming")]
        {
            self.stream_limits = configuration.stream_limits;
        }
        Ok(())
    }

    pub fn validate_binding(&mut self) -> Result<(), ServingError> {
        if self.binding_failed {
            return Err(ServingError::Catalog);
        }
        let mut errors = std::mem::take(&mut self.configuration_errors);
        for batch in &self.pending {
            for factory in &batch.operations {
                errors.extend(factory.missing_resources(&self.resources, &self.provided));
            }
        }
        if !errors.is_empty() {
            self.binding_failed = true;
            return Err(ServingError::Startup(Box::new(StartupError::Multiple { errors })));
        }

        let mut staged = self.configuration()?;
        staged.resources = self.resources.clone();
        staged.trusted_resources = self.trusted_resources.clone();
        let pending = std::mem::take(&mut self.pending);
        for batch in pending {
            if let Err(source) = staged.register_now(batch) {
                errors.push(StartupError::Binding { source: Box::new(source) });
            }
        }
        if staged.handlers.keys().chain(staged.services.keys()).any(|id| self.handlers.contains_key(id) || self.services.contains_key(id)) {
            errors.push(StartupError::Binding { source: Box::new(ServingError::Catalog) });
        }
        if !errors.is_empty() {
            self.binding_failed = true;
            return Err(ServingError::Startup(Box::new(StartupError::Multiple { errors })));
        }
        self.handlers.append(&mut staged.handlers);
        self.services.append(&mut staged.services);
        self.resources_frozen = true;
        Ok(())
    }
}

impl crate::setup::SetupConfiguration for WireTable {
    fn resource<T: Send + Sync + 'static>(&mut self, value: Arc<T>, replace: bool) {
        if replace && !self.resources_frozen {
            self.resources.replace(value);
            return;
        }
        self.resource::<T>(value);
    }

    fn validate(&mut self) -> Result<(), StartupError> {
        self.validate_binding().map_err(|error| match error {
            ServingError::Startup(error) => *error,
            source => StartupError::Binding { source: Box::new(source) },
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
