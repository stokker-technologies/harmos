/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed In-Process Operation Calls

/* === Modules === */

mod context;
pub mod storage;
pub mod streams;

/* === Re-Exports === */

pub use context::CallContextSource;

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::operations::{HostContext, Manager, NativeInvocation, NativeOperation, OperationPolicy};
use crate::{Application, Error, Result, Runtime, runtime::Handle};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// One In-Process Caller; Each Method Admits Its Registered Native Operation.
#[derive(Clone)]
pub struct Call {
    operations: Manager,
    parent: Option<HostContext>,
    options: CallOptions,
}

/// Overrides for Calls Made With This Context; Registration Is Unchanged.
#[derive(Clone, Copy, Debug, Default)]
pub struct CallOptions {
    pub policy: Option<OperationPolicy>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Call {
    pub(crate) fn new(operations: Manager, context: &(impl CallContextSource + ?Sized)) -> Self {
        let parent = context.parent();
        let mut context = context.call_context().clone();
        let options = context.remove::<CallOptions>().map(|options| *options).unwrap_or_default();
        Self {
            operations: operations.with_context(context),
            parent,
            options,
        }
    }

    pub fn storage(&self) -> storage::Storage<'_> {
        storage::Storage { call: self }
    }

    pub fn streams(&self) -> streams::Streams<'_> {
        streams::Streams { call: self }
    }

    /// Start Typed Work Without Encoding; Dropping Its Unsettled Handle Cancels It.
    pub fn submit<O: NativeOperation>(&self, request: O::Request, endpoints: O::Endpoints) -> Result<NativeInvocation<O>> {
        let policy = self.options.policy.map(|policy| (policy, "CallOptions.policy"));
        let parent = self.parent.as_ref().map(HostContext::borrow);
        if parent.as_ref().is_some_and(|parent| !Arc::ptr_eq(&parent.operations.resident_identity(), &self.operations.resident_identity()))
        {
            return Err(Error::InvalidInput {
                field: "Call Native Operation",
                reason: "caller belongs to another runtime; use the caller's owning runtime",
            });
        }
        self.operations.submit_native_in::<O>(request, endpoints, policy, parent.as_ref())
    }

    pub async fn call<O: NativeOperation<Endpoints = ()>>(&self, request: O::Request) -> Result<O::Response> {
        let response = self.submit::<O>(request, ())?.await?;
        response.try_unwrap().map_err(|_| Error::InvalidInput {
            field: "Call Native Operation",
            reason: "response is shared; retain one response owner",
        })
    }
}

impl<A: Application> Runtime<A> {
    pub fn for_call(&self, context: &(impl CallContextSource + ?Sized)) -> Call {
        Call::new(self.operations.execution(), context)
    }
}

impl<A: Application> Handle<A> {
    pub fn for_call(&self, context: &(impl CallContextSource + ?Sized)) -> Call {
        Call::new(self.operations.execution(), context)
    }
}

impl HostContext {
    /// Retain This Parent's Cancellation Scope and Admission Ancestry.
    pub fn for_call(&self) -> Call {
        Call::new(self.borrow().operations, self)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
