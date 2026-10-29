/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::{future::Future, pin::Pin, sync::Arc};

use harmos_domain::operations::instance::{Id, Status};

pub type Changed<'a> = Pin<Box<dyn Future<Output = bool> + Send + 'a>>;

#[derive(Clone)]
pub struct InstanceAccess(pub Arc<dyn Instances>);

pub trait Instances: Send + Sync {
    fn owned(&self, id: Id) -> Option<Arc<dyn InstanceControl>>;
}
pub trait InstanceControl: Send + Sync {
    fn status(&self) -> Status;
    fn cancel(&self);
    fn subscribe(&self) -> Box<dyn InstanceChanges>;
}
pub trait InstanceChanges: Send {
    fn current(&mut self) -> Status;
    fn changed(&mut self) -> Changed<'_>;
}
