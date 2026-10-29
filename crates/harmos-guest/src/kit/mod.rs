/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod context;
mod instance;
mod publisher;

pub mod plugin;

pub use harmos_sdk::{Operation, Service};

pub use crate::export_plugin;
pub use context::Context;
pub use harmos_domain::Refusal as OperationRefusal;
pub use harmos_domain::{UNKNOWN, UNREADABLE};
pub use instance::OperationInstance;
pub use publisher::{PublishError, Publisher};

pub const WIT: &str = include_str!("../../wit/harmos.wit");
