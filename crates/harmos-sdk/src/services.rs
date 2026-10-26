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

pub use harmos_domain::model::services::policy::{RestartPolicy, ServicePolicy};

/* === Imports === */

// Standard Library Imports
use std::future::Future;

// Third Party Imports

// Local Imports
use crate::operations::OperationContext;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

pub trait Service: Send + Sized + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn initialize(context: OperationContext) -> impl Future<Output = Result<Self, Self::Error>> + Send;
    fn run(&mut self, context: OperationContext) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn stop(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
    fn cleanup(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
}

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
