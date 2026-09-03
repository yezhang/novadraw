mod app_window;
mod scene_manager;
mod selection;
mod system;

use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::app_window::start_app;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "editor=info,novadraw=info,novadraw_scene=info".into());

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(filter)
        .init();

    let _ = start_app();
    Ok(())
}
