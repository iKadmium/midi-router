mod config;
mod device;
mod mapping;
mod osc_listener;
mod processor;
mod router;
mod session_manager;

use crate::config::ConfigLoader;
use crate::device::DeviceConfig;
use crate::mapping::MapConfig;
use crate::osc_listener::OscListener;
use crate::processor::MidiProcessor;
use crate::router::MidiRouter;
use crate::session_manager::SessionManager;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting MIDI Router application");

    // Load configurations
    let device_config = load_or_create_device_config().await?;
    let map_config = load_or_create_map_config().await?;

    // Wrap in Arc<RwLock> for shared access
    let device_config = Arc::new(RwLock::new(device_config));
    let map_config = Arc::new(RwLock::new(map_config));

    // Create MIDI processor
    let mut processor = MidiProcessor::new(device_config.clone(), map_config.clone())?;

    // Create session manager
    let session_manager = SessionManager::new();

    // Set up processor with session manager
    processor.set_session_manager(session_manager.clone());

    let processor = Arc::new(processor);

    // Create MIDI router
    let mut router = MidiRouter::new(processor.clone(), session_manager);

    // Initialize RTP MIDI sessions
    {
        let map_config_read = map_config.read().await;
        router.initialize_sessions(&map_config_read).await?;
    }

    // Initialize OSC listeners
    {
        let map_config_read = map_config.read().await;
        if !map_config_read.osc_sources.is_empty() {
            let osc_listener = OscListener::new(processor.clone());
            osc_listener
                .start_listeners(&map_config_read.osc_sources)
                .await?;
            info!(
                "Started {} OSC listeners",
                map_config_read.osc_sources.len()
            );
        }
    }

    let session_count = router.get_session_names().await.len();
    info!("MIDI Router ready with {session_count} sessions");

    // Keep the application running
    tokio::signal::ctrl_c().await?;
    info!("Shutting down MIDI Router");

    Ok(())
}

async fn load_or_create_device_config() -> Result<DeviceConfig> {
    let path = "config/devices.json";

    match ConfigLoader::load_device_config(path) {
        Ok(config) => {
            info!("Loaded device configuration from {}", path);
            Ok(config)
        }
        Err(e) => {
            error!("Failed to load device config: {e}.");
            Err(e)
        }
    }
}

async fn load_or_create_map_config() -> Result<MapConfig> {
    let path = "config/map.json";

    match ConfigLoader::load_map_config(path) {
        Ok(config) => {
            info!("Loaded map configuration from {}", path);
            Ok(config)
        }
        Err(e) => {
            error!("Failed to load map config: {e}");
            Err(e)
        }
    }
}
