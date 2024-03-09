use std::sync::Once;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

static TESTING_SUBSCRIBER: Once = Once::new();

pub fn init_for_tests() {
    // This is intended to be equivalent to `env_logger::try_init().ok();`
    // `debug!()` output is seen. We could maybe add logging for `#[tracing::instrument]`?
    TESTING_SUBSCRIBER.call_once(|| {
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(EnvFilter::from_default_env())
            .init();
    });
}
