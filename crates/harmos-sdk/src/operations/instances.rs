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
use std::{future::Future, pin::Pin, sync::Arc};
// Third Party Imports
use harmos_domain::operations::instance::{Id, Status};
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Changed<'a> = Pin<Box<dyn Future<Output = bool> + Send + 'a>>;

/* === Structs === */

#[derive(Clone)]
pub struct InstanceAccess(pub Arc<dyn Instances>);

/* === Enums === */

/* === Traits === */

pub trait Instances: Send + Sync {
    fn owned(&self, id: Id) -> Option<Arc<dyn InstanceControl>>;
}
pub trait InstanceControl: Send + Sync {
    fn status(&self) -> Status;
    fn request_cancel(&self);
    fn subscribe(&self) -> Box<dyn InstanceChanges>;
}
pub trait InstanceChanges: Send {
    fn current(&mut self) -> Status;
    fn changed(&mut self) -> Changed<'_>;
}

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
