/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Bind Root and Scoped Call Contexts

/* === Modules === */

mod private {
    pub trait Sealed {}
    impl Sealed for super::CallContext {}
    impl Sealed for super::HostContext {}
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::operations::{CallContext, HostContext, OperationHost};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/// The Trusted Context Bound by a Runtime's Typed Facade.
pub trait CallContextSource: private::Sealed {
    fn call_context(&self) -> &CallContext;

    #[doc(hidden)]
    fn parent(&self) -> Option<HostContext>;
}

/* === Implementations === */

impl CallContextSource for CallContext {
    fn call_context(&self) -> &CallContext {
        self
    }

    fn parent(&self) -> Option<HostContext> {
        None
    }
}

impl CallContextSource for HostContext {
    fn call_context(&self) -> &CallContext {
        self.context()
    }

    fn parent(&self) -> Option<HostContext> {
        Some(self.clone())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
