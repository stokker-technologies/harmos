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
use std::future::Future;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::{Failure, OperationDeclaration};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */
pub trait EndpointSet: Send + 'static {
    fn item_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>);
    fn transferred(&self) -> bool {
        Self::item_types() != (None, None)
    }
}

pub trait InputEndpoint: Send + 'static {
    type Item: Send + 'static;
    fn transferred(&self) -> bool {
        true
    }
}
pub trait OutputEndpoint: Send + 'static {
    type Item: Send + 'static;
    fn transferred(&self) -> bool {
        true
    }
}

pub trait Operation: Send + Sized + 'static {
    type Declaration: OperationDeclaration;
    type Resources: Send + Sync + 'static;
    type Error: std::error::Error + Send + Sync + 'static;
    type Context<'a>: Send;
    type Endpoints: EndpointSet;

    fn new(resources: Arc<Self::Resources>) -> Result<Self, Self::Error>;

    fn initialize<'ctx>(
        &mut self,
        _context: Self::Context<'ctx>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }

    fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut <Self::Declaration as OperationDeclaration>::Request,
        endpoints: &mut Self::Endpoints,
    ) -> impl Future<
        Output = Result<<Self::Declaration as OperationDeclaration>::Response, Self::Error>,
    > + Send;

    fn stop(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }

    fn finalize(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }

    fn transferred(&self) -> bool {
        false
    }

    fn classify(_error: &Self::Error) -> Failure {
        Failure::Permanent
    }
}

/* === Implementations === */
impl EndpointSet for () {
    fn item_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (None, None)
    }
}

impl<I: InputEndpoint, O: OutputEndpoint> EndpointSet for (I, O) {
    fn transferred(&self) -> bool {
        self.0.transferred() || self.1.transferred()
    }
    fn item_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (
            Some(std::any::TypeId::of::<I::Item>()),
            Some(std::any::TypeId::of::<O::Item>()),
        )
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
