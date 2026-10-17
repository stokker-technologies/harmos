/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// In-process binding of the portable host: a thin layer over the
// manager's own submission, so in-process and remote bodies share one shape.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::caller::Identity;
use super::{
    CallContext, Callable, Context, ContextError, ContextValue, OperationInstance,
    OperationManager, Response,
};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos_domain::operations::Host for OperationManager {
    type Error = Error;
    type Instance<O: Callable> = OperationInstance<O>;

    fn context(&self) -> &CallContext {
        self.call_context()
    }

    fn with_value<T: ContextValue>(&self, value: T) -> std::result::Result<Self, ContextError> {
        let context = self.call_context().with_value(value)?;
        Ok(self.with_context(context))
    }

    fn submit_as<O: Callable>(
        &self,
        request: O::Request,
    ) -> impl Future<Output = Result<OperationInstance<O>>> + Send {
        std::future::ready(OperationManager::submit_as::<O>(self, request))
    }

    fn call_as<O: Callable>(
        &self,
        request: O::Request,
    ) -> impl Future<Output = Result<Response<O::Response>>> + Send {
        let submitted = OperationManager::submit_as::<O>(self, request);
        async move { submitted?.await }
    }

    fn submit_with_resources<O: Callable<Response = ()>, R: Send + Sync + 'static>(
        &self,
        _: O::Request,
        resources: Arc<R>,
    ) -> impl Future<Output = Result<OperationInstance<O>>> + Send {
        self.activate_resident::<O, R>(O::ID, resources, Identity::Host)
    }
}

impl harmos_domain::operations::Host for Context<'_> {
    type Error = Error;
    type Instance<O: Callable> = OperationInstance<O>;

    fn context(&self) -> &CallContext {
        Context::context(self)
    }

    fn with_value<T: ContextValue>(&self, value: T) -> std::result::Result<Self, ContextError> {
        let context = Context::context(self).with_value(value)?;
        let mut child = self.clone();
        child.operations = child.operations.with_context(context);
        Ok(child)
    }

    fn submit_as<O: Callable>(
        &self,
        request: O::Request,
    ) -> impl Future<Output = Result<OperationInstance<O>>> + Send {
        std::future::ready(Context::submit_as::<O>(self, request))
    }

    fn call_as<O: Callable>(
        &self,
        request: O::Request,
    ) -> impl Future<Output = Result<Response<O::Response>>> + Send {
        let submitted = Context::submit_as::<O>(self, request);
        async move { submitted?.await }
    }

    // A resident outlives the attempt that activated it; its lease names the
    // attempt's caller.
    fn submit_with_resources<O: Callable<Response = ()>, R: Send + Sync + 'static>(
        &self,
        _: O::Request,
        resources: Arc<R>,
    ) -> impl Future<Output = Result<OperationInstance<O>>> + Send {
        self.operations
            .activate_resident::<O, R>(O::ID, resources, self.origin.identity.clone())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
