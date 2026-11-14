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
use std::{collections::HashMap, sync::Weak};

// Third Party Imports
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

// Local Imports
use super::Status;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct StatusLayer {
    owner: Weak<Status>,
}
#[derive(Default)]
struct Fields(HashMap<String, String>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl StatusLayer {
    pub(super) fn new(owner: Weak<Status>) -> Self {
        Self { owner }
    }
}
impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name().into(), format!("{value:?}"));
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().into(), value.into());
    }
}
impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for StatusLayer {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let mut fields = Fields::default();
        event.record(&mut fields);
        let message = fields.0.remove("message").unwrap_or_default();
        let value = harmos_protocol::client::status::Event {
            level: event.metadata().level().to_string(),
            target: event.metadata().target().into(),
            message,
            fields: fields.0,
            ..Default::default()
        };
        let _ = owner.record_log(value.clone());
        let _ = owner.record_trace(value);
    }

    fn on_new_span(&self, attributes: &Attributes<'_>, id: &Id, _: Context<'_, S>) {
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let mut fields = Fields::default();
        attributes.record(&mut fields);
        fields.0.insert("span_id".into(), id.into_u64().to_string());
        fields.0.insert("event".into(), "span_open".into());
        let _ = owner.record_trace(harmos_protocol::client::status::Event {
            level: attributes.metadata().level().to_string(),
            target: attributes.metadata().target().into(),
            message: attributes.metadata().name().into(),
            fields: fields.0,
            ..Default::default()
        });
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, _: Context<'_, S>) {
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let mut fields = Fields::default();
        values.record(&mut fields);
        fields.0.insert("span_id".into(), id.into_u64().to_string());
        fields.0.insert("event".into(), "span_record".into());
        let _ = owner.record_trace(harmos_protocol::client::status::Event { fields: fields.0, ..Default::default() });
    }

    fn on_close(&self, id: Id, context: Context<'_, S>) {
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let Some(span) = context.span(&id) else {
            return;
        };
        let _ = owner.record_trace(harmos_protocol::client::status::Event {
            level: span.metadata().level().to_string(),
            target: span.metadata().target().into(),
            message: span.metadata().name().into(),
            fields: HashMap::from([("span_id".into(), id.into_u64().to_string()), ("event".into(), "span_close".into())]),
            ..Default::default()
        });
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
