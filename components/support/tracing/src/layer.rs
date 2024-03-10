/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tracing_subscriber::Layer;

use crate::EventSink;
use tracing::field::{Field, Visit};

struct LogEntry {
    level: tracing::Level,
    sink: Arc<dyn EventSink>,
}

static SINKS_BY_TARGET: Lazy<RwLock<HashMap<String, LogEntry>>> = Lazy::new(|| {
    let h: HashMap<String, LogEntry> = HashMap::new();

    RwLock::new(h)
});

pub fn register_event_sink(target: &str, level: crate::Level, sink: Arc<dyn EventSink>) {
    SINKS_BY_TARGET.write().insert(
        target.to_string(),
        LogEntry {
            level: level.into(),
            sink,
        },
    );
}

pub fn unregister_event_sink(target: &str) {
    SINKS_BY_TARGET.write().remove(target);
}

pub struct SimpleEventLayer;

impl<S> Layer<S> for SimpleEventLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let target = event.metadata().target();
        if let Some(entry) = SINKS_BY_TARGET.read().get(target) {
            let level = *event.metadata().level();
            if level <= entry.level {
                let mut fields = BTreeMap::new();
                let mut message = String::default();
                let mut visitor = JsonVisitor(&mut message, &mut fields);
                event.record(&mut visitor);
                let event = crate::Event {
                    level: level.into(),
                    target: target.to_string(),
                    name: event.metadata().name().to_string(),
                    message,
                    fields: serde_json::to_value(&fields).unwrap_or_default(),
                };
                println!("MSG: {event:?}");
                entry.sink.on_event(event);
            }
        }
    }
}

// from https://burgers.io/custom-logging-in-rust-using-tracing
struct JsonVisitor<'a>(&'a mut String, &'a mut BTreeMap<String, serde_json::Value>);

impl<'a> JsonVisitor<'a> {
    fn record_str_value(&mut self, field_name: &str, value: String) {
        if field_name == "message" {
            *self.0 = value.to_string()
        } else {
            self.1
                .insert(field_name.to_string(), serde_json::json!(value));
        }
    }
}

impl<'a> Visit for JsonVisitor<'a> {
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.1
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.1
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.1
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.1
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_str_value(field.name(), value.to_string());
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.record_str_value(field.name(), value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_str_value(field.name(), format!("{:?}", value));
    }
}
