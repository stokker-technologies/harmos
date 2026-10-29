/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::{future::Future, pin::Pin, sync::Arc};

use harmos_domain::model::operations::lifecycle::Action;

use super::{ExecutionContext, access::AccessError};

pub type Transition<'a> = Pin<Box<dyn Future<Output = Result<(), AccessError>> + Send + 'a>>;

pub struct LifecycleAccess(pub Arc<dyn PluginLifecycle>);

pub trait PluginLifecycle: Send + Sync {
    fn begin(self: Arc<Self>, action: Action) -> Box<dyn LifecycleInstance>;
}
pub trait LifecycleInstance: Send {
    fn run<'a>(&'a mut self, context: ExecutionContext, config: &'a [u8], generation: &'a [u8], assignments: &'a [u8]) -> Transition<'a>;
    fn cleanup(&mut self) -> Transition<'_>;
}
