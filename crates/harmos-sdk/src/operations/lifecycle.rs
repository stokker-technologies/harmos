/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use super::{OperationContext, access::AccessError};
use harmos_domain::model::operations::lifecycle::Action;
use std::{future::Future, pin::Pin, sync::Arc};

/* === Type Aliases === */

pub type Transition<'a> = Pin<Box<dyn Future<Output = Result<(), AccessError>> + Send + 'a>>;

/* === Structs === */

pub struct LifecycleAccess(pub Arc<dyn PluginLifecycle>);

/* === Traits === */

pub trait PluginLifecycle: Send + Sync {
    fn begin(self: Arc<Self>, action: Action) -> Box<dyn LifecycleInstance>;
}
pub trait LifecycleInstance: Send {
    fn run<'a>(&'a mut self, context: OperationContext, config: &'a [u8], generation: &'a [u8], assignments: &'a [u8]) -> Transition<'a>;
    fn cleanup(&mut self) -> Transition<'_>;
}
