use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub struct LoggerConfig {
    pub debug: bool,
}

pub fn init_logger(config: LoggerConfig) -> anyhow::Result<()> {
    let filter = if config.debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_timer(tracing_subscriber::fmt::time::ChronoUtc::rfc_3339()),
        )
        .with(filter)
        .init();

    Ok(())
}

