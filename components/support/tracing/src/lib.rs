mod layer;

#[cfg(feature = "testing")]
mod testing;

#[cfg(feature = "testing")]
pub use testing::init_for_tests;

pub use layer::{register_event_sink, unregister_event_sink, SimpleEventLayer};

// udl enum from `rust_log_forwarder.udl`
#[derive(Debug, Eq, PartialEq)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<tracing::Level> for Level {
    fn from(level: tracing::Level) -> Self {
        if level == tracing::Level::ERROR {
            Level::Error
        } else if level == tracing::Level::WARN {
            Level::Warn
        } else if level == tracing::Level::INFO {
            Level::Info
        } else if level == tracing::Level::DEBUG {
            Level::Debug
        } else if level == tracing::Level::TRACE {
            Level::Trace
        } else {
            unreachable!();
        }
    }
}

impl From<Level> for tracing::Level {
    fn from(level: Level) -> Self {
        match level {
            Level::Error => tracing::Level::ERROR,
            Level::Warn => tracing::Level::WARN,
            Level::Info => tracing::Level::INFO,
            Level::Debug => tracing::Level::DEBUG,
            Level::Trace => tracing::Level::TRACE,
        }
    }
}

#[derive(Debug)]
pub struct Event {
    pub level: Level,
    pub target: String,
    pub name: String,
    pub message: String,
    pub fields: serde_json::Value,
}

// uniffi foreign trait.
pub trait EventSink: Send + Sync {
    fn on_event(&self, event: Event);
}

use serde_json::Value as JsonValue;
impl UniffiCustomTypeConverter for JsonValue {
    type Builtin = String;

    fn into_custom(val: Self::Builtin) -> uniffi::Result<JsonValue> {
        Ok(serde_json::from_str(val.as_str()).unwrap())
    }

    fn from_custom(obj: Self) -> Self::Builtin {
        obj.to_string()
    }
}

uniffi::include_scaffolding!("tracing-support");

#[cfg(test)]
mod tests {
    use parking_lot::RwLock;
    use std::sync::Arc;

    use super::*;

    #[test]
    fn test_app() {
        use tracing_subscriber::prelude::*;
        tracing_subscriber::registry()
            .with(layer::SimpleEventLayer)
            .init();

        struct Sink {
            events: RwLock<Vec<Event>>,
        }

        impl Sink {
            fn new() -> Self {
                Self {
                    events: RwLock::new(Vec::new()),
                }
            }
        }

        impl EventSink for Sink {
            fn on_event(&self, event: Event) {
                self.events.write().push(event);
            }
        }
        let sink = Arc::new(Sink::new());

        register_event_sink("first_target", Level::Info, sink.clone());
        register_event_sink("second_target", Level::Info, sink.clone());

        tracing::event!(target: "first_target", tracing::Level::INFO, extra = -1, "event message");

        assert_eq!(sink.events.read().len(), 1);
        let event = &sink.events.read()[0];
        assert_eq!(event.target, "first_target");
        assert_eq!(event.level, Level::Info);
        assert_eq!(event.message, "event message");
        assert_eq!(event.fields.get("extra").unwrap().as_i64(), Some(-1));
    }
}
