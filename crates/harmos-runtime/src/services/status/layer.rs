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

use std::{collections::HashMap, sync::Weak};
use tracing::{Event,Subscriber,field::{Field,Visit}};
use tracing_subscriber::{Layer,layer::Context};
use super::Status;


// Standard Library Imports

// Third Party Imports

// Local Imports


/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct StatusLayer { owner: Weak<Status> }
#[derive(Default)]
struct Fields(HashMap<String,String>);


/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl StatusLayer { pub(super) fn new(owner: Weak<Status>) -> Self { Self { owner } } }
impl Visit for Fields {
 fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) { self.0.insert(field.name().into(),format!("{value:?}")); }
 fn record_str(&mut self, field: &Field, value: &str) { self.0.insert(field.name().into(),value.into()); }
}
impl<S: Subscriber> Layer<S> for StatusLayer {
 fn on_event(&self,event: &Event<'_>,_: Context<'_,S>) {
  let Some(owner) = self.owner.upgrade() else { return; };
  let mut fields=Fields::default(); event.record(&mut fields);
  let message=fields.0.remove("message").unwrap_or_default();
  let value=harmos_protocol::status::Event { level:event.metadata().level().to_string(), target:event.metadata().target().into(),message,fields:fields.0,..Default::default() };
  let _ = owner.record_log(value.clone()); let _ = owner.record_trace(value);
 }
}


/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
